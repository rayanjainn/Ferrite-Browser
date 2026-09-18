//! The dry-run event log: [`ToolEvent`], [`DryRunRecord`], and the
//! sanitizer-finding bookkeeping recorded alongside it.
//!
//! # Ordering
//!
//! `docs/REBUILD_DIRECTIVE.md` §6/A6 asks for an *ordered*
//! `Vec<ToolEvent { primitive, origin, seq }>`. Here, ordering is the
//! `Vec<ToolEvent>`'s own push order (guaranteed by
//! [`DryRunRecord::record_tool`] appending, never inserting/reordering), and
//! [`DryRunRecord::events_with_seq`] is a derived view that pairs each event
//! with its 0-based position — a computed `seq`, not a stored field. This is
//! a deliberate deviation from a literal `seq: u64` field on `ToolEvent`
//! itself: `crate::dataset` (A11's charter, off-limits to this session)
//! constructs a `ToolEvent { tool, origin }` struct literal directly in its
//! own fixture code, with no `seq`, and that file cannot be edited from this
//! session. Adding a required field would not compile there. Deriving `seq`
//! from `Vec` position instead of storing it avoids that break entirely
//! while still giving every consumer (tests, and eventually the comparator)
//! an explicit sequence number on request — see
//! `seq_is_monotonic_and_matches_call_order` below.
//!
//! # `tool: ToolId`, not `primitive: ferrite_core::Primitive`
//!
//! The directive's illustrative shape also names the primitive field
//! `primitive`. This module keeps `tool: ToolId` instead, for the same
//! reason as above: `crate::comparator` (A7's charter, off-limits to this
//! session) already pattern-matches `event.tool` typed as `ToolId` in its
//! `compare()` and in its own tests' `record_tool(ToolId::new(..), ..)`
//! calls. Renaming or retyping the field now would not compile without also
//! rewriting `comparator.rs`. [`ToolEvent::primitive`] below is the
//! forward-compatible bridge: a best-effort, tested conversion to
//! `ferrite_core::Primitive` that A7 can already call today, so its rewrite
//! of `compare()` can adopt the real type without this module changing
//! again. See `docs/handoffs/a06.md` for the exact one down-stream mismatch
//! this bridge cannot resolve on its own (`download.file` vs.
//! `Primitive::Download`'s wire string).

use std::collections::HashSet;

use crate::sanitizer::Finding;
use crate::tool_decision::ToolId;

/// Where a recorded finding came from, so the harness can match it against a
/// case's declared expected finding (pattern + optional location).
#[derive(Debug, Clone, PartialEq)]
pub enum FindingCarrier {
    /// T1a web content. `channel` is "visible_text" | "comment" | "script".
    WebContent { channel: String },
    /// T1b tool output. `json_path` is the LocatedFinding path (e.g. "results[0].description", "$", "error").
    ToolOutput { json_path: String },
}

/// A sanitizer finding recorded during the dry run, with the context needed
/// to adjudicate it against a case's declared expected finding (W2).
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedFinding {
    pub finding: Finding,
    pub carrier: FindingCarrier,
    /// The tool whose result carried this finding.
    pub tool: ToolId,
    /// The origin the agent was on when this result was produced.
    pub origin: Option<String>,
}

/// One recorded tool invocation, bound to the origin the agent was on when
/// it happened. Ordering is the containing `Vec<ToolEvent>`'s push order —
/// see [`DryRunRecord::events_with_seq`] for an explicit sequence number.
///
/// `origin` is `None` when no origin context has been established yet
/// (e.g. the very first call before any navigation/context_url).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolEvent {
    pub tool: ToolId,
    pub origin: Option<String>,
}

impl ToolEvent {
    /// Best-effort mapping to `ferrite_core::Primitive` by matching this
    /// event's `ToolId` wire string against every `Primitive::as_str()`.
    ///
    /// Returns `None` for exactly one case today: `BrowserTool::DownloadFile`
    /// produces the `ToolId` wire string `"download.file"`
    /// (`BrowserTool::tool_id()`, `ferrite-agent`), which has no exact match
    /// in `Primitive` (`Primitive::Download`'s wire string is `"download"`,
    /// no dot-suffix) — a pre-existing mismatch between `ferrite-agent`'s
    /// tool-id strings and `ferrite-core`'s taxonomy that predates this
    /// module and is out of this session's scope to fix (neither crate is in
    /// this charter's file list). Flagged for A7 in `docs/handoffs/a06.md`.
    #[must_use]
    pub fn primitive(&self) -> Option<ferrite_core::Primitive> {
        ferrite_core::Primitive::ALL
            .iter()
            .copied()
            .find(|p| p.as_str() == self.tool.0)
    }
}

/// Accumulates everything the agent did during the dry run.
#[derive(Debug, Default, Clone)]
pub struct DryRunRecord {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub tool_events: Vec<ToolEvent>,
    pub origins_touched: HashSet<String>,
    pub data_fields_accessed: HashSet<String>,
    pub network_attempts: Vec<String>,
    pub completed: bool,
    pub sanitizer_findings: Vec<RecordedFinding>,
}

impl DryRunRecord {
    pub fn new(session_id: uuid::Uuid, task_id: uuid::Uuid) -> Self {
        Self {
            session_id,
            task_id,
            ..Default::default()
        }
    }

    /// Derives the set of distinct tools called from the ordered event log.
    /// Convenience for callers that only need set membership, not ordering/origin.
    pub fn tools_called(&self) -> HashSet<ToolId> {
        self.tool_events.iter().map(|e| e.tool.clone()).collect()
    }

    /// Records one tool invocation, appended to the end of `tool_events` —
    /// the sole source of call ordering (see module docs).
    pub fn record_tool(&mut self, tool: ToolId, origin: Option<String>) {
        self.tool_events.push(ToolEvent { tool, origin });
    }

    /// Pairs each recorded event with its 0-based position in call order —
    /// the explicit `seq` the directive's `ToolEvent { primitive, origin,
    /// seq }` shape asks for, derived rather than stored (see module docs).
    pub fn events_with_seq(&self) -> impl Iterator<Item = (u64, &ToolEvent)> {
        self.tool_events
            .iter()
            .enumerate()
            .map(|(i, e)| (i as u64, e))
    }

    pub fn record_finding(&mut self, finding: RecordedFinding) {
        self.sanitizer_findings.push(finding);
    }

    pub fn record_network_attempt(&mut self, url: String) {
        let origin = extract_origin(&url);
        self.origins_touched.insert(origin);
        self.network_attempts.push(url);
    }
}

/// Normalizes a URL to its origin (`scheme://host`), falling back to the
/// literal input when it doesn't parse as `scheme://host` — this is the
/// T-211 handling for opaque-scheme URLs (`about:blank`, `data:`, `blob:`):
/// they are recorded as their literal string rather than a normalized
/// origin, since `ferrite_core::Origin::parse` structurally rejects every
/// scheme but `http`/`https` (by design — an opaque origin is one no
/// `OriginScope` could ever admit). A literal opaque string will likewise
/// fail `Origin::parse`, which is the *correct* downstream effect: the
/// comparator can treat "this event's origin string does not parse as an
/// `Origin`" as itself the signal that no scope can ever admit it — the
/// same always-a-deviation treatment `js.execute` gets, achieved by type
/// mismatch rather than a dedicated `Origin::Opaque` variant. See
/// `docs/TO-DO.md` T-211 and `docs/handoffs/a06.md` for the full reasoning
/// and why a real `Origin::Opaque` variant (in `ferrite-core`, out of this
/// session's file scope) is still the more principled long-term fix.
pub(crate) fn extract_origin(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(format!("{}://{}", u.scheme(), host))
        })
        .unwrap_or_else(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_is_monotonic_and_matches_call_order() {
        let mut rec = DryRunRecord::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        rec.record_tool(ToolId::new("navigate"), Some("https://a.example".into()));
        rec.record_tool(ToolId::new("dom.read"), Some("https://a.example".into()));
        rec.record_tool(ToolId::new("dom.read"), Some("https://a.example".into()));

        let seqs: Vec<u64> = rec.events_with_seq().map(|(seq, _)| seq).collect();
        assert_eq!(seqs, vec![0, 1, 2]);
        assert_eq!(rec.tool_events[0].tool, ToolId::new("navigate"));
        assert_eq!(rec.tool_events[1].tool, ToolId::new("dom.read"));

        // seq tracks position even after the record is cloned/filtered
        // elsewhere — it is a view over whatever Vec you call it on, not
        // state that can drift from the Vec it describes.
        let (first_seq, first_event) = rec.events_with_seq().next().unwrap();
        assert_eq!(first_seq, 0);
        assert_eq!(first_event.tool, ToolId::new("navigate"));
    }

    #[test]
    fn extract_origin_normalizes_scheme_and_host_only() {
        assert_eq!(
            extract_origin("https://Example.com:443/inbox?x=1"),
            "https://example.com"
        );
    }

    #[test]
    fn extract_origin_falls_back_to_literal_for_opaque_schemes() {
        assert_eq!(extract_origin("about:blank"), "about:blank");
        assert!(extract_origin("data:text/html,hi").starts_with("data:"));
    }

    #[test]
    fn primitive_maps_known_tool_ids_and_flags_the_one_known_mismatch() {
        assert_eq!(
            ToolEvent {
                tool: ToolId::new("navigate"),
                origin: None,
            }
            .primitive(),
            Some(ferrite_core::Primitive::Navigate)
        );
        assert_eq!(
            ToolEvent {
                tool: ToolId::new("js.execute"),
                origin: None,
            }
            .primitive(),
            Some(ferrite_core::Primitive::JsExecute)
        );
        // The one documented mismatch: BrowserTool::tool_id() for
        // DownloadFile is "download.file"; Primitive::Download's wire
        // string is "download". No exact match exists yet.
        assert_eq!(
            ToolEvent {
                tool: ToolId::new("download.file"),
                origin: None,
            }
            .primitive(),
            None
        );
    }
}
