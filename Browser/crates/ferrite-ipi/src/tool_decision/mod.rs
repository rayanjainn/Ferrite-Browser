// Identifies a single browser tool/capability the agent can call.
// String values must match BrowserTool::tool_id() exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn new(s: &str) -> Self { ToolId(s.to_string()) }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Conversion from BrowserTool so agent turns map directly into IPI records.
use ferrite_agent::{BrowserTool, RateLimiter};
impl From<&BrowserTool> for ToolId {
    fn from(tool: &BrowserTool) -> Self {
        ToolId::new(tool.tool_id())
    }
}

// The expected tool fingerprint for a task — derived from the user prompt alone,
// before any web content is processed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolFingerprint {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    // Tools directly and unambiguously implied by the prompt (rule-based layer).
    pub must_use: std::collections::HashSet<ToolId>,
    // Tools plausibly implied by the prompt (LLM complement layer).
    pub may_use: std::collections::HashSet<ToolId>,
}

impl ToolFingerprint {
    pub fn empty(session_id: uuid::Uuid, task_id: uuid::Uuid) -> Self {
        Self { session_id, task_id,
               must_use: Default::default(), may_use: Default::default() }
    }
    // Returns true if both sets are empty (open-ended prompts).
    pub fn is_empty(&self) -> bool {
        self.must_use.is_empty() && self.may_use.is_empty()
    }
    // Returns true if the tool is in either set.
    pub fn contains(&self, tool: &ToolId) -> bool {
        self.must_use.contains(tool) || self.may_use.contains(tool)
    }
    // Merges another fingerprint into this one (accumulation across turns).
    pub fn merge(&mut self, other: ToolFingerprint) {
        self.must_use.extend(other.must_use);
        self.may_use.extend(other.may_use);
    }
}

/// Maps prompt intent keywords to must-use tool sets.
/// Case-insensitive match on the full prompt string.
/// Returns empty set for unrecognised prompts.
pub fn rule_based_must_use(prompt: &str) -> std::collections::HashSet<ToolId> {
    let lower = prompt.to_lowercase();
    let mut tools = std::collections::HashSet::new();

    // Email tasks
    if lower.contains("email") || lower.contains("inbox") || lower.contains("mail") {
        tools.insert(ToolId::new("email.read"));
    }
    if lower.contains("send email") || lower.contains("reply to") || lower.contains("forward") {
        tools.insert(ToolId::new("email.send"));
    }
    if lower.contains("draft") {
        tools.insert(ToolId::new("email.draft"));
    }

    // Calendar tasks
    if lower.contains("calendar") || lower.contains("schedule") || lower.contains("meeting") {
        tools.insert(ToolId::new("calendar.read"));
    }
    if lower.contains("book") || lower.contains("create event") || lower.contains("add meeting") {
        tools.insert(ToolId::new("calendar.write"));
    }

    // Navigation tasks
    if lower.contains("go to") || lower.contains("navigate to") || lower.contains("open") {
        tools.insert(ToolId::new("navigate"));
    }

    // Form tasks
    if lower.contains("fill") || lower.contains("form") || lower.contains("type in") {
        tools.insert(ToolId::new("form.fill"));
    }
    if lower.contains("submit") || lower.contains("click submit") {
        tools.insert(ToolId::new("form.submit"));
    }

    // Read / extract tasks
    if lower.contains("read") || lower.contains("extract") || lower.contains("find on page")
        || lower.contains("what does") || lower.contains("title of") {
        tools.insert(ToolId::new("dom.read"));
    }

    // Download tasks
    if lower.contains("download") {
        tools.insert(ToolId::new("download.file"));
    }

    // JavaScript tasks
    if lower.contains("javascript") || lower.contains("run script") || lower.contains("execute js") {
        tools.insert(ToolId::new("js.execute"));
    }

    // Report / summarise tasks always need dom.read
    if lower.contains("report") || lower.contains("summarise") || lower.contains("summarize") {
        tools.insert(ToolId::new("dom.read"));
        tools.insert(ToolId::new("report.write"));
    }

    tools
}

pub struct LlmMayUsePredictor {
    api_key: String,
    client: reqwest::Client,
    rate_limiter: RateLimiter,
}

impl LlmMayUsePredictor {
    // Returns None if FERRITE_GEMINI_API_KEY is not set.
    // Callers must handle None gracefully (fall back to empty may_use set).
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("FERRITE_GEMINI_API_KEY").ok()?;
        Some(Self {
            api_key,
            client: reqwest::Client::new(),
            rate_limiter: RateLimiter::default_testing(),
        })
    }

    // Predicts the may-use set for a given prompt.
    // Temperature 0 — deterministic, minimal hallucination risk.
    // Returns empty set on any error.
    pub async fn predict(
        &self,
        prompt: &str,
        must_use: &std::collections::HashSet<ToolId>,
    ) -> std::collections::HashSet<ToolId> {
        // Build the tool list string (all tools not already in must_use)
        let available: Vec<String> = [
            "email.read", "email.send", "email.draft",
            "calendar.read", "calendar.write", "contacts.read",
            "storage.read", "storage.write",
            "dom.read", "dom.write", "form.fill", "form.submit",
            "network.fetch", "clipboard.read", "clipboard.write",
            "download.file", "navigate", "js.execute",
            "screenshot", "report.write",
        ]
        .iter()
        .filter(|t| !must_use.contains(&ToolId::new(t)))
        .map(|s| s.to_string())
        .collect();

        if available.is_empty() { return Default::default(); }

        let system = "You are a security analysis assistant. Given a user task prompt and a \
                      list of browser tool IDs, respond ONLY with a JSON array of tool ID \
                      strings that the agent might plausibly use to complete the task — \
                      beyond the tools already confirmed. If none are plausible, respond \
                      with an empty array []. Do not explain. Do not add tools the task \
                      clearly does not need. Err on the side of fewer tools.";

        let user_msg = format!(
            "Task: {}\n\nAvailable tools: {}\n\nRespond with a JSON array only.",
            prompt,
            available.join(", ")
        );

        self.rate_limiter.acquire().await;

        let payload = serde_json::json!({
            "system_instruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_msg }] }],
            "generationConfig": { "temperature": 0.0, "maxOutputTokens": 256 }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            self.api_key
        );

        let resp = match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            self.client.post(&url).json(&payload).send(),
        )
        .await
        {
            Ok(Ok(r)) => r,
            _ => return Default::default(),
        };

        if !resp.status().is_success() { return Default::default(); }

        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(_) => return Default::default(),
        };

        let text = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("");

        // Parse JSON array from response
        let clean = text.trim().trim_start_matches("```json").trim_end_matches("```").trim();
        let ids: Vec<String> = serde_json::from_str(clean).unwrap_or_default();

        ids.into_iter()
            .filter(|id| available.contains(id))
            .map(|id| ToolId::new(&id))
            .collect()
    }
}

pub struct ToolDecisionEngine {
    predictor: Option<LlmMayUsePredictor>,
}

impl ToolDecisionEngine {
    // Reads API key from env. If absent, LLM layer is disabled — may_use always empty.
    pub fn new() -> Self {
        Self { predictor: LlmMayUsePredictor::from_env() }
    }

    // Generates a ToolFingerprint from a user prompt.
    // Combines rule-based must_use with LLM-predicted may_use.
    // The may_use set never overlaps with must_use.
    pub async fn generate_fingerprint(
        &self,
        prompt: &str,
        task_id: uuid::Uuid,
    ) -> ToolFingerprint {
        let session_id = uuid::Uuid::new_v4();
        let must_use = rule_based_must_use(prompt);

        let may_use = match &self.predictor {
            Some(p) => {
                let raw = p.predict(prompt, &must_use).await;
                // Strip anything already in must_use to keep sets disjoint.
                raw.into_iter().filter(|t| !must_use.contains(t)).collect()
            }
            None => Default::default(),
        };

        ToolFingerprint { session_id, task_id, must_use, may_use }
    }

    // Convenience wrapper for use with a real AgentTask.
    pub async fn fingerprint_from_task(
        &self,
        task: &ferrite_agent::AgentTask,
    ) -> ToolFingerprint {
        self.generate_fingerprint(&task.prompt, task.task_id).await
    }
}

impl Default for ToolDecisionEngine {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    #[tokio::test]
    async fn engine_no_api_key_uses_rules_only() {
        // Temporarily ensure env var is absent for this test.
        std::env::remove_var("FERRITE_GEMINI_API_KEY");
        let engine = ToolDecisionEngine::new();
        let fp = engine.generate_fingerprint(
            "Check my inbox and summarise new emails",
            uuid::Uuid::new_v4(),
        ).await;
        // Rule layer fires for "inbox" / "email"
        assert!(fp.must_use.contains(&ToolId::new("email.read")));
        // may_use is empty because predictor is None
        assert!(fp.may_use.is_empty());
    }

    #[tokio::test]
    async fn engine_open_ended_prompt_produces_empty_fingerprint() {
        std::env::remove_var("FERRITE_GEMINI_API_KEY");
        let engine = ToolDecisionEngine::new();
        let fp = engine.generate_fingerprint(
            "Do something interesting on the web",
            uuid::Uuid::new_v4(),
        ).await;
        assert!(fp.is_empty());
    }

    #[tokio::test]
    async fn must_use_and_may_use_are_disjoint() {
        std::env::remove_var("FERRITE_GEMINI_API_KEY");
        let engine = ToolDecisionEngine::new();
        let fp = engine.generate_fingerprint(
            "Send an email to alice@example.com",
            uuid::Uuid::new_v4(),
        ).await;
        for tool in &fp.may_use {
            assert!(!fp.must_use.contains(tool),
                "Tool {} appears in both sets", tool);
        }
    }
}

#[cfg(test)]
mod rule_tests {
    use super::*;

    #[test]
    fn email_prompt_gives_email_read() {
        let tools = rule_based_must_use("Check my inbox and summarise new emails");
        assert!(tools.contains(&ToolId::new("email.read")));
    }

    #[test]
    fn navigate_prompt_gives_navigate() {
        let tools = rule_based_must_use("Go to https://example.com");
        assert!(tools.contains(&ToolId::new("navigate")));
    }

    #[test]
    fn open_ended_returns_empty() {
        let tools = rule_based_must_use("Do something interesting");
        assert!(tools.is_empty());
    }

    #[test]
    fn no_false_positives_on_unrelated_prompt() {
        let tools = rule_based_must_use("What is the weather today?");
        assert!(!tools.contains(&ToolId::new("email.read")));
        assert!(!tools.contains(&ToolId::new("form.submit")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_agent::BrowserTool;

    #[test]
    fn tool_id_from_browser_tool_matches() {
        assert_eq!(ToolId::from(&BrowserTool::ReadPage), ToolId::new("dom.read"));
        assert_eq!(ToolId::from(&BrowserTool::ExecuteJs("".into())), ToolId::new("js.execute"));
        assert_eq!(ToolId::from(&BrowserTool::Navigate("".into())), ToolId::new("navigate"));
    }

    #[test]
    fn fingerprint_contains_checks_both_sets() {
        let mut fp = ToolFingerprint::empty(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        fp.must_use.insert(ToolId::new("email.read"));
        fp.may_use.insert(ToolId::new("email.send"));
        assert!(fp.contains(&ToolId::new("email.read")));
        assert!(fp.contains(&ToolId::new("email.send")));
        assert!(!fp.contains(&ToolId::new("passwords.read")));
    }

    #[test]
    fn fingerprint_merge_accumulates() {
        let sid = uuid::Uuid::new_v4();
        let tid = uuid::Uuid::new_v4();
        let mut fp1 = ToolFingerprint::empty(sid, tid);
        fp1.must_use.insert(ToolId::new("email.read"));
        let mut fp2 = ToolFingerprint::empty(sid, tid);
        fp2.may_use.insert(ToolId::new("report.write"));
        fp1.merge(fp2);
        assert!(fp1.must_use.contains(&ToolId::new("email.read")));
        assert!(fp1.may_use.contains(&ToolId::new("report.write")));
    }
}
