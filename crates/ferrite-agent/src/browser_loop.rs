//! The engine-agnostic, provider-agnostic agent loop
//! (`docs/REBUILD_DIRECTIVE.md` §6/A9, T-109): plan → select tool → act →
//! observe → repeat, over `ferrite_engine::BrowserEngine` and
//! `ferrite_model::ModelProvider`.
//!
//! # Why this is new code, not a rebuild of `BrowserTool`/`AgentRuntime`
//!
//! `ferrite-agent`'s existing `BrowserTool` enum, `AgentRuntime`/
//! `ToolExecutor` traits and `GeminiAgent` (this crate's `lib.rs`/
//! `gemini.rs`) are **not** touched or migrated by this module. They are
//! live, load-bearing infrastructure for the dry-run/eval/consent path
//! built across A5–A8 and consumed by `ferrite-ipi::dry_run`,
//! `ferrite-eval::harness`/`corpus`, and `ferrite-ui` — dozens of call
//! sites, none of which this charter's scope
//! (`ferrite-engine`/`ferrite-engine-servo`/`ferrite-agent`'s *new* loop)
//! includes rewriting. `GeminiAgent` is also still the live agent runtime
//! `ferrite-shell`/`ferrite-ui` actually run. Migrating that whole path
//! onto `BrowserEngine` is real, larger work belonging to whichever later
//! agent owns wiring the full defense loop into production (the directive's
//! own A9 charter text names this explicitly as out of scope here). See
//! `docs/handoffs/a09.md`.
//!
//! What *is* new here is a second, additive agent loop built directly on
//! this charter's two other deliverables: [`ferrite_engine::BrowserEngine`]
//! (engine-agnostic — generic over any implementation, `MockEngine` or
//! `ServoEngine`) and `ferrite_model::ModelProvider` (provider-agnostic —
//! `&dyn ModelProvider`, never a concrete backend, matching A4's
//! `fingerprint::generate_fingerprint` precedent).
//!
//! # Scope note: no IPI wiring here
//!
//! This loop does **not** call `ferrite-ipi`'s fingerprint/dry-run/compare/
//! consent machinery. Doing so for real is not a small, obvious integration
//! (see the module docs above on why the existing path is a separate,
//! parallel, already-large system) — the directive's own A9 text
//! anticipates this and scopes it out explicitly, naming it as later
//! integration work.

use std::time::Duration;

use ferrite_core::Clock;
use ferrite_engine::{BrowserEngine, WaitCondition};
use ferrite_model::{CompletionRequest, Message, ModelProvider, ModelTier};

/// One action the loop can select — a 1:1, serde-friendly mirror of
/// [`BrowserEngine`]'s methods, plus [`AgentAction::Finish`] to end the
/// loop with a final answer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    /// [`BrowserEngine::navigate`].
    Navigate {
        /// The URL to navigate to.
        url: String,
    },
    /// [`BrowserEngine::go_back`].
    GoBack,
    /// [`BrowserEngine::go_forward`].
    GoForward,
    /// [`BrowserEngine::reload`].
    Reload,
    /// [`BrowserEngine::dom_snapshot`].
    ReadDom,
    /// [`BrowserEngine::query`].
    Query {
        /// The selector to resolve.
        selector: String,
    },
    /// [`BrowserEngine::read_text`].
    ReadText {
        /// The selector to read text from.
        selector: String,
    },
    /// [`BrowserEngine::click`].
    Click {
        /// The selector to click.
        selector: String,
    },
    /// [`BrowserEngine::type_text`].
    TypeText {
        /// The selector to type into.
        selector: String,
        /// The text to type.
        text: String,
    },
    /// [`BrowserEngine::fill_form`].
    FillForm {
        /// `(selector, value)` pairs.
        fields: Vec<(String, String)>,
    },
    /// [`BrowserEngine::select_option`].
    SelectOption {
        /// The `<select>`-shaped element's selector.
        selector: String,
        /// The option value to select.
        value: String,
    },
    /// [`BrowserEngine::scroll`].
    Scroll {
        /// Horizontal scroll delta, CSS pixels.
        dx: i64,
        /// Vertical scroll delta, CSS pixels.
        dy: i64,
    },
    /// [`BrowserEngine::wait_for`] with [`WaitCondition::Selector`].
    WaitForSelector {
        /// The selector to wait for.
        selector: String,
    },
    /// [`BrowserEngine::wait_for`] with [`WaitCondition::Idle`].
    WaitIdle,
    /// [`BrowserEngine::screenshot`].
    Screenshot,
    /// [`BrowserEngine::download`].
    Download {
        /// The URL to download.
        url: String,
    },
    /// [`BrowserEngine::clipboard_read`].
    ClipboardRead,
    /// [`BrowserEngine::clipboard_write`].
    ClipboardWrite {
        /// The text to write.
        text: String,
    },
    /// [`BrowserEngine::js_execute`]. **Privileged** — see
    /// `ferrite_engine`'s module docs: this action existing on the loop's
    /// vocabulary is not a safety claim, the same way the trait method
    /// isn't.
    JsExecute {
        /// The script to run.
        script: String,
    },
    /// End the loop with a final answer for the user.
    Finish {
        /// The final answer.
        answer: String,
    },
}

/// Budgets bounding one [`run_agent_loop`] call — the directive's "step
/// budget, wall-clock budget, and a hard stop on repeated identical
/// actions."
#[derive(Debug, Clone, Copy)]
pub struct LoopBudget {
    /// Maximum number of model round-trips before the loop gives up.
    pub max_steps: usize,
    /// Maximum wall-clock time (measured via the injected [`Clock`], never
    /// real sleep in a test — R8) before the loop gives up.
    pub max_wall_clock: Duration,
    /// The loop stops the moment the *same* action would be issued this
    /// many times in a row (including the one about to be executed). `3`
    /// means: two identical actions already taken, plus this would-be
    /// third, stops before the third one executes.
    pub max_repeated_identical: usize,
}

impl Default for LoopBudget {
    fn default() -> Self {
        Self {
            max_steps: 20,
            max_wall_clock: Duration::from_secs(120),
            max_repeated_identical: 3,
        }
    }
}

/// Why [`run_agent_loop`] stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopStopReason {
    /// The model issued [`AgentAction::Finish`].
    Finished(String),
    /// `max_steps` model round-trips were used without finishing.
    StepBudgetExhausted,
    /// `max_wall_clock` elapsed without finishing.
    WallClockBudgetExhausted,
    /// The same action was about to be issued `max_repeated_identical`
    /// times in a row.
    RepeatedActionDetected(AgentAction),
    /// The model provider returned an error.
    ModelError(String),
    /// The model's response could not be parsed as an [`AgentAction`].
    MalformedAction(String),
}

/// The full record of one [`run_agent_loop`] call.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentLoopResult {
    /// Why the loop stopped.
    pub stop_reason: LoopStopReason,
    /// Every action actually executed, in order (does not include an
    /// action that triggered [`LoopStopReason::RepeatedActionDetected`] —
    /// that one is reported, not executed).
    pub actions_taken: Vec<AgentAction>,
    /// One observation string per action in `actions_taken`, same order.
    pub observations: Vec<String>,
}

fn describe_wait(condition: &AgentAction) -> WaitCondition {
    match condition {
        AgentAction::WaitForSelector { selector } => WaitCondition::Selector(selector.clone()),
        AgentAction::WaitIdle => WaitCondition::Idle,
        _ => unreachable!("describe_wait called on a non-wait action"),
    }
}

/// Executes one [`AgentAction`] against `engine`, returning a short
/// human-readable observation string to feed back to the model — or the
/// action's own [`AgentAction::Finish`] answer, which the caller must
/// intercept before calling this (there is no engine call for `Finish`).
fn execute_action(engine: &mut dyn BrowserEngine, action: &AgentAction) -> String {
    let result = match action {
        AgentAction::Navigate { url } => engine
            .navigate(url)
            .map(|((), o)| format!("navigated to {url} (origin {})", o.as_str())),
        AgentAction::GoBack => engine
            .go_back()
            .map(|((), o)| format!("went back (origin {})", o.as_str())),
        AgentAction::GoForward => engine
            .go_forward()
            .map(|((), o)| format!("went forward (origin {})", o.as_str())),
        AgentAction::Reload => engine
            .reload()
            .map(|((), o)| format!("reloaded (origin {})", o.as_str())),
        AgentAction::ReadDom => engine.dom_snapshot().map(|(snap, o)| {
            format!(
                "dom snapshot at {}: root role={}",
                o.as_str(),
                snap.root.role
            )
        }),
        AgentAction::Query { selector } => engine.query(selector).map(|(handles, o)| {
            format!(
                "query {selector} at {} -> {} element(s)",
                o.as_str(),
                handles.len()
            )
        }),
        AgentAction::ReadText { selector } => engine
            .read_text(selector)
            .map(|(text, o)| format!("text of {selector} at {}: {text}", o.as_str())),
        AgentAction::Click { selector } => engine
            .click(selector)
            .map(|((), o)| format!("clicked {selector} (origin {})", o.as_str())),
        AgentAction::TypeText { selector, text } => engine
            .type_text(selector, text)
            .map(|((), o)| format!("typed into {selector} (origin {})", o.as_str())),
        AgentAction::FillForm { fields } => engine
            .fill_form(fields)
            .map(|((), o)| format!("filled {} field(s) (origin {})", fields.len(), o.as_str())),
        AgentAction::SelectOption { selector, value } => engine
            .select_option(selector, value)
            .map(|((), o)| format!("selected {value} in {selector} (origin {})", o.as_str())),
        AgentAction::Scroll { dx, dy } => engine
            .scroll(*dx, *dy)
            .map(|((), o)| format!("scrolled by ({dx}, {dy}) (origin {})", o.as_str())),
        AgentAction::WaitForSelector { .. } | AgentAction::WaitIdle => engine
            .wait_for(describe_wait(action))
            .map(|((), o)| format!("wait satisfied (origin {})", o.as_str())),
        AgentAction::Screenshot => engine
            .screenshot()
            .map(|((w, h, _), o)| format!("captured {w}x{h} screenshot (origin {})", o.as_str())),
        AgentAction::Download { url } => engine
            .download(url)
            .map(|(path, o)| format!("downloaded {url} to {path} (origin {})", o.as_str())),
        AgentAction::ClipboardRead => engine
            .clipboard_read()
            .map(|(text, o)| format!("clipboard: {text} (origin {})", o.as_str())),
        AgentAction::ClipboardWrite { text } => engine
            .clipboard_write(text)
            .map(|((), o)| format!("wrote clipboard (origin {})", o.as_str())),
        AgentAction::JsExecute { script } => engine.js_execute(script).map(|(result, o)| {
            format!(
                "js_execute({} chars) at {} -> {result}",
                script.len(),
                o.as_str()
            )
        }),
        AgentAction::Finish { .. } => {
            unreachable!("Finish is intercepted by run_agent_loop before execute_action")
        }
    };

    match result {
        Ok(observation) => observation,
        Err(e) => format!("error: {e}"),
    }
}

/// System prompt describing the action vocabulary to the model. Kept as a
/// plain constant (not versioned/cached like `ferrite-model`'s §10.3
/// system prompts) because this loop does not yet route through the
/// content-addressed cache — see the module docs' scope note.
const SYSTEM_PROMPT: &str = r#"You are Ferrite, an agentic browser assistant.
Respond with exactly one JSON object describing your next action, matching
this shape: {"action": "<name>", ...fields}. Valid actions: navigate{url},
go_back, go_forward, reload, read_dom, query{selector}, read_text{selector},
click{selector}, type_text{selector,text}, fill_form{fields},
select_option{selector,value}, scroll{dx,dy}, wait_for_selector{selector},
wait_idle, screenshot, download{url}, clipboard_read,
clipboard_write{text}, js_execute{script}, finish{answer}.
Respond with the JSON object only, no other text."#;

/// Runs the plan → select tool → act → observe → repeat loop until the
/// model finishes, or a budget/loop-detection stop fires.
///
/// Engine-agnostic (`&mut dyn BrowserEngine`) and provider-agnostic
/// (`&dyn ModelProvider`), per the directive. `clock` is injected so a test
/// can enforce the wall-clock budget deterministically (R8) — production
/// callers pass `&ferrite_core::SystemClock`.
pub async fn run_agent_loop(
    provider: &dyn ModelProvider,
    engine: &mut dyn BrowserEngine,
    clock: &dyn Clock,
    model_tag: &str,
    tier: ModelTier,
    task_prompt: &str,
    budget: LoopBudget,
) -> AgentLoopResult {
    let start = clock.now();
    let mut actions_taken: Vec<AgentAction> = Vec::new();
    let mut observations: Vec<String> = Vec::new();
    let mut messages = vec![Message::user(task_prompt)];

    for _ in 0..budget.max_steps {
        let elapsed = clock.now() - start;
        let budget_delta =
            chrono::TimeDelta::from_std(budget.max_wall_clock).unwrap_or(chrono::TimeDelta::MAX);
        if elapsed >= budget_delta {
            return AgentLoopResult {
                stop_reason: LoopStopReason::WallClockBudgetExhausted,
                actions_taken,
                observations,
            };
        }

        let request = CompletionRequest::new(model_tag, tier, messages.clone())
            .with_system_prompt(SYSTEM_PROMPT, 1);
        let response = match provider.complete(request).await {
            Ok(r) => r,
            Err(e) => {
                return AgentLoopResult {
                    stop_reason: LoopStopReason::ModelError(e.to_string()),
                    actions_taken,
                    observations,
                }
            }
        };

        let action: AgentAction = match serde_json::from_str(response.content.trim()) {
            Ok(a) => a,
            Err(e) => {
                return AgentLoopResult {
                    stop_reason: LoopStopReason::MalformedAction(format!(
                        "{e} (raw: {})",
                        response.content
                    )),
                    actions_taken,
                    observations,
                }
            }
        };

        if let AgentAction::Finish { answer } = &action {
            return AgentLoopResult {
                stop_reason: LoopStopReason::Finished(answer.clone()),
                actions_taken,
                observations,
            };
        }

        // Repeated-identical-action hard stop: fires *before* executing the
        // action that would make it N in a row, so a real BrowserEngine
        // (ServoEngine included) never actually performs the Nth repeat.
        if budget.max_repeated_identical > 0 {
            let window = budget.max_repeated_identical - 1;
            if window <= actions_taken.len()
                && actions_taken[actions_taken.len() - window..]
                    .iter()
                    .all(|a| a == &action)
            {
                return AgentLoopResult {
                    stop_reason: LoopStopReason::RepeatedActionDetected(action),
                    actions_taken,
                    observations,
                };
            }
        }

        let observation = execute_action(engine, &action);
        messages.push(Message::assistant(
            serde_json::to_string(&action).unwrap_or_default(),
        ));
        messages.push(Message::user(format!("Observation: {observation}")));
        observations.push(observation);
        actions_taken.push(action);
    }

    AgentLoopResult {
        stop_reason: LoopStopReason::StepBudgetExhausted,
        actions_taken,
        observations,
    }
}

/// Convenience: a fixed clock that advances by a set delta every time
/// `now()` is read, so a wall-clock budget test does not need to sleep for
/// real (R8) — every call to `now()` moves time forward deterministically.
///
/// Lives here (not in `ferrite-core`) because it is specific to this loop's
/// test needs: `ferrite_core::FixedClock` only advances when a test
/// explicitly calls `advance()`, which does not fit a loop that reads the
/// clock exactly once per iteration and needs each iteration to look like
/// real elapsed time without an explicit advance call between them.
#[cfg(test)]
#[derive(Debug)]
struct AutoAdvanceClock {
    start: chrono::DateTime<chrono::Utc>,
    step: chrono::TimeDelta,
    calls: std::sync::atomic::AtomicU32,
}

#[cfg(test)]
impl Clock for AutoAdvanceClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.start + self.step * i32::try_from(n).unwrap_or(i32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use ferrite_engine::MockEngine;
    use ferrite_model::MockProvider;

    use super::*;

    fn navigate_json(url: &str) -> String {
        serde_json::to_string(&AgentAction::Navigate {
            url: url.to_string(),
        })
        .unwrap()
    }

    fn finish_json(answer: &str) -> String {
        serde_json::to_string(&AgentAction::Finish {
            answer: answer.to_string(),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn the_loop_stops_when_the_model_finishes() {
        let provider = MockProvider::new()
            .push_content(navigate_json("https://a.example/"))
            .push_content(finish_json("done"));
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "go to a.example",
            LoopBudget::default(),
        )
        .await;

        assert_eq!(
            result.stop_reason,
            LoopStopReason::Finished("done".to_string())
        );
        assert_eq!(result.actions_taken.len(), 1);
    }

    #[tokio::test]
    async fn step_budget_is_enforced() {
        // Always issues a distinct-enough action to not trip loop
        // detection first: alternate two different navigations.
        let provider = MockProvider::new().always(|req| {
            let n = req.messages.len();
            let url = if n % 4 == 0 {
                "https://a.example/"
            } else {
                "https://b.example/"
            };
            ferrite_model::MockStep::Content(navigate_json(url))
        });
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;
        let budget = LoopBudget {
            max_steps: 5,
            max_wall_clock: Duration::from_secs(3600),
            max_repeated_identical: 100, // effectively disabled
        };

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "wander",
            budget,
        )
        .await;

        assert_eq!(result.stop_reason, LoopStopReason::StepBudgetExhausted);
        assert_eq!(result.actions_taken.len(), 5);
    }

    #[tokio::test]
    async fn wall_clock_budget_is_enforced_with_an_injected_clock_never_real_sleep() {
        let provider = MockProvider::new().always_content(navigate_json("https://a.example/"));
        let mut engine = MockEngine::new();
        // Each `now()` call jumps 60s forward; a 120s budget is exhausted
        // by the third iteration's pre-check — well before max_steps.
        let clock = AutoAdvanceClock {
            start: chrono::DateTime::UNIX_EPOCH,
            step: chrono::TimeDelta::seconds(60),
            calls: std::sync::atomic::AtomicU32::new(0),
        };
        let budget = LoopBudget {
            max_steps: 1000,
            max_wall_clock: Duration::from_secs(120),
            max_repeated_identical: 1000,
        };

        let started = std::time::Instant::now();
        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "loiter",
            budget,
        )
        .await;
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "must not have actually slept to exhaust the wall-clock budget"
        );

        assert_eq!(result.stop_reason, LoopStopReason::WallClockBudgetExhausted);
        assert!(
            result.actions_taken.len() < 1000,
            "must stop well short of the step budget: {}",
            result.actions_taken.len()
        );
    }

    #[tokio::test]
    async fn repeated_identical_actions_trigger_a_hard_stop_not_an_infinite_loop() {
        // MockEngine always returns the same state, and the scripted
        // provider always asks for the exact same click — this would spin
        // forever without the repeat guard.
        let click_json = serde_json::to_string(&AgentAction::Click {
            selector: "#retry".to_string(),
        })
        .unwrap();
        let provider = MockProvider::new().always_content(click_json.clone());
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;
        let budget = LoopBudget {
            max_steps: 1000,
            max_wall_clock: Duration::from_secs(3600),
            max_repeated_identical: 3,
        };

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "click forever",
            budget,
        )
        .await;

        let expected_action: AgentAction = serde_json::from_str(&click_json).unwrap();
        assert_eq!(
            result.stop_reason,
            LoopStopReason::RepeatedActionDetected(expected_action)
        );
        // Two identical clicks were actually executed; the third was
        // caught before execution.
        assert_eq!(result.actions_taken.len(), 2);
    }

    #[tokio::test]
    async fn a_model_error_stops_the_loop_rather_than_panicking() {
        let provider = MockProvider::new().push_error(ferrite_model::ModelError::EmptyResponse {
            provider: ferrite_model::ProviderId::Mock,
        });
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "task",
            LoopBudget::default(),
        )
        .await;

        assert!(matches!(result.stop_reason, LoopStopReason::ModelError(_)));
    }

    #[tokio::test]
    async fn a_malformed_action_stops_the_loop_rather_than_panicking() {
        let provider = MockProvider::new().push_content("not json");
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "task",
            LoopBudget::default(),
        )
        .await;

        assert!(matches!(
            result.stop_reason,
            LoopStopReason::MalformedAction(_)
        ));
    }

    #[tokio::test]
    async fn executed_actions_carry_real_origin_tracking_through_observations() {
        let provider = MockProvider::new()
            .push_content(navigate_json("https://a.example/"))
            .push_content(finish_json("ok"));
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "go",
            LoopBudget::default(),
        )
        .await;

        assert_eq!(result.observations.len(), 1);
        assert!(
            result.observations[0].contains("https://a.example"),
            "observation must surface the real origin the action returned: {}",
            result.observations[0]
        );
    }
}
