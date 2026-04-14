use std::collections::HashSet;

use crate::dry_run::DryRunRecord;
use crate::tool_decision::{ToolFingerprint, ToolId};

/// Every tool and origin the agent used beyond what the prompt implied.
#[derive(Debug, Default, Clone)]
pub struct FingerprintDiff {
    /// Tools the agent called that were not in `must_use` or `may_use`.
    pub extra_tools: HashSet<ToolId>,
    /// Origins the agent contacted that were not implied by the task.
    pub extra_origins: HashSet<String>,
}

impl FingerprintDiff {
    /// Returns `true` when the agent stayed within its expected footprint.
    pub fn is_clean(&self) -> bool {
        self.extra_tools.is_empty() && self.extra_origins.is_empty()
    }

    /// Human-readable description of unexpected activity, suitable for the consent UI.
    pub fn summary(&self) -> String {
        if self.is_clean() {
            return "No unexpected activity detected.".to_string();
        }
        let mut lines =
            vec!["The agent attempted the following actions beyond your request:".to_string()];
        for tool in &self.extra_tools {
            lines.push(format!("  - Used tool: {}", tool));
        }
        for origin in &self.extra_origins {
            lines.push(format!("  - Contacted: {}", origin));
        }
        lines.join("\n")
    }
}

/// Compares the expected `ToolFingerprint` against what the agent actually did.
pub fn compare(expected: &ToolFingerprint, actual: &DryRunRecord) -> FingerprintDiff {
    let extra_tools = actual
        .tools_called
        .iter()
        .filter(|tool| !expected.contains(tool))
        .cloned()
        .collect();

    // Flag external origins only when network.fetch is not in the expected set.
    let extra_origins = if expected.contains(&ToolId::new("network.fetch")) {
        HashSet::new()
    } else {
        actual.origins_touched.clone()
    };

    FingerprintDiff { extra_tools, extra_origins }
}

/// The user's per-tool approval/rejection state after reviewing a `FingerprintDiff`.
#[derive(Debug, Default, Clone)]
pub struct ConsentDecision {
    pub approved: HashSet<ToolId>,
    pub rejected: HashSet<ToolId>,
}

impl ConsentDecision {
    /// Returns `true` when every extra tool in `diff` has been approved or rejected.
    pub fn is_complete(&self, diff: &FingerprintDiff) -> bool {
        diff.extra_tools
            .iter()
            .all(|t| self.approved.contains(t) || self.rejected.contains(t))
    }

    pub fn approve(&mut self, tool: ToolId) {
        self.rejected.remove(&tool);
        self.approved.insert(tool);
    }

    pub fn reject(&mut self, tool: ToolId) {
        self.approved.remove(&tool);
        self.rejected.insert(tool);
    }
}

/// A labelled event for the IPI dataset pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpiEvent {
    pub event_id: uuid::Uuid,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub origin: String,
    pub payload_hash: String,
    pub extra_tools: Vec<String>,
    pub task_context_hash: String,
    pub label: IpiLabel,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum IpiLabel {
    TruePositive,
    FalsePositive,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_fp(must: &[&str], may: &[&str]) -> ToolFingerprint {
        ToolFingerprint {
            session_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            must_use: must.iter().map(|s| ToolId::new(s)).collect(),
            may_use: may.iter().map(|s| ToolId::new(s)).collect(),
        }
    }

    fn make_record(tools: &[&str]) -> DryRunRecord {
        let mut r = DryRunRecord::new(Uuid::new_v4(), Uuid::new_v4());
        for t in tools {
            r.record_tool(ToolId::new(t));
        }
        r
    }

    #[test]
    fn clean_when_agent_uses_expected_tools_only() {
        let fp = make_fp(&["email.read"], &["report.write"]);
        let record = make_record(&["email.read", "report.write"]);
        assert!(compare(&fp, &record).is_clean());
    }

    #[test]
    fn detects_extra_tool() {
        let fp = make_fp(&["email.read"], &[]);
        let record = make_record(&["email.read", "passwords.read"]);
        let diff = compare(&fp, &record);
        assert!(!diff.is_clean());
        assert!(diff.extra_tools.contains(&ToolId::new("passwords.read")));
    }

    #[test]
    fn may_use_tools_are_not_flagged() {
        let fp = make_fp(&["email.read"], &["email.send"]);
        let record = make_record(&["email.read", "email.send"]);
        assert!(compare(&fp, &record).is_clean());
    }

    #[test]
    fn summary_describes_extras_clearly() {
        let fp = make_fp(&["email.read"], &[]);
        let record = make_record(&["email.read", "passwords.read"]);
        let diff = compare(&fp, &record);
        assert!(diff.summary().contains("passwords.read"));
    }

    #[test]
    fn consent_is_complete_when_all_decided() {
        let mut diff = FingerprintDiff::default();
        diff.extra_tools.insert(ToolId::new("passwords.read"));
        diff.extra_tools.insert(ToolId::new("network.fetch"));
        let mut decision = ConsentDecision::default();
        assert!(!decision.is_complete(&diff));
        decision.approve(ToolId::new("passwords.read"));
        assert!(!decision.is_complete(&diff));
        decision.reject(ToolId::new("network.fetch"));
        assert!(decision.is_complete(&diff));
    }

    #[test]
    fn approve_removes_from_rejected() {
        let mut decision = ConsentDecision::default();
        decision.reject(ToolId::new("email.send"));
        decision.approve(ToolId::new("email.send"));
        assert!(decision.approved.contains(&ToolId::new("email.send")));
        assert!(!decision.rejected.contains(&ToolId::new("email.send")));
    }
}
