pub mod gemini;
pub use gemini::GeminiAgent;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// All browser actions an agent can request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BrowserTool {
    Navigate(String),
    ReadPage,
    ClickElement(String),
    FillForm { selector: String, value: String },
    ExtractData(String),
    ReadClipboard,
    WriteClipboard(String),
    ExecuteJs(String),
    DownloadFile(String),
}

impl BrowserTool {
    // Stable string ID used in fingerprinting and audit logs.
    // Must match the ToolId strings in ferrite-ipi::tool_decision.
    pub fn tool_id(&self) -> &'static str {
        match self {
            BrowserTool::Navigate(_)       => "navigate",
            BrowserTool::ReadPage          => "dom.read",
            BrowserTool::ClickElement(_)   => "dom.write",
            BrowserTool::FillForm { .. }   => "form.fill",
            BrowserTool::ExtractData(_)    => "dom.read",
            BrowserTool::ReadClipboard     => "clipboard.read",
            BrowserTool::WriteClipboard(_) => "clipboard.write",
            BrowserTool::ExecuteJs(_)      => "js.execute",
            BrowserTool::DownloadFile(_)   => "download.file",
        }
    }
}

// A task submitted by the user to the agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentTask {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub prompt: String,
    pub context_url: Option<String>,
}

impl AgentTask {
    pub fn new(prompt: impl Into<String>, context_url: Option<String>) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4(),
            task_id: uuid::Uuid::new_v4(),
            prompt: prompt.into(),
            context_url,
        }
    }
}

// One tool call the agent wants to execute.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentToolCall {
    pub call_id: uuid::Uuid,
    pub tool: BrowserTool,
}

impl AgentToolCall {
    pub fn new(tool: BrowserTool) -> Self {
        Self { call_id: uuid::Uuid::new_v4(), tool }
    }
}

// The result of executing one tool call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentToolResult {
    pub call_id: uuid::Uuid,
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

impl AgentToolResult {
    pub fn ok(call_id: uuid::Uuid, data: impl serde::Serialize) -> Self {
        Self {
            call_id,
            success: true,
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
            error: None,
        }
    }

    pub fn err(call_id: uuid::Uuid, error: impl Into<String>) -> Self {
        Self {
            call_id,
            success: false,
            data: serde_json::Value::Null,
            error: Some(error.into()),
        }
    }
}

// One complete round-trip: task -> tool calls -> results -> response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentTurn {
    pub turn_id: uuid::Uuid,
    pub tool_calls: Vec<AgentToolCall>,
    pub tool_results: Vec<AgentToolResult>,
    pub final_response: Option<String>,
    pub is_complete: bool,
}

impl AgentTurn {
    pub fn new() -> Self {
        Self {
            turn_id: uuid::Uuid::new_v4(),
            tool_calls: vec![],
            tool_results: vec![],
            final_response: None,
            is_complete: false,
        }
    }
}

impl Default for AgentTurn {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Rate limit exceeded — retry after {retry_after_secs}s")]
    RateLimit { retry_after_secs: u64 },
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Tool execution error: {0}")]
    ToolError(String),
}

// Implemented by GeminiAgent. The IPI dry-run executor calls this trait.
#[async_trait::async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn run_turn(
        &self,
        task: &AgentTask,
        history: &[AgentTurn],
        executor: &dyn ToolExecutor,
    ) -> Result<AgentTurn, AgentError>;
}

// Executes tool calls on behalf of the agent runtime.
// Implemented by the Iced bridge in ferrite-ui (real run)
// and by RecordingExecutor in ferrite-ipi (dry run).
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: &AgentToolCall) -> AgentToolResult;
}

// Token-bucket rate limiter. Default: 2 req/s, burst 5.
// Conservative for testing with real API keys.
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterState>>,
}

struct RateLimiterState {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(max_rps: f64, burst: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterState {
                tokens: burst,
                max_tokens: burst,
                refill_rate: max_rps,
                last_refill: Instant::now(),
            })),
        }
    }

    /// 2 req/s, burst 5. Prevents runaway API usage during testing.
    pub fn default_testing() -> Self {
        Self::new(2.0, 5.0)
    }

    /// Returns `Ok(())` if a token is available, `Err(wait_duration)` otherwise.
    pub fn try_acquire(&self) -> Result<(), Duration> {
        let mut state = self.inner.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens = (state.tokens + elapsed * state.refill_rate).min(state.max_tokens);
        state.last_refill = now;
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            Ok(())
        } else {
            let wait = (1.0 - state.tokens) / state.refill_rate;
            Err(Duration::from_secs_f64(wait))
        }
    }

    /// Acquires one token, sleeping if necessary.
    pub async fn acquire(&self) {
        loop {
            match self.try_acquire() {
                Ok(()) => return,
                Err(wait) => tokio::time::sleep(wait).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_task_has_unique_ids() {
        let t1 = AgentTask::new("search for cats", None);
        let t2 = AgentTask::new("search for dogs", None);
        assert_ne!(t1.task_id, t2.task_id);
    }

    #[test]
    fn browser_tool_ids_are_stable() {
        assert_eq!(BrowserTool::Navigate("x".into()).tool_id(), "navigate");
        assert_eq!(BrowserTool::ReadPage.tool_id(), "dom.read");
        assert_eq!(BrowserTool::ExecuteJs("".into()).tool_id(), "js.execute");
    }

    #[test]
    fn tool_result_ok_serialises() {
        let id = uuid::Uuid::new_v4();
        let r = AgentToolResult::ok(id, "page content here");
        assert!(r.success);
    }

    #[test]
    fn rate_limiter_depletes_burst() {
        let rl = RateLimiter::new(1.0, 3.0);
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_err());
    }
}
