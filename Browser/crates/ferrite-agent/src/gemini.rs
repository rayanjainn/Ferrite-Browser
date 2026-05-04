use crate::{
    AgentError, AgentTask, AgentToolCall, AgentToolResult, AgentTurn, BrowserTool,
    AgentRuntime, RateLimiter, ToolExecutor,
};

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const DEFAULT_MODEL: &str = "gemini-2.0-flash";
const MAX_TURNS_PER_TASK: usize = 10;
const TURN_TIMEOUT_SECS: u64 = 30;

pub struct GeminiAgent {
    api_key: String,
    model: String,
    client: reqwest::Client,
    rate_limiter: RateLimiter,
}

pub fn read_api_key() -> Result<String, String> {
    if let Ok(key) = std::env::var("FERRITE_GEMINI_API_KEY") {
        let trimmed = key.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    let key_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("gemini_key.txt")));

    if let Some(ref path) = key_path {
        if path.exists() {
            let contents = std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
            let trimmed = contents.trim().to_string();
            return if trimmed.is_empty() {
                Err("gemini_key.txt exists but is empty — paste your key inside it".to_string())
            } else {
                Ok(trimmed)
            };
        }
    }

    let path_display = key_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    Err(format!(
        "Gemini API key not found.\n  Checked path: {path}\n\
         Option A: create gemini_key.txt at {path} with your key inside.\n\
         Option B: set the FERRITE_GEMINI_API_KEY environment variable.",
        path = path_display
    ))
}

impl GeminiAgent {
    pub fn from_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: DEFAULT_MODEL.to_string(),
            client: reqwest::Client::new(),
            rate_limiter: RateLimiter::default_testing(),
        }
    }

    pub fn from_env() -> Self {
        let api_key = read_api_key()
            .expect("Gemini API key not found — see error above");
        Self::from_key(api_key)
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn build_tool_manifest(&self) -> serde_json::Value {
        serde_json::json!({
            "functionDeclarations": [
                {
                    "name": "browser_navigate",
                    "description": "Navigate the browser to the given URL.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "url": { "type": "STRING", "description": "The URL to navigate to." }
                        },
                        "required": ["url"]
                    }
                },
                {
                    "name": "browser_read_page",
                    "description": "Read the full text content of the current page.",
                    "parameters": { "type": "OBJECT", "properties": {} }
                },
                {
                    "name": "browser_click",
                    "description": "Click a DOM element identified by a CSS selector.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "selector": { "type": "STRING", "description": "CSS selector of the element to click." }
                        },
                        "required": ["selector"]
                    }
                },
                {
                    "name": "browser_fill_form",
                    "description": "Fill an input field identified by a CSS selector with a value.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "selector": { "type": "STRING", "description": "CSS selector of the input element." },
                            "value":    { "type": "STRING", "description": "Value to fill in." }
                        },
                        "required": ["selector", "value"]
                    }
                },
                {
                    "name": "browser_extract_data",
                    "description": "Extract text content from a DOM element identified by a CSS selector.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "selector": { "type": "STRING", "description": "CSS selector of the element to extract from." }
                        },
                        "required": ["selector"]
                    }
                },
                {
                    "name": "browser_execute_js",
                    "description": "Execute a JavaScript snippet in the current page and return its result.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "script": { "type": "STRING", "description": "JavaScript code to execute." }
                        },
                        "required": ["script"]
                    }
                },
                {
                    "name": "browser_read_clipboard",
                    "description": "Read the current contents of the system clipboard.",
                    "parameters": { "type": "OBJECT", "properties": {} }
                },
                {
                    "name": "browser_write_clipboard",
                    "description": "Write content to the system clipboard.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "content": { "type": "STRING", "description": "Text to write to the clipboard." }
                        },
                        "required": ["content"]
                    }
                },
                {
                    "name": "browser_download_file",
                    "description": "Download a file from the given URL.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "url": { "type": "STRING", "description": "URL of the file to download." }
                        },
                        "required": ["url"]
                    }
                }
            ]
        })
    }

    fn parse_function_call(part: &serde_json::Value) -> Option<AgentToolCall> {
        let fc = part.get("functionCall")?;
        let name = fc.get("name")?.as_str()?;
        let args = fc.get("args").cloned().unwrap_or(serde_json::Value::Object(Default::default()));

        let tool = match name {
            "browser_navigate" => {
                let url = args.get("url")?.as_str()?.to_string();
                BrowserTool::Navigate(url)
            }
            "browser_read_page" => BrowserTool::ReadPage,
            "browser_click" => {
                let selector = args.get("selector")?.as_str()?.to_string();
                BrowserTool::ClickElement(selector)
            }
            "browser_fill_form" => {
                let selector = args.get("selector")?.as_str()?.to_string();
                let value = args.get("value")?.as_str()?.to_string();
                BrowserTool::FillForm { selector, value }
            }
            "browser_extract_data" => {
                let selector = args.get("selector")?.as_str()?.to_string();
                BrowserTool::ExtractData(selector)
            }
            "browser_execute_js" => {
                let script = args.get("script")?.as_str()?.to_string();
                BrowserTool::ExecuteJs(script)
            }
            "browser_read_clipboard" => BrowserTool::ReadClipboard,
            "browser_write_clipboard" => {
                let content = args.get("content")?.as_str()?.to_string();
                BrowserTool::WriteClipboard(content)
            }
            "browser_download_file" => {
                let url = args.get("url")?.as_str()?.to_string();
                BrowserTool::DownloadFile(url)
            }
            _ => return None,
        };

        Some(AgentToolCall::new(tool))
    }

    fn format_tool_results(
        results: &[AgentToolResult],
        calls: &[AgentToolCall],
    ) -> serde_json::Value {
        let parts: Vec<serde_json::Value> = results
            .iter()
            .zip(calls.iter())
            .map(|(result, call)| {
                let fn_name = match &call.tool {
                    BrowserTool::Navigate(_)       => "browser_navigate",
                    BrowserTool::ReadPage          => "browser_read_page",
                    BrowserTool::ClickElement(_)   => "browser_click",
                    BrowserTool::FillForm { .. }   => "browser_fill_form",
                    BrowserTool::ExtractData(_)    => "browser_extract_data",
                    BrowserTool::ExecuteJs(_)      => "browser_execute_js",
                    BrowserTool::ReadClipboard     => "browser_read_clipboard",
                    BrowserTool::WriteClipboard(_) => "browser_write_clipboard",
                    BrowserTool::DownloadFile(_)   => "browser_download_file",
                };
                serde_json::json!({
                    "functionResponse": {
                        "name": fn_name,
                        "response": {
                            "success": result.success,
                            "data": result.data,
                            "error": result.error
                        }
                    }
                })
            })
            .collect();

        serde_json::json!({ "role": "user", "parts": parts })
    }
}

#[async_trait::async_trait]
impl AgentRuntime for GeminiAgent {
    async fn run_turn(
        &self,
        task: &AgentTask,
        history: &[AgentTurn],
        executor: &dyn ToolExecutor,
    ) -> Result<AgentTurn, AgentError> {
        let mut turn = AgentTurn::new();

        // 1. Build system prompt
        let system_text = if let Some(url) = &task.context_url {
            format!(
                "You are Ferrite, an AI browser agent. The user is currently on: {}. \
                 Use the browser tools to help with the task. \
                 When you have a final answer, respond with plain text (no tool calls).",
                url
            )
        } else {
            "You are Ferrite, an AI browser agent. \
             Use the browser tools to help with the task. \
             When you have a final answer, respond with plain text (no tool calls)."
                .to_string()
        };

        // 2. Build contents from task prompt + history
        let mut contents: Vec<serde_json::Value> = vec![serde_json::json!({
            "role": "user",
            "parts": [{ "text": task.prompt }]
        })];

        for past_turn in history {
            if let Some(resp) = &past_turn.final_response {
                contents.push(serde_json::json!({
                    "role": "model",
                    "parts": [{ "text": resp }]
                }));
            }
        }

        let url = format!(
            "{}/{}:generateContent?key={}",
            GEMINI_API_BASE, self.model, self.api_key
        );

        let tools = self.build_tool_manifest();
        let tool_config = serde_json::json!({
            "function_calling_config": { "mode": "AUTO" }
        });

        // 3. Agentic loop
        for _ in 0..MAX_TURNS_PER_TASK {
            // a. Rate limit
            self.rate_limiter.acquire().await;

            // b+c. POST with timeout
            let body = serde_json::json!({
                "system_instruction": {
                    "parts": [{ "text": system_text }]
                },
                "contents": contents,
                "tools": [tools],
                "tool_config": tool_config
            });

            let request = self.client.post(&url).json(&body).send();
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(TURN_TIMEOUT_SECS),
                request,
            )
            .await
            .map_err(|_| AgentError::Timeout(TURN_TIMEOUT_SECS))?
            .map_err(|e| AgentError::ApiError(e.to_string()))?;

            // d. 429 rate limit
            if response.status().as_u16() == 429 {
                return Err(AgentError::RateLimit { retry_after_secs: 60 });
            }

            // e. Other non-2xx
            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                return Err(AgentError::ApiError(format!("{}: {}", status, body_text)));
            }

            // f. Parse response
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| AgentError::ParseError(e.to_string()))?;

            let parts = json
                .pointer("/candidates/0/content/parts")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();

            // g. Check for function calls
            let fn_call_parts: Vec<&serde_json::Value> = parts
                .iter()
                .filter(|p| p.get("functionCall").is_some())
                .collect();

            if fn_call_parts.is_empty() {
                // No tool calls — extract text response
                let text = parts
                    .iter()
                    .find_map(|p| p.get("text")?.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                turn.final_response = Some(text);
                turn.is_complete = true;
                return Ok(turn);
            }

            // h. Execute each function call
            let mut new_calls: Vec<AgentToolCall> = Vec::new();
            let mut new_results: Vec<AgentToolResult> = Vec::new();

            for part in &fn_call_parts {
                if let Some(call) = Self::parse_function_call(part) {
                    let result = executor.execute(&call).await;
                    new_calls.push(call.clone());
                    new_results.push(result);
                    turn.tool_calls.push(call);
                }
            }
            turn.tool_results.extend(new_results.clone());

            // i. Append model's function call parts + tool results to contents
            let model_parts: Vec<serde_json::Value> = fn_call_parts
                .iter()
                .map(|p| (*p).clone())
                .collect();
            contents.push(serde_json::json!({
                "role": "model",
                "parts": model_parts
            }));
            contents.push(Self::format_tool_results(&new_results, &new_calls));
        }

        // 4. Hit turn limit
        turn.final_response = Some("[agent hit turn limit]".to_string());
        turn.is_complete = true;
        Ok(turn)
    }
}
