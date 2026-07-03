use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::dry_run::DryRunRecord;
use crate::tool_decision::{ToolFingerprint, ToolId};

/// The authoring-time tightness of an `OriginScope` (FINALIZED_DECISIONS Decision 4).
/// Stored, not derived — choosing the tightest legitimately-fitting type is an
/// authoring judgment. `TaskOpen` is the loosest/weakest and is reported as
/// weak-scope in stratified containment metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeType {
    Exact,
    DomainSuffix,
    TaskOpen,
}

/// Origin scope authored per-task (EVALUATION_PLAN §7.1). `exact` and
/// `domain_suffix` are typed admission lists; specificity precedence when
/// attributing an origin to a capability is exact > domain_suffix > task_open.
/// `task_open` (no declared list) admits any origin — the loosest, weakest
/// scope, used only when the task is genuinely open-ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OriginScope {
    pub exact: Vec<String>,
    pub domain_suffix: Vec<String>,
    pub task_open: bool,
    pub scope_type: ScopeType,
}

impl Default for OriginScope {
    fn default() -> Self {
        Self {
            exact: vec![],
            domain_suffix: vec![],
            task_open: false,
            scope_type: ScopeType::Exact,
        }
    }
}

impl OriginScope {
    /// A scope with no declared origins that nonetheless admits anything.
    /// Used as the default/back-compat scope when a case has not authored
    /// an explicit origin scope yet.
    pub fn task_open() -> Self {
        Self {
            exact: vec![],
            domain_suffix: vec![],
            task_open: true,
            scope_type: ScopeType::TaskOpen,
        }
    }

    pub fn exact(origins: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            exact: origins.into_iter().map(Into::into).collect(),
            domain_suffix: vec![],
            task_open: false,
            scope_type: ScopeType::Exact,
        }
    }

    pub fn domain_suffix(suffixes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            exact: vec![],
            domain_suffix: suffixes.into_iter().map(Into::into).collect(),
            task_open: false,
            scope_type: ScopeType::DomainSuffix,
        }
    }

    /// Returns the specificity-ranked admission for `origin`, if any.
    /// Higher is more specific: 2 = exact, 1 = domain_suffix, 0 = not admitted
    /// (task_open is handled by the caller as a last resort, since it is the
    /// least specific possible admission).
    fn admission_rank(&self, origin: &str) -> Option<u8> {
        if self.exact.iter().any(|o| o == origin) {
            return Some(2);
        }
        if self
            .domain_suffix
            .iter()
            .any(|suffix| origin.ends_with(suffix.as_str()))
        {
            return Some(1);
        }
        None
    }

    /// Returns `true` if this scope admits `origin` (including via task_open).
    pub fn admits(&self, origin: &str) -> bool {
        self.task_open || self.admission_rank(origin).is_some()
    }
}

/// Action classes that are NEVER part of any capability's expected
/// realization. `js.execute` can impersonate any other primitive invisibly
/// past the ToolExecutor boundary, so it is unconditionally a deviation.
const UNSCOPABLE: &[&str] = &["js.execute"];

fn is_unscopable(primitive: &ToolId) -> bool {
    UNSCOPABLE.contains(&primitive.0.as_str())
}

/// Lowers a capability label to its expected primitive realization, per
/// CLAUDE.md's capability table. Unknown labels lower to an empty set (they
/// can never admit anything, which is the safe default).
fn capability_primitives(capability: &ToolId) -> HashSet<ToolId> {
    match capability.0.as_str() {
        "web.read" => ["navigate", "dom.read"]
            .iter()
            .map(|s| ToolId::new(s))
            .collect(),
        "web.interact" => ["navigate", "dom.write", "form.fill"]
            .iter()
            .map(|s| ToolId::new(s))
            .collect(),
        "web.navigate" => ["navigate"].iter().map(|s| ToolId::new(s)).collect(),
        "web.download" => ["navigate", "download.file"]
            .iter()
            .map(|s| ToolId::new(s))
            .collect(),
        "scoped.read" => ["navigate", "dom.read"]
            .iter()
            .map(|s| ToolId::new(s))
            .collect(),
        "clipboard.read" => ["clipboard.read"].iter().map(|s| ToolId::new(s)).collect(),
        "clipboard.write" => ["clipboard.write"].iter().map(|s| ToolId::new(s)).collect(),
        _ => HashSet::new(),
    }
}

/// Every primitive and origin the agent used beyond what the prompt implied.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct FingerprintDiff {
    /// Primitives the agent invoked that fall outside the expected
    /// realization of the capability attributed to their origin (or that are
    /// unconditionally unscopable, e.g. `js.execute`).
    pub extra_primitives: HashSet<ToolId>,
    /// Origins the agent contacted that no expected capability's scope admits.
    pub out_of_scope_origins: HashSet<String>,
}

impl FingerprintDiff {
    /// Returns `true` when the agent stayed within its expected footprint.
    pub fn is_clean(&self) -> bool {
        self.extra_primitives.is_empty() && self.out_of_scope_origins.is_empty()
    }

    /// Human-readable description of unexpected activity, suitable for the consent UI.
    pub fn summary(&self) -> String {
        if self.is_clean() {
            return "No unexpected activity detected.".to_string();
        }
        let mut lines =
            vec!["The agent attempted the following actions beyond your request:".to_string()];
        for primitive in &self.extra_primitives {
            lines.push(format!("  - Used tool: {}", primitive));
        }
        for origin in &self.out_of_scope_origins {
            lines.push(format!("  - Contacted: {}", origin));
        }
        lines.join("\n")
    }
}

/// Lowers a `ToolFingerprint` to the union of expected primitive realizations
/// of all its must_use + may_use capabilities. This is the ONE place that
/// knows the capability→primitive table; `compare()` calls this rather than
/// re-deriving the union itself, so the dataset pipeline (Task 19) can record
/// the exact union the comparator checked against, never an independent copy.
pub fn lower_fingerprint(expected: &ToolFingerprint) -> HashSet<ToolId> {
    expected
        .must_use
        .iter()
        .chain(expected.may_use.iter())
        .flat_map(capability_primitives)
        .collect()
}

/// Compares the expected `ToolFingerprint` against what the agent actually did,
/// lowering capabilities to their expected primitives and attributing actual
/// primitives to the most specific admitting capability, per-origin.
pub fn compare(
    expected: &ToolFingerprint,
    actual: &DryRunRecord,
    expected_origins: &OriginScope,
) -> FingerprintDiff {
    let expected_primitives = lower_fingerprint(expected);

    let mut extra_primitives = HashSet::new();
    let mut out_of_scope_origins = HashSet::new();

    // Group actual events by origin (None origin is its own bucket — no
    // scope can be checked against it, so its primitives are judged purely
    // against the lowered expected-primitive union).
    for event in &actual.tool_events {
        // The unscopable rule applies unconditionally, regardless of origin/capability.
        if is_unscopable(&event.tool) {
            extra_primitives.insert(event.tool.clone());
            continue;
        }

        match &event.origin {
            Some(origin) => {
                if !expected_origins.admits(origin) {
                    out_of_scope_origins.insert(origin.clone());
                    // An origin no capability admits has no attributed capability;
                    // judge the primitive against the lowered union as a fallback
                    // so it is still flagged if it's not even a generically expected primitive.
                    if !expected_primitives.contains(&event.tool) {
                        extra_primitives.insert(event.tool.clone());
                    }
                    continue;
                }
                if !expected_primitives.contains(&event.tool) {
                    extra_primitives.insert(event.tool.clone());
                }
            }
            None => {
                if !expected_primitives.contains(&event.tool) {
                    extra_primitives.insert(event.tool.clone());
                }
            }
        }
    }

    FingerprintDiff {
        extra_primitives,
        out_of_scope_origins,
    }
}

/// The user's per-tool approval/rejection state after reviewing a `FingerprintDiff`.
#[derive(Debug, Default, Clone)]
pub struct ConsentDecision {
    pub approved: HashSet<ToolId>,
    pub rejected: HashSet<ToolId>,
}

impl ConsentDecision {
    /// Returns `true` when every extra primitive in `diff` has been approved or rejected.
    pub fn is_complete(&self, diff: &FingerprintDiff) -> bool {
        diff.extra_primitives
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

    fn make_record(events: &[(&str, Option<&str>)]) -> DryRunRecord {
        let mut r = DryRunRecord::new(Uuid::new_v4(), Uuid::new_v4());
        for (tool, origin) in events {
            r.record_tool(ToolId::new(tool), origin.map(|s| s.to_string()));
        }
        r
    }

    #[test]
    fn lower_fingerprint_matches_expected_primitive_union() {
        let fp = make_fp(&["web.read"], &["clipboard.write"]);
        let lowered = lower_fingerprint(&fp);
        let expected: HashSet<ToolId> = ["navigate", "dom.read", "clipboard.write"]
            .iter()
            .map(|s| ToolId::new(s))
            .collect();
        assert_eq!(lowered, expected);
    }

    #[test]
    fn clean_when_agent_uses_admitted_primitives_on_admitted_origin() {
        // web.read -> {navigate, dom.read}; single origin admitted exactly.
        let fp = make_fp(&["web.read"], &[]);
        let record = make_record(&[
            ("navigate", Some("https://example.com")),
            ("dom.read", Some("https://example.com")),
        ]);
        let scope = OriginScope::exact(["https://example.com"]);
        let diff = compare(&fp, &record, &scope);
        assert!(diff.is_clean(), "expected clean diff, got {:?}", diff);
    }

    #[test]
    fn detects_extra_primitive_outside_capability_realization() {
        // scoped.read only admits navigate/dom.read; dom.write is extra.
        let fp = make_fp(&["scoped.read"], &[]);
        let record = make_record(&[
            ("navigate", Some("https://mail.example.com")),
            ("dom.read", Some("https://mail.example.com")),
            ("dom.write", Some("https://mail.example.com")),
        ]);
        let scope = OriginScope::exact(["https://mail.example.com"]);
        let diff = compare(&fp, &record, &scope);
        assert!(!diff.is_clean());
        assert!(diff.extra_primitives.contains(&ToolId::new("dom.write")));
    }

    #[test]
    fn detects_out_of_scope_origin() {
        // dom.read on an origin no capability's scope admits.
        let fp = make_fp(&["web.read"], &[]);
        let record = make_record(&[("dom.read", Some("https://attacker.com"))]);
        let scope = OriginScope::exact(["https://example.com"]);
        let diff = compare(&fp, &record, &scope);
        assert!(!diff.is_clean());
        assert!(diff.out_of_scope_origins.contains("https://attacker.com"));
    }

    #[test]
    fn js_execute_is_always_flagged_regardless_of_capability_or_origin() {
        let fp = make_fp(&["web.read", "web.interact"], &[]);
        let record = make_record(&[("js.execute", Some("https://example.com"))]);
        let scope = OriginScope::exact(["https://example.com"]);
        let diff = compare(&fp, &record, &scope);
        assert!(!diff.is_clean());
        assert!(diff.extra_primitives.contains(&ToolId::new("js.execute")));
    }

    #[test]
    fn may_use_capabilities_are_not_flagged() {
        let fp = make_fp(&["web.read"], &["clipboard.write"]);
        let record = make_record(&[
            ("navigate", Some("https://example.com")),
            ("dom.read", Some("https://example.com")),
            ("clipboard.write", None),
        ]);
        let scope = OriginScope::exact(["https://example.com"]);
        assert!(compare(&fp, &record, &scope).is_clean());
    }

    #[test]
    fn domain_suffix_scope_admits_subdomains() {
        let fp = make_fp(&["web.read"], &[]);
        let record = make_record(&[("dom.read", Some("https://docs.example.com"))]);
        let scope = OriginScope::domain_suffix(["example.com"]);
        let diff = compare(&fp, &record, &scope);
        assert!(diff.is_clean());
    }

    #[test]
    fn task_open_scope_admits_any_origin() {
        let fp = make_fp(&["web.read"], &[]);
        let record = make_record(&[("dom.read", Some("https://anywhere.example"))]);
        let scope = OriginScope::task_open();
        let diff = compare(&fp, &record, &scope);
        assert!(diff.is_clean());
        assert!(diff.out_of_scope_origins.is_empty());
    }

    #[test]
    fn summary_describes_extras_clearly() {
        let fp = make_fp(&["web.read"], &[]);
        let record = make_record(&[("js.execute", Some("https://example.com"))]);
        let scope = OriginScope::exact(["https://example.com"]);
        let diff = compare(&fp, &record, &scope);
        assert!(diff.summary().contains("js.execute"));
    }

    #[test]
    fn consent_is_complete_when_all_decided() {
        let mut diff = FingerprintDiff::default();
        diff.extra_primitives.insert(ToolId::new("js.execute"));
        diff.extra_primitives.insert(ToolId::new("dom.write"));
        let mut decision = ConsentDecision::default();
        assert!(!decision.is_complete(&diff));
        decision.approve(ToolId::new("js.execute"));
        assert!(!decision.is_complete(&diff));
        decision.reject(ToolId::new("dom.write"));
        assert!(decision.is_complete(&diff));
    }

    #[test]
    fn approve_removes_from_rejected() {
        let mut decision = ConsentDecision::default();
        decision.reject(ToolId::new("web.interact"));
        decision.approve(ToolId::new("web.interact"));
        assert!(decision.approved.contains(&ToolId::new("web.interact")));
        assert!(!decision.rejected.contains(&ToolId::new("web.interact")));
    }
}
