//! Compatibility shim: the pre-rebuild flat capability → primitive lowering.
//!
//! [`compare`](super::compare) does **not** use anything in this file — it
//! lowers through [`ferrite_core::ExpectedCapabilitySet::lowered`] instead,
//! which is per-capability-scoped (see the parent module's docs). This file
//! exists only because `crate::dataset::ExpectedRealization` (A11's schema,
//! off-limits to this session) records `expected_primitives:
//! HashSet<ToolId>` — the exact flat union [`lower_fingerprint`] produces —
//! and both `ferrite_eval::harness::run_one` and this crate's own
//! `dataset.rs` tests still call it to populate that field against the
//! legacy, stringly [`crate::tool_decision::ToolFingerprint`]. Deleting this
//! would not be a "minimal mechanical" compile fix for those callers; it
//! would force a schema change on `dataset.rs`, which is out of this
//! session's charter (T-107 touches `comparator/` only).
//!
//! Do not extend this table for a new capability or primitive — that
//! belongs in [`ferrite_core::taxonomy::LOWERING`], the single typed source
//! of truth `compare` actually checks against.

use std::collections::HashSet;

use crate::tool_decision::{ToolFingerprint, ToolId};

/// Lowers a legacy string capability label to its expected primitive
/// realization, per the pre-rebuild flat table (CLAUDE.md's old capability
/// table, carried forward verbatim). Unknown labels lower to an empty set —
/// they can never admit anything, which is the safe default.
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

/// Lowers a legacy [`ToolFingerprint`] to the flat union of expected
/// primitive realizations of all its `must_use` + `may_use` capabilities.
///
/// Recorded verbatim (never re-derived independently) by
/// `dataset::ExpectedRealization::expected_primitives` for reproducibility.
/// **Not** used by [`super::compare`] — see the module docs for why both
/// this flat, unscoped union and the real per-capability-scoped lowering
/// still exist side by side.
#[must_use]
pub fn lower_fingerprint(expected: &ToolFingerprint) -> HashSet<ToolId> {
    expected
        .must_use
        .iter()
        .chain(expected.may_use.iter())
        .flat_map(capability_primitives)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fp(must: &[&str], may: &[&str]) -> ToolFingerprint {
        ToolFingerprint {
            session_id: uuid::Uuid::new_v4(),
            task_id: uuid::Uuid::new_v4(),
            must_use: must.iter().map(|s| ToolId::new(s)).collect(),
            may_use: may.iter().map(|s| ToolId::new(s)).collect(),
        }
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
    fn unknown_capability_label_lowers_to_nothing() {
        let fp = make_fp(&["phantom.capability"], &[]);
        assert!(lower_fingerprint(&fp).is_empty());
    }
}
