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
///
/// `pub` (not crate-private) so a caller whose own engine cannot cross a
/// `tokio::spawn` boundary — `ferrite-ui`'s live `BorrowedServoEngine`,
/// which wraps a `!Send` `HeadlessServoSession` — can still reuse this
/// exact per-action dispatch logic from a message-driven step loop of its
/// own, rather than duplicating it, even though it cannot call
/// [`run_agent_loop`] directly for the reason [`BrowserEngine`]'s own
/// module docs give (no `Send` bound, by design). See
/// `docs/handoffs/b03.md` for the full reasoning.
pub fn execute_action(engine: &mut dyn BrowserEngine, action: &AgentAction) -> String {
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
///
/// `pub` so a caller that cannot call [`run_agent_loop`] directly (e.g.
/// `ferrite-ui`'s live loop, driven step-by-step because its engine cannot
/// cross a `tokio::spawn` boundary — see [`execute_action`]'s own doc
/// comment) can still ask the model with the exact same prompt this loop
/// uses, rather than a second, independently-maintained copy of the text.
pub const SYSTEM_PROMPT: &str = r#"You are Ferrite, an agentic browser assistant.

Respond with EXACTLY ONE valid JSON object and nothing else.

The JSON object MUST have an "action" field. The value of "action" must be ONLY the action name. NEVER put parameters inside the "action" value.

Valid JSON formats:

{"action":"navigate","url":"https://example.com"}
{"action":"go_back"}
{"action":"go_forward"}
{"action":"reload"}
{"action":"read_dom"}
{"action":"query","selector":"CSS_SELECTOR"}
{"action":"read_text","selector":"CSS_SELECTOR"}
{"action":"click","selector":"CSS_SELECTOR"}
{"action":"type_text","selector":"CSS_SELECTOR","text":"TEXT"}
{"action":"fill_form","fields":[["CSS_SELECTOR","TEXT"]]}
{"action":"select_option","selector":"CSS_SELECTOR","value":"VALUE"}
{"action":"scroll","dx":0,"dy":500}
{"action":"wait_for_selector","selector":"CSS_SELECTOR"}
{"action":"wait_idle"}
{"action":"screenshot"}
{"action":"download","url":"https://example.com/file"}
{"action":"clipboard_read"}
{"action":"clipboard_write","text":"TEXT"}
{"action":"js_execute","script":"JAVASCRIPT"}
{"action":"finish","answer":"FINAL ANSWER"}

IMPORTANT:
- "action" must contain ONLY one of the action names above.
- For type_text, "selector" and "text" MUST be separate JSON fields.
- For fill_form, "fields" MUST be an array of [selector, text] pairs.
- For select_option, "selector" and "value" MUST be separate JSON fields.
- For scroll, "dx" and "dy" MUST be separate JSON fields.
- NEVER write type_text{selector,text}.
- NEVER write {"action":"type_text{...}"}.
- NEVER put parameters inside the "action" string.
- NEVER use an object/map for fill_form fields.
- Do NOT use markdown.
- Do NOT add explanations.
- Output JSON only."#;

/// Version tag passed to [`ferrite_model::CompletionRequest::with_system_prompt`]
/// alongside [`SYSTEM_PROMPT`] — kept as a named constant so both this
/// module and any external caller reusing the same prompt text pass the
/// identical version rather than two independently-chosen literals.
pub const SYSTEM_PROMPT_VERSION: u32 = 1;

/// `options.num_predict` for every agent-loop step (`ModelTier::Main`).
///
/// `ferrite_model::SamplingOptions::default()`'s `num_predict = 128` is
/// correct for the fingerprint call (§10.2: "a short JSON array... capped
/// around 128"), but this loop's own responses are a full `AgentAction`
/// JSON object — for `Finish`, a freeform prose `answer` that can easily
/// run well past 128 tokens. Left at the default, a real Ollama backend
/// hard-truncates mid-generation once the cap is hit, so a longer answer
/// comes back as invalid JSON with an unterminated string — a directly
/// observed, reproduced failure (`serde_json`'s "EOF while parsing a
/// string"), not a hypothetical one. `2048` is generous headroom for a
/// realistic answer or a `fill_form`/`js_execute` payload while still
/// being a real, finite cap, per §10.3's "cap `num_predict` on every
/// call" — never uncapped.
pub const AGENT_LOOP_NUM_PREDICT: u32 = 2048;

/// How many *consecutive* unparseable model responses [`run_agent_loop`]
/// tolerates, by feeding the parse error back to the model as an
/// observation and asking it to try again, before giving up with
/// [`LoopStopReason::MalformedAction`].
///
/// A single malformed response — truncation, stray prose around the JSON,
/// a markdown code fence — is exactly the kind of mistake a model often
/// self-corrects from on the very next turn once told what was wrong with
/// its last one. Ending the whole task on the first such glitch (the prior
/// behavior) turned a recoverable hiccup into a hard failure surfaced
/// straight to the user as a raw parser error. Retries do not count
/// against `LoopBudget::max_steps` (a retry takes no real browser action,
/// so it should not cost part of the user's step budget); they are
/// instead independently bounded by this constant, and reset to zero the
/// moment a valid action is parsed, so a model stuck producing garbage
/// still fails fast rather than looping forever.
pub const MAX_CONSECUTIVE_MALFORMED_STEPS: u32 = 2;

/// Maximum approximate character budget for the conversation history sent
/// to the model on each step.
///
/// We deliberately stay well below a real model's context window. Character
/// count is only an approximation of tokens, so this leaves substantial
/// headroom for tokenization differences and the system prompt.
const MAX_HISTORY_CHARS: usize = 120_000;

/// Maximum number of characters retained from a single browser observation
/// before it is added to the conversation.
///
/// Browser text/clipboard/JS results can be unexpectedly large, so one
/// observation must never be allowed to dominate the context on its own.
const MAX_OBSERVATION_CHARS: usize = 8_000;

/// Truncates `observation` to [`MAX_OBSERVATION_CHARS`] before it is added
/// to the model conversation. Keeps both the head (usually the useful
/// description) and the tail (errors/results often appear there).
///
/// `pub` for the same reason [`execute_action`] and [`SYSTEM_PROMPT`] are:
/// `ferrite-ui`'s live loop builds its own conversation history step by
/// step (see those items' doc comments for why it cannot call
/// [`run_agent_loop`] directly) and must apply the identical budget to it,
/// rather than a second, independently-tuned one.
#[must_use]
pub fn compact_observation(observation: &str) -> String {
    if observation.len() <= MAX_OBSERVATION_CHARS {
        return observation.to_string();
    }

    let head_target = MAX_OBSERVATION_CHARS * 3 / 4;
    let tail_target = MAX_OBSERVATION_CHARS - head_target;

    let head: String = observation.chars().take(head_target).collect();
    let tail: String = {
        let mut rev: Vec<char> = observation.chars().rev().take(tail_target).collect();
        rev.reverse();
        rev.into_iter().collect()
    };

    let omitted = observation
        .chars()
        .count()
        .saturating_sub(head.chars().count())
        .saturating_sub(tail.chars().count());

    format!(
        "{head}\n\n[... observation truncated: approximately {omitted} characters omitted ...]\n\n{tail}"
    )
}

/// Keeps the original task prompt plus the newest action/observation pairs
/// while `messages`' total content size stays within [`MAX_HISTORY_CHARS`].
///
/// Messages are stored as `[task, action, observation, action, observation,
/// ...]` (see [`run_agent_loop`]), so repeatedly dropping the oldest pair
/// after `messages[0]` preserves the original task while discarding the
/// oldest, least-relevant turns first.
///
/// `pub` for the same reason as [`compact_observation`].
pub fn trim_message_history(messages: &mut Vec<Message>) {
    loop {
        let size: usize = messages.iter().map(|m| m.content.len()).sum();
        if size <= MAX_HISTORY_CHARS || messages.len() <= 3 {
            break;
        }

        // Drop the oldest action/observation pair, keeping messages[0] (the
        // original task) untouched.
        messages.remove(1);
        if messages.len() > 1 {
            messages.remove(1);
        }
    }
}

/// Runs the plan → select tool → act → observe → repeat loop until the
/// model finishes, or a budget/loop-detection stop fires.
///
/// Engine-agnostic and provider-agnostic (`&dyn ModelProvider`), per the
/// directive. `clock` is injected so a test can enforce the wall-clock
/// budget deterministically (R8) — production callers pass
/// `&ferrite_core::SystemClock`.
///
/// # Why `<E: BrowserEngine>` rather than `&mut dyn BrowserEngine`
///
/// A trait object erases its concrete type's auto traits: even though
/// `ferrite_ipi::dry_run::DryRunEngine` is `Send` (plain owned fields, no
/// `Rc`), calling a function whose *signature* names `&mut dyn
/// BrowserEngine` produces a future that is unconditionally `!Send`,
/// because `dyn BrowserEngine` itself carries no `Send` bound (deliberately
/// — see that trait's own module docs, for `ServoEngine`'s sake). Being
/// generic instead lets the caller's own monomorphized type's `Send`-ness
/// propagate: `run_agent_loop::<DryRunEngine>`'s future is `Send` (so
/// `ferrite-ui`'s `BrowserLoopDryRunDriver` can call it inside a
/// `tokio::spawn`ed dry-run task), while `run_agent_loop::<ServoEngine>`
/// correctly stays `!Send`, exactly reflecting that engine's real
/// constraint. `E` stays `Sized` (no `?Sized`): the internal call to
/// [`execute_action`] (which takes `&mut dyn BrowserEngine`) needs an
/// unsized-coercion site, and that coercion itself requires a `Sized`
/// source type.
pub async fn run_agent_loop<E: BrowserEngine>(
    provider: &dyn ModelProvider,
    engine: &mut E,
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
    let mut steps_taken: usize = 0;
    let mut consecutive_malformed: u32 = 0;

    loop {
        if steps_taken >= budget.max_steps {
            return AgentLoopResult {
                stop_reason: LoopStopReason::StepBudgetExhausted,
                actions_taken,
                observations,
            };
        }

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
            .with_system_prompt(SYSTEM_PROMPT, SYSTEM_PROMPT_VERSION)
            .with_options(
                ferrite_model::SamplingOptions::default().with_num_predict(AGENT_LOOP_NUM_PREDICT),
            );
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
            Ok(a) => {
                consecutive_malformed = 0;
                a
            }
            Err(e) => {
                consecutive_malformed += 1;
                if consecutive_malformed > MAX_CONSECUTIVE_MALFORMED_STEPS {
                    return AgentLoopResult {
                        stop_reason: LoopStopReason::MalformedAction(format!(
                            "{e} (raw: {})",
                            response.content
                        )),
                        actions_taken,
                        observations,
                    };
                }
                // Give the model a chance to self-correct: feed the raw
                // (possibly truncated) response back as its own turn, then
                // ask for a single complete, valid JSON object. This does
                // not consume a step of `budget.max_steps` — no real
                // browser action was taken — but is itself bounded by
                // `MAX_CONSECUTIVE_MALFORMED_STEPS` above, so a model stuck
                // producing garbage still fails fast rather than spinning
                // forever.
                messages.push(Message::assistant(compact_observation(
                    response.content.trim(),
                )));
                messages.push(Message::user(format!(
                    "Observation: your last response could not be parsed as a single, \
                     complete, valid JSON action ({e}). It may have been cut off or \
                     included extra text. Respond with EXACTLY ONE complete, valid JSON \
                     object and nothing else."
                )));
                trim_message_history(&mut messages);
                continue;
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
        let compacted_observation = compact_observation(&observation);
        messages.push(Message::user(format!(
            "Observation: {compacted_observation}"
        )));
        trim_message_history(&mut messages);
        observations.push(observation);
        actions_taken.push(action);
        steps_taken += 1;
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
        // Every attempt (the first try plus every retry) comes back
        // unparseable, so the retry budget itself must be what ends this,
        // not an exhausted mock queue.
        let provider = MockProvider::new().always_content("not json");
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
        assert_eq!(
            result.actions_taken.len(),
            0,
            "a retry that never produces a valid action must never be recorded as one taken"
        );
    }

    #[tokio::test]
    async fn a_malformed_response_is_retried_and_recovers_on_the_next_valid_one() {
        // First attempt: truncated/malformed JSON (the real failure mode
        // this retry exists for — a response cut short by num_predict).
        // Second attempt: a valid finish. The loop must recover instead of
        // ending on the first glitch.
        let provider = MockProvider::new()
            .push_content(r#"{"action":"finish","answer":"This page is a"#)
            .push_content(finish_json("done"));
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

        assert_eq!(
            result.stop_reason,
            LoopStopReason::Finished("done".to_string()),
            "a malformed response must not end the task once a later response is valid"
        );
    }

    #[tokio::test]
    async fn a_malformed_response_retry_does_not_consume_the_step_budget() {
        // Two malformed responses (within the retry budget), then two
        // distinct real actions, then finish — with max_steps set to
        // exactly 3 (matching the 3 real model turns: navigate, navigate,
        // finish), this must still succeed. If the two malformed retries
        // counted against the budget, only one real action would fit
        // before `StepBudgetExhausted` fired.
        let provider = MockProvider::new()
            .push_content("not json")
            .push_content("still not json")
            .push_content(navigate_json("https://a.example/"))
            .push_content(navigate_json("https://b.example/"))
            .push_content(finish_json("done"));
        let mut engine = MockEngine::new();
        let clock = ferrite_core::SystemClock;
        let budget = LoopBudget {
            max_steps: 3,
            ..LoopBudget::default()
        };

        let result = run_agent_loop(
            &provider,
            &mut engine,
            &clock,
            "tag",
            ModelTier::Main,
            "task",
            budget,
        )
        .await;

        assert_eq!(
            result.stop_reason,
            LoopStopReason::Finished("done".to_string()),
            "the two malformed retries must not have eaten into the 3-step budget"
        );
        assert_eq!(result.actions_taken.len(), 2);
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

    // ── compact_observation / trim_message_history: context-budget guards ──

    #[test]
    fn compact_observation_leaves_a_short_observation_unchanged() {
        let short = "navigated to https://a.example/ (origin https://a.example)";
        assert_eq!(compact_observation(short), short);
    }

    #[test]
    fn compact_observation_truncates_a_long_observation_keeping_head_and_tail() {
        let long = "A".repeat(20_000) + "TAIL_MARKER";
        let compacted = compact_observation(&long);

        assert!(compacted.len() < long.len());
        assert!(compacted.starts_with('A'));
        assert!(
            compacted.ends_with("TAIL_MARKER"),
            "the tail (often where errors/results appear) must survive truncation: {compacted}"
        );
        assert!(compacted.contains("truncated"));
    }

    #[test]
    fn trim_message_history_keeps_the_original_task_and_drops_the_oldest_pairs_first() {
        let mut messages = vec![Message::user("original task")];
        for i in 0..50 {
            messages.push(Message::assistant(format!("action {i}")));
            messages.push(Message::user("O".repeat(5_000)));
        }

        trim_message_history(&mut messages);

        let size: usize = messages.iter().map(|m| m.content.len()).sum();
        assert!(
            size <= MAX_HISTORY_CHARS,
            "history must be trimmed to the budget: {size}"
        );
        assert_eq!(
            messages[0].content, "original task",
            "the original task must never be dropped"
        );
        assert_eq!(
            messages.last().unwrap().content,
            "O".repeat(5_000),
            "the newest turns must be kept, not the oldest"
        );
    }

    #[test]
    fn trim_message_history_never_drops_below_the_task_plus_one_pair() {
        let mut messages = vec![
            Message::user("original task"),
            Message::assistant("action"),
            Message::user("O".repeat(1_000_000)),
        ];

        trim_message_history(&mut messages);

        assert_eq!(
            messages.len(),
            3,
            "must stop trimming once only the task and its newest pair remain, even over budget"
        );
    }
}
