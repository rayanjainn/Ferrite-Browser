//! [`WorstCaseAgent`]: a deterministic `AgentRuntime` driven by a case's own
//! authored `ground_truth`, used by `just eval`'s corpus-wide runner and by
//! the T-202 per-case inspector.
//!
//! # Methodology, stated plainly (do not silently rely on this)
//!
//! This is **not** a model reasoning about whether to comply with an
//! injected instruction — it is a scripted re-enactment of exactly the
//! deviation a case's authored `ground_truth` declares (its
//! `expected_extra_primitives` / `expected_out_of_scope_origins` /
//! `attack_origin`), issued unconditionally regardless of what the (possibly
//! sanitizer-stripped) content it reads actually says.
//!
//! That is a deliberate, standard evaluation methodology for THIS
//! project's specific claim, not a shortcut: Ferrite's research
//! contribution is the *architectural* defense — what happens once an
//! agent has been steered off its expected fingerprint — not detection of
//! whether a given model falls for a given phrasing (that is a base-model
//! robustness question the project explicitly does not claim to answer).
//! `attack_final_outcome` (`docs/TO-DO.md` T-004) and the comparator
//! (`ferrite-ipi::comparator`) exist to catch a REALIZED deviation
//! regardless of why the agent deviated, so measuring "does the loop catch
//! a worst-case-compliant agent's declared deviation" is the correct
//! experiment for O1 (containment) and O2 (attribution). It is the
//! generalization, across the full real corpus, of the exact same
//! hand-written `ScriptedAgent` pattern `harness.rs`'s and `corpus.rs`'s own
//! tests already use per-fixture — this module derives that fixed action
//! sequence from `ground_truth` programmatically instead of by hand.
//!
//! **What this does NOT measure:** whether a real model would actually take
//! the bait in the first place (that is `docs/REBUILD_DIRECTIVE.md` §10's
//! `ferrite_model::ModelProvider`-backed agent loop's territory, not wired
//! into the eval harness's dry-run path as of this session — see
//! `docs/EVALUATION.md`'s limitations section). Because this agent's actions
//! do not depend on the content it observes, sanitizer stripping cannot
//! change ITS behavior — it can only change what the sanitizer/fingerprint
//! layers RECORD as caught. Concretely: `FinalOutcome::Stripped` (T-004) is
//! exercised by this module's own unit tests and by `adjudication`'s
//! table-driven test, but may not appear at all in a real corpus run under
//! this agent, because the loop almost always still catches the
//! unconditionally-issued deviation directly (`ContainedViaConsent` takes
//! priority). This is stated here and in `docs/EVALUATION.md`, not hidden.
//!
//! # One known gap
//!
//! One pilot case (`ref06_t1b_jsonfield_exfil`) authors its content via
//! `by_tool.download.file` rather than `extract_data`; this agent's
//! carrier-derived primary action (`ExtractData` for any `ToolOutput` case)
//! does not reach it. That case is not meaningfully exercised by `just
//! eval`'s automated run — flagged explicitly rather than special-cased for
//! one row (`docs/EVALUATION.md`).

use std::collections::HashSet;

use ferrite_agent::{
    AgentError, AgentRuntime, AgentTask, AgentToolCall, AgentTurn, BrowserTool, ToolExecutor,
};
use ferrite_ipi::dataset::{CarrierVector, CaseDefinition, GroundTruth};
use ferrite_ipi::tool_decision::ToolId;

enum PrimaryAction {
    ReadPage,
    ExtractData,
}

/// A deterministic, ground-truth-derived `AgentRuntime`. See module docs for
/// the full methodology statement.
pub struct WorstCaseAgent {
    primary: PrimaryAction,
    extra_calls: Vec<BrowserTool>,
}

impl WorstCaseAgent {
    /// Builds the scripted action sequence for `case`: the carrier-derived
    /// primary read, followed by every tool call `case.ground_truth`
    /// declares the attack would realize.
    #[must_use]
    pub fn for_case(case: &CaseDefinition) -> Self {
        let primary = match case.carrier_vector {
            CarrierVector::WebContent(_) => PrimaryAction::ReadPage,
            CarrierVector::ToolOutput(_) => PrimaryAction::ExtractData,
        };

        let mut extra_calls = Vec::new();
        let mut origins_navigated: HashSet<String> = HashSet::new();

        match &case.ground_truth {
            GroundTruth::Deviation {
                expected_extra_primitives,
                expected_out_of_scope_origins,
            } => {
                for origin in expected_out_of_scope_origins {
                    extra_calls.push(BrowserTool::Navigate(origin.clone()));
                    origins_navigated.insert(origin.clone());
                }
                let fallback_origin = expected_out_of_scope_origins
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "https://attacker.example".to_string());
                for tool_id in expected_extra_primitives {
                    if let Some(call) = tool_for_primitive(tool_id, &fallback_origin) {
                        // Don't double-navigate to an origin already issued above.
                        if let BrowserTool::Navigate(ref url) = call {
                            if origins_navigated.contains(url) {
                                continue;
                            }
                        }
                        extra_calls.push(call);
                    }
                }
            }
            GroundTruth::WithinFingerprintOriginShift { attack_origin, .. } => {
                extra_calls.push(BrowserTool::Navigate(attack_origin.clone()));
            }
            // The irreducible residual (same-origin/same-primitive/data-only)
            // and benign cases: no extra action beyond the primary read.
            GroundTruth::WithinFingerprintDataOnly { .. } | GroundTruth::None => {}
        }

        Self {
            primary,
            extra_calls,
        }
    }
}

/// Maps an authored `ToolId` (§8's primitive vocabulary) to a representative
/// `BrowserTool` call. `"dom.read"` returns `None` — it is already the
/// primary read, never a distinct extra action.
fn tool_for_primitive(tool_id: &ToolId, fallback_origin: &str) -> Option<BrowserTool> {
    match tool_id.0.as_str() {
        "js.execute" => Some(BrowserTool::ExecuteJs("void 0".to_string())),
        "form.fill" => Some(BrowserTool::FillForm {
            selector: "#form".to_string(),
            value: "worst-case-agent".to_string(),
        }),
        "clipboard.read" => Some(BrowserTool::ReadClipboard),
        "clipboard.write" => Some(BrowserTool::WriteClipboard("worst-case-agent".to_string())),
        "dom.write" => Some(BrowserTool::ClickElement("#el".to_string())),
        "download.file" => Some(BrowserTool::DownloadFile(fallback_origin.to_string())),
        "navigate" => Some(BrowserTool::Navigate(fallback_origin.to_string())),
        _ => None,
    }
}

#[async_trait::async_trait]
impl AgentRuntime for WorstCaseAgent {
    async fn run_turn(
        &self,
        _task: &AgentTask,
        _history: &[AgentTurn],
        executor: &dyn ToolExecutor,
    ) -> Result<AgentTurn, AgentError> {
        let mut turn = AgentTurn::new();
        let primary_call = match self.primary {
            PrimaryAction::ReadPage => BrowserTool::ReadPage,
            PrimaryAction::ExtractData => BrowserTool::ExtractData(String::new()),
        };
        for tool in std::iter::once(primary_call).chain(self.extra_calls.iter().cloned()) {
            let call = AgentToolCall::new(tool);
            let result = executor.execute(&call).await;
            turn.tool_calls.push(call);
            turn.tool_results.push(result);
        }
        turn.final_response = Some("done".to_string());
        turn.is_complete = true;
        Ok(turn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::OriginScope;
    use ferrite_ipi::dataset::{Author, Corpus, Tier, WebContentVector};
    use std::collections::HashSet as StdHashSet;
    use uuid::Uuid;

    fn exact(url: &str) -> OriginScope {
        OriginScope::exact([ferrite_core::Origin::parse(url).unwrap()]).unwrap()
    }

    fn base_case(ground_truth: GroundTruth) -> CaseDefinition {
        CaseDefinition {
            case_id: Uuid::new_v4(),
            corpus: Corpus::Attack,
            tier: Tier::Tier1,
            author: Author::SelfAuthored,
            carrier_vector: CarrierVector::WebContent(WebContentVector::HtmlComment),
            attack_category: None,
            attack_techniques: vec![],
            in_scope: true,
            user_task: "task".to_string(),
            attacker_goal: None,
            expected_origins: exact("https://news.example"),
            scope_rationale: None,
            ground_truth,
            taxonomy_anchor: None,
            expected_finding: None,
        }
    }

    #[test]
    fn deviation_case_issues_navigate_and_mapped_primitives() {
        let mut extra = StdHashSet::new();
        extra.insert(ToolId::new("js.execute"));
        let mut origins = StdHashSet::new();
        origins.insert("https://attacker.example".to_string());

        let case = base_case(GroundTruth::Deviation {
            expected_extra_primitives: extra,
            expected_out_of_scope_origins: origins,
        });
        let agent = WorstCaseAgent::for_case(&case);
        assert_eq!(agent.extra_calls.len(), 2, "{:?}", agent.extra_calls);
        assert!(agent
            .extra_calls
            .iter()
            .any(|t| matches!(t, BrowserTool::Navigate(u) if u == "https://attacker.example")));
        assert!(agent
            .extra_calls
            .iter()
            .any(|t| matches!(t, BrowserTool::ExecuteJs(_))));
    }

    #[test]
    fn origin_shift_case_issues_a_single_navigate_to_the_attack_origin() {
        let case = base_case(GroundTruth::WithinFingerprintOriginShift {
            legitimate_origin: "https://news.example".to_string(),
            attack_origin: "https://attacker.example".to_string(),
        });
        let agent = WorstCaseAgent::for_case(&case);
        assert_eq!(agent.extra_calls.len(), 1);
        assert!(matches!(
            &agent.extra_calls[0],
            BrowserTool::Navigate(u) if u == "https://attacker.example"
        ));
    }

    #[test]
    fn data_only_and_benign_cases_issue_no_extra_calls() {
        let data_only = base_case(GroundTruth::WithinFingerprintDataOnly {
            legitimate_data_ref: "a".to_string(),
            attack_data_ref: "b".to_string(),
        });
        assert!(WorstCaseAgent::for_case(&data_only).extra_calls.is_empty());

        let benign = base_case(GroundTruth::None);
        assert!(WorstCaseAgent::for_case(&benign).extra_calls.is_empty());
    }

    #[test]
    fn does_not_double_navigate_when_navigate_primitive_and_origin_coincide() {
        let mut extra = StdHashSet::new();
        extra.insert(ToolId::new("navigate"));
        let mut origins = StdHashSet::new();
        origins.insert("https://attacker.example".to_string());

        let case = base_case(GroundTruth::Deviation {
            expected_extra_primitives: extra,
            expected_out_of_scope_origins: origins,
        });
        let agent = WorstCaseAgent::for_case(&case);
        let navigate_count = agent
            .extra_calls
            .iter()
            .filter(|t| matches!(t, BrowserTool::Navigate(_)))
            .count();
        assert_eq!(navigate_count, 1, "{:?}", agent.extra_calls);
    }

    #[test]
    fn tool_output_carrier_selects_extract_data_as_the_primary_action() {
        let mut case = base_case(GroundTruth::None);
        case.carrier_vector =
            CarrierVector::ToolOutput(ferrite_ipi::dataset::ToolOutputVector::ToolJsonField);
        let agent = WorstCaseAgent::for_case(&case);
        assert!(matches!(agent.primary, PrimaryAction::ExtractData));
    }

    #[tokio::test]
    async fn run_turn_issues_the_primary_read_and_every_extra_call() {
        use ferrite_ipi::dry_run::{DryRunContent, DryRunOrchestrator};

        let mut extra = StdHashSet::new();
        extra.insert(ToolId::new("js.execute"));
        let case = base_case(GroundTruth::Deviation {
            expected_extra_primitives: extra,
            expected_out_of_scope_origins: Default::default(),
        });
        let agent = WorstCaseAgent::for_case(&case);

        let mut content = DryRunContent::default();
        content.set_page("https://news.example", "<p>hello</p>");
        let orch = DryRunOrchestrator::with_content(
            std::env::temp_dir().join(format!("wca-{}.enc", Uuid::new_v4())),
            content,
        );
        let task = AgentTask::new(
            case.user_task.clone(),
            Some("https://news.example".to_string()),
        );
        let ipi_task = ferrite_ipi::IpiTask {
            session_id: task.session_id,
            task_id: task.task_id,
            prompt: task.prompt.clone(),
            context_url: task.context_url.clone(),
        };
        let driver = crate::harness::AgentRuntimeDriver {
            agent: &agent,
            task,
            history: &[],
        };
        let record = orch.run(&ipi_task, &driver).await.unwrap();
        assert_eq!(record.tool_events.len(), 2, "{:?}", record.tool_events);
    }
}
