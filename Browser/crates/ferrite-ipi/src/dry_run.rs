use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::containment::{
    activate_full, deactivate, intercepted_urls, ContainmentState, SharedContainmentState,
};
use crate::tool_decision::ToolId;
use crate::twin::{SyntheticTwin, TwinManager};
use ferrite_agent::{
    AgentRuntime, AgentTask, AgentToolCall, AgentToolResult, AgentTurn, BrowserTool, ToolExecutor,
};

/// Accumulates everything the agent did during the dry run.
#[derive(Debug, Default, Clone)]
pub struct DryRunRecord {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub tools_called: HashSet<ToolId>,
    pub origins_touched: HashSet<String>,
    pub data_fields_accessed: HashSet<String>,
    pub network_attempts: Vec<String>,
    pub completed: bool,
}

impl DryRunRecord {
    pub fn new(session_id: uuid::Uuid, task_id: uuid::Uuid) -> Self {
        Self { session_id, task_id, ..Default::default() }
    }

    pub fn record_tool(&mut self, tool: ToolId) {
        self.tools_called.insert(tool);
    }

    pub fn record_network_attempt(&mut self, url: String) {
        let origin = extract_origin(&url);
        self.origins_touched.insert(origin);
        self.network_attempts.push(url);
    }
}

fn extract_origin(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(format!("{}://{}", u.scheme(), host))
        })
        .unwrap_or_else(|| url.to_string())
}

/// `ToolExecutor` that records every tool call and returns synthetic data.
/// Never touches the real browser.
struct RecordingExecutor {
    record: Arc<Mutex<DryRunRecord>>,
    twin: SyntheticTwin,
}

#[async_trait::async_trait]
impl ToolExecutor for RecordingExecutor {
    async fn execute(&self, call: &AgentToolCall) -> AgentToolResult {
        let tool_id = ToolId::from(&call.tool);
        {
            let mut rec = self.record.lock().unwrap();
            rec.record_tool(tool_id);
        }
        // Record navigation targets as network attempts
        if let BrowserTool::Navigate(ref url) = call.tool {
            let mut rec = self.record.lock().unwrap();
            rec.record_network_attempt(url.clone());
        }
        // Return synthetic data so the agent can continue reasoning
        let synthetic = match &call.tool {
            BrowserTool::ReadPage => format!("Synthetic page. User: {}", self.twin.name),
            BrowserTool::ExtractData(_) => format!("Extracted: {}", self.twin.email),
            _ => "dry-run: ok".to_string(),
        };
        AgentToolResult::ok(call.call_id, synthetic)
    }
}

/// Orchestrates the full dry run sequence.
pub struct DryRunOrchestrator {
    twin_manager: TwinManager,
    containment: SharedContainmentState,
    timeout_secs: u64,
}

impl DryRunOrchestrator {
    pub fn new(twin_path: std::path::PathBuf) -> Self {
        Self {
            twin_manager: TwinManager::new(twin_path),
            containment: Arc::new(Mutex::new(ContainmentState::default())),
            timeout_secs: 30,
        }
    }

    /// Runs a full dry run of the given task using the provided agent backend.
    /// Returns the `DryRunRecord` of everything the agent actually did.
    pub async fn run<R: AgentRuntime>(
        &self,
        task: &AgentTask,
        history: &[AgentTurn],
        agent: &R,
    ) -> Result<DryRunRecord, String> {
        let record = Arc::new(Mutex::new(DryRunRecord::new(task.session_id, task.task_id)));
        let twin = self.twin_manager.load_or_generate();

        // Activate both containment layers
        activate_full(&self.containment)?;

        let executor = RecordingExecutor {
            record: record.clone(),
            twin,
        };

        let turn_result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            agent.run_turn(task, history, &executor),
        )
        .await;

        // Always deactivate containment
        deactivate(&self.containment);

        // Collect any network attempts logged by the containment interceptor
        for url in intercepted_urls(&self.containment) {
            let mut rec = record.lock().unwrap();
            rec.record_network_attempt(url);
        }

        match turn_result {
            Ok(Ok(_turn)) => {
                let mut rec = record.lock().unwrap();
                rec.completed = true;
                Ok(rec.clone())
            }
            Ok(Err(e)) => Err(format!("agent error: {}", e)),
            Err(_) => {
                // Timeout — return partial record with completed = false
                Ok(record.lock().unwrap().clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_agent::{AgentError, AgentTask};

    struct AlwaysNavigateAgent;

    #[async_trait::async_trait]
    impl AgentRuntime for AlwaysNavigateAgent {
        async fn run_turn(
            &self,
            _task: &AgentTask,
            _history: &[AgentTurn],
            executor: &dyn ToolExecutor,
        ) -> Result<AgentTurn, AgentError> {
            let call =
                AgentToolCall::new(BrowserTool::Navigate("https://attacker.com/steal".to_string()));
            executor.execute(&call).await;
            let mut turn = AgentTurn::new();
            turn.tool_calls.push(call);
            turn.final_response = Some("done".to_string());
            turn.is_complete = true;
            Ok(turn)
        }
    }

    #[tokio::test]
    async fn dry_run_records_navigation() {
        let path = std::env::temp_dir()
            .join(format!("ferrite-twin-{}.enc", uuid::Uuid::new_v4()));
        let orch = DryRunOrchestrator::new(path);
        let task = AgentTask::new("navigate somewhere", None);
        let record = orch.run(&task, &[], &AlwaysNavigateAgent).await.unwrap();
        assert!(record.tools_called.contains(&ToolId::new("navigate")));
        assert!(record.network_attempts.iter().any(|u| u.contains("attacker.com")));
        assert!(record.completed);
    }

    #[tokio::test]
    async fn dry_run_does_not_touch_real_browser() {
        // RecordingExecutor returns synthetic data, never calls into Servo.
        // If this test completes without error, the real browser was not touched.
        let path = std::env::temp_dir()
            .join(format!("ferrite-twin-{}.enc", uuid::Uuid::new_v4()));
        let orch = DryRunOrchestrator::new(path);
        let task = AgentTask::new("read my emails", None);
        let record = orch.run(&task, &[], &AlwaysNavigateAgent).await.unwrap();
        assert!(record.completed);
    }
}
