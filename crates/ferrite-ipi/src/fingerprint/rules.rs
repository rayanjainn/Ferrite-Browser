//! The deterministic rule layer: keyword → `must_use` (directive §6/A4).
//!
//! Retyped from the pre-rebuild `tool_decision::rule_based_must_use`
//! (`crates/ferrite-ipi/src/tool_decision/mod.rs`, kept in place as
//! reference material per `docs/AUDIT.md` — not called from here) against
//! the closed [`Capability`] enum instead of a stringly `ToolId`. Same
//! keyword groups, same case-insensitive substring match; the only change is
//! that the output type makes an out-of-vocabulary result unrepresentable
//! instead of merely absent from a hand-checked list.

use std::collections::BTreeSet;

use ferrite_core::Capability;

/// One keyword group and the capability it pins down when any phrase in it
/// appears in the (lowercased) prompt.
///
/// A table rather than a chain of `if` statements: the mapping is legible as
/// data, and a new keyword is one array entry rather than a new branch to
/// review for correctness.
const RULES: &[(Capability, &[&str])] = &[
    (
        Capability::ScopedRead,
        &[
            "email", "inbox", "mail", "calendar", "schedule", "meeting", "contacts",
        ],
    ),
    (
        Capability::WebInteract,
        &[
            "send email",
            "reply to",
            "forward",
            "draft",
            "book",
            "create event",
            "add meeting",
            "fill",
            "form",
            "type in",
            "submit",
            "click submit",
        ],
    ),
    (Capability::WebNavigate, &["go to", "navigate to", "open"]),
    (
        Capability::WebRead,
        &[
            "read",
            "extract",
            "find on page",
            "what does",
            "title of",
            "report",
            "summarise",
            "summarize",
        ],
    ),
    (Capability::WebDownload, &["download"]),
];

/// The deterministic keyword layer: capabilities directly and unambiguously
/// implied by the prompt text alone, computed offline with no model call.
///
/// Case-insensitive substring match against [`RULES`]. A prompt matching no
/// group yields the empty set — an open-ended prompt is a legitimate input,
/// not a failure; the model layer (`engine::generate_fingerprint`) may still
/// propose `may_use` capabilities for it.
///
/// `js.execute` can never appear in the result: [`Capability`] has no
/// variant belonging to [`ferrite_core::ActionClass::Execute`], so there is
/// no value this function could return that names it (see the
/// [module docs](crate::fingerprint) for the full argument).
#[must_use]
pub fn rule_based_must_use(prompt: &str) -> BTreeSet<Capability> {
    let lower = prompt.to_lowercase();
    RULES
        .iter()
        .filter(|(_, keywords)| keywords.iter().any(|kw| lower.contains(kw)))
        .map(|(capability, _)| *capability)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_prompt_gives_scoped_read() {
        let caps = rule_based_must_use("Check my inbox and summarise new emails");
        assert!(caps.contains(&Capability::ScopedRead));
    }

    #[test]
    fn navigate_and_read_prompt_gives_both_capabilities() {
        let caps = rule_based_must_use("Please open https://example.com and read the article");
        assert!(caps.contains(&Capability::WebNavigate));
        assert!(caps.contains(&Capability::WebRead));
    }

    #[test]
    fn download_and_report_prompt_gives_download_and_read() {
        let caps = rule_based_must_use("Download the quarterly report");
        assert!(caps.contains(&Capability::WebDownload));
        assert!(
            caps.contains(&Capability::WebRead),
            "\"report\" matches the read group"
        );
    }

    #[test]
    fn form_prompt_gives_web_interact() {
        let caps = rule_based_must_use("Fill out the signup form and submit it");
        assert_eq!(caps, BTreeSet::from([Capability::WebInteract]));
    }

    #[test]
    fn open_ended_prompt_returns_empty() {
        let caps = rule_based_must_use("What's the capital of France?");
        assert!(caps.is_empty());
    }

    #[test]
    fn no_false_positive_on_unrelated_prompt() {
        let caps = rule_based_must_use("Tell me a joke");
        assert!(!caps.contains(&Capability::ScopedRead));
        assert!(!caps.contains(&Capability::WebInteract));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(
            rule_based_must_use("CHECK MY INBOX"),
            rule_based_must_use("check my inbox")
        );
    }

    #[test]
    fn rule_layer_never_emits_a_capability_outside_the_closed_seven() {
        // Trivially true by the return type (`BTreeSet<Capability>` cannot
        // hold anything else), but stated as a test so the property is
        // checked against every rule-table entry rather than only trusted
        // by inspection: every capability named in `RULES` must be a real
        // `Capability::ALL` member.
        for (capability, _) in RULES {
            assert!(Capability::ALL.contains(capability));
        }
    }

    #[test]
    fn rule_layer_can_never_pin_down_js_execute() {
        // There is no `Capability` variant for the `Execute` action class,
        // so this is unreachable by construction; asserted here as the
        // rule layer's share of the module-wide js.execute guarantee.
        for prompt in [
            "run some javascript on this page",
            "execute js: alert(1)",
            "eval this script",
        ] {
            for capability in rule_based_must_use(prompt) {
                assert_ne!(
                    capability.action_class(),
                    ferrite_core::ActionClass::Execute
                );
            }
        }
    }
}
