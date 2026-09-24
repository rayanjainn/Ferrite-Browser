// ferrite-ui — Iced UI shell for Ferrite Browser.
//
// ## Architecture
//
// Iced 0.13 functional builder API — no trait to implement. State is
// `FerriteBrowser`, messages are `FerriteBrowserMessage`, and the three
// free functions `update`, `view`, `subscription` are passed to the builder.
//
// ## Interaction model
//
// The Servo frame is rendered as an `iced_widget::image` inside a
// `mouse_area` that captures mouse move, mouse press, mouse release, and
// wheel events and forwards them to `HeadlessServoSession` as native Servo
// input events (`InputEvent::MouseMove`, `MouseButton`, `Wheel`).
//
// ## Coordinate spaces (C3a)
//
// `mouse_area`/`responsive` report positions and sizes in iced's logical
// points; `HeadlessServoSession`'s render buffer and `send_mouse_*`/
// `send_scroll`'s `DevicePoint` coordinates are physical pixels. Two
// fields bridge that gap: `scale_factor` (physical px per logical point,
// fetched once via `iced::window::get_scale_factor` in `launch()`) scales
// every pointer-event coordinate before it reaches the session, and
// `content_area_size` (the content container's true logical size, measured
// by wrapping it in `iced_widget::responsive` in `view()`) drives
// `ServoFrame`'s tick handler to keep the active tab's session buffer
// resized to match. Before this, the buffer stayed at its hardcoded
// startup size forever regardless of window size or panel visibility,
// which is what made the displayed frame blurry (stretched to fill a
// differently-sized area) and put the cursor Servo actually saw somewhere
// other than where it visually was.
//
// ## Live agent execution (B3, docs/TO-DO.md T-224/T-220/T-229)
//
// The agent loop drives `ferrite_agent::browser_loop::{AgentAction,
// execute_action}` against a `ferrite_engine_servo::BorrowedServoEngine`
// wrapping the active tab's own `HeadlessServoSession` — the same session
// this file's own Servo-frame code already drives correctly via a real
// winit event loop (Iced's), which is exactly the ingredient
// `ferrite_engine_servo::ServoEngine`'s own standalone conformance tests
// found missing (`docs/TO-DO.md` T-220).
//
// `ferrite_engine::BrowserEngine` deliberately has no `Send` bound (see
// that trait's own module docs) because the real engine wraps `Rc`-based
// Servo state — so it cannot be moved into a `tokio::spawn`ed background
// task, which rules out calling `browser_loop::run_agent_loop` directly
// against the live engine (unlike the dry run's `DryRunEngine`, which holds
// no such state and *is* driven by a direct `run_agent_loop` call — see
// `BrowserLoopDryRunDriver` below). Instead, only the per-step model round
// trip (`&dyn ModelProvider`, `Send + Sync`) is spawned in the background;
// each returned `AgentAction` is dispatched against the live engine
// synchronously, on the Iced update thread, via `execute_action` — see the
// `AgentStepReady` handler in `update()`. `docs/handoffs/b03.md` has the
// full reasoning for this deviation from calling `run_agent_loop` directly.
//
// ## Keyboard shortcuts (platform-aware)
//   macOS : Cmd+T/W/R/L/J, F5, F12, Alt+←/→, Esc
//   other : Ctrl+T/W/R/L/J, F5, F12, Alt+←/→, Esc

use std::collections::HashMap;
use std::sync::Arc;

mod icons;
use icons::{icon, Icon};

use ferrite_agent::browser_loop::{
    compact_observation, execute_action, run_agent_loop, trim_message_history, AgentAction,
    LoopBudget, LoopStopReason, AGENT_LOOP_NUM_PREDICT, MAX_CONSECUTIVE_MALFORMED_STEPS,
    SYSTEM_PROMPT, SYSTEM_PROMPT_VERSION,
};
use ferrite_audit_log::{AuditEntry, AuditEventKind, PersistentAuditLog};
use ferrite_engine_servo::BorrowedServoEngine;
use ferrite_ipi::comparator::{compare, ConsentDecision, ExpectedFingerprint, FingerprintDiff};
use ferrite_ipi::dry_run::DryRunRecord;
use ferrite_ipi::tool_decision::{DefenseMode, LoopOutcome, ToolDecisionEngine, ToolId};
use ferrite_ipi::IpiTask;
use ferrite_model::{CompletionRequest, Message, ModelProvider, ModelTier, SamplingOptions};
use ferrite_servo::session::{HeadlessServoSession, LoadStatus};
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{
    keyboard, time, window, Background, Border, Color, Element, Length, Padding, Size,
    Subscription, Task, Theme,
};
use iced_widget::image::{Handle as ImageHandle, Image as ServoImage};
use iced_widget::responsive;
use std::cell::Cell;

// ---------------------------------------------------------------------------
// Real ModelProvider construction (T-229)
// ---------------------------------------------------------------------------

/// Constructs a real, configured `ferrite_model::ModelProvider` — mirrors
/// `ferrite-eval::harness::try_real_provider()` exactly (that function is
/// this project's own tested, live-verified reference implementation, see
/// `docs/EVALUATION.md` §2.6): `ModelConfig::from_env()`, Ollama first
/// (keyring-backed, service `"ferrite"`), Gemini as the fallback.
///
/// Returns `None` — never an `Err` — for any construction failure (unset
/// `FERRITE_MODEL_SMALL`/`FERRITE_MODEL_MAIN`, no key anywhere, no keyring
/// on this machine). `FerriteBrowser::default()` falls back to
/// `ferrite_model::MockProvider::new()` when this returns `None`, giving
/// the fingerprint's `may_use` layer the same fail-to-empty behavior
/// `CLAUDE.md`'s invariant requires (`must_use`, the rule layer, and
/// therefore containment itself, is unaffected either way). The live agent
/// loop itself still needs a real, reachable provider to choose actions at
/// all — with none configured it surfaces as a model error on the first
/// step, never as a bypass of the fingerprint/dry-run/consent gate in front
/// of it.
fn try_real_model_provider() -> Option<(Arc<dyn ModelProvider>, ferrite_model::ModelConfig)> {
    let config = ferrite_model::ModelConfig::from_env().ok()?;

    if let Ok(ollama) = ferrite_model::backends::shared_ollama(
        &config,
        ModelTier::Small,
        &ferrite_model::SystemEnv,
        &ferrite_model::OsKeyring,
    ) {
        let provider: Arc<dyn ModelProvider> = ollama;
        return Some((provider, config));
    }

    if let Ok(gemini) = ferrite_model::GeminiProvider::from_config(
        &config,
        ModelTier::Small,
        &ferrite_model::SystemEnv,
        &ferrite_model::OsKeyring,
    ) {
        let provider: Arc<dyn ModelProvider> = Arc::new(gemini);
        return Some((provider, config));
    }

    None
}

// ---------------------------------------------------------------------------
// Live agent-action vocabulary helpers — map `AgentAction` (the loop's real
// action enum) onto `ferrite_core::Primitive`/`ToolId`, the same wire
// vocabulary the fingerprint/comparator/consent machinery already speaks,
// rather than a second, independent one.
// ---------------------------------------------------------------------------

fn primitive_of_action(action: &AgentAction) -> ferrite_core::Primitive {
    use ferrite_core::Primitive;
    match action {
        AgentAction::Navigate { .. }
        | AgentAction::GoBack
        | AgentAction::GoForward
        | AgentAction::Reload => Primitive::Navigate,
        AgentAction::ReadDom | AgentAction::ReadText { .. } => Primitive::DomRead,
        AgentAction::Query { .. } => Primitive::DomQuery,
        AgentAction::Click { .. } => Primitive::Click,
        AgentAction::TypeText { .. } | AgentAction::SelectOption { .. } => Primitive::DomWrite,
        AgentAction::FillForm { .. } => Primitive::FormFill,
        AgentAction::Scroll { .. } => Primitive::Scroll,
        AgentAction::WaitForSelector { .. } | AgentAction::WaitIdle => Primitive::Wait,
        AgentAction::Screenshot => Primitive::Screenshot,
        AgentAction::Download { .. } => Primitive::Download,
        AgentAction::ClipboardRead => Primitive::ClipboardRead,
        AgentAction::ClipboardWrite { .. } => Primitive::ClipboardWrite,
        AgentAction::JsExecute { .. } => Primitive::JsExecute,
        AgentAction::Finish { .. } => {
            unreachable!("Finish is intercepted before an action is ever dispatched or logged")
        }
    }
}

/// `ToolId` for a live `AgentAction` — the same string vocabulary the
/// comparator/consent panel already key on (`ferrite_core::Primitive::as_str()`).
fn action_tool_id(action: &AgentAction) -> ToolId {
    ToolId::new(primitive_of_action(action).as_str())
}

/// The URL an `AgentAction` itself carries, if any — the only actions that
/// can be checked against a rejected origin without a live session (the
/// same honest limitation the pre-B3 `FilteredToolExecutor::tool_url` had:
/// an action with no URL of its own acts on "whatever the active tab
/// currently is," which cannot be checked from the action alone).
fn action_url(action: &AgentAction) -> Option<&str> {
    match action {
        AgentAction::Navigate { url } | AgentAction::Download { url } => Some(url.as_str()),
        _ => None,
    }
}

/// Normalizes a URL to `scheme://host`, lowercased — the same shape
/// `ferrite_ipi::dry_run::record::extract_origin` produces, reimplemented
/// locally since that function is crate-private to `ferrite-ipi`. Returns
/// `None` if `url` does not parse as an absolute URL with a host.
fn origin_of_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    Some(format!("{}://{}", parsed.scheme(), host).to_ascii_lowercase())
}

/// Whether `action` is blocked by the user's consent decision — checked
/// before every live-loop step executes. Real enforcement, not only a UI
/// filter: a rejected action never reaches `execute_action` (see
/// `a_rejected_tool_id_blocks_the_action_before_it_reaches_the_engine` and
/// `a_rejected_origin_blocks_a_navigate_before_it_reaches_the_engine`).
fn is_action_rejected(
    action: &AgentAction,
    rejected: &std::collections::HashSet<ToolId>,
    rejected_origins: &std::collections::HashSet<String>,
) -> bool {
    if rejected.contains(&action_tool_id(action)) {
        return true;
    }
    if let Some(origin) = action_url(action).and_then(origin_of_url) {
        if rejected_origins.contains(&origin) {
            return true;
        }
    }
    false
}

/// C2: a short, human-readable name for `action`'s kind — e.g. "Navigate",
/// "Click" — shown as the step's title in the agent sidebar's activity
/// feed. Paired with [`action_detail`] (the action's own parameter, if
/// any) and [`icon_for_action`] (its icon), replacing the old flat
/// `[primitive] detail` single-string log line (B3-era) with three
/// separately-styleable pieces.
fn action_label(action: &AgentAction) -> &'static str {
    match action {
        AgentAction::Navigate { .. } => "Navigate",
        AgentAction::GoBack => "Go back",
        AgentAction::GoForward => "Go forward",
        AgentAction::Reload => "Reload",
        AgentAction::ReadDom => "Read page",
        AgentAction::Query { .. } => "Query elements",
        AgentAction::ReadText { .. } => "Read text",
        AgentAction::Click { .. } => "Click",
        AgentAction::TypeText { .. } => "Type text",
        AgentAction::FillForm { .. } => "Fill form",
        AgentAction::SelectOption { .. } => "Select option",
        AgentAction::Scroll { .. } => "Scroll",
        AgentAction::WaitForSelector { .. } => "Wait for element",
        AgentAction::WaitIdle => "Wait",
        AgentAction::Screenshot => "Screenshot",
        AgentAction::Download { .. } => "Download",
        AgentAction::ClipboardRead => "Read clipboard",
        AgentAction::ClipboardWrite { .. } => "Write clipboard",
        AgentAction::JsExecute { .. } => "Run JavaScript",
        AgentAction::Finish { .. } => "Finish",
    }
}

/// C2: `action`'s own parameter, rendered as the step's detail line (a URL,
/// a selector, the text typed, ...) — empty for an action with nothing
/// further to show (`GoBack`, `WaitIdle`, ...). See [`action_label`].
fn action_detail(action: &AgentAction) -> String {
    match action {
        AgentAction::Navigate { url } | AgentAction::Download { url } => url.clone(),
        AgentAction::Query { selector }
        | AgentAction::ReadText { selector }
        | AgentAction::Click { selector }
        | AgentAction::WaitForSelector { selector } => selector.clone(),
        AgentAction::TypeText { selector, text } => format!("{selector} \u{2192} \"{text}\""),
        AgentAction::SelectOption { selector, value } => format!("{selector} \u{2192} {value}"),
        AgentAction::FillForm { fields } => format!("{} field(s)", fields.len()),
        AgentAction::Scroll { dx, dy } => format!("({dx}, {dy})"),
        AgentAction::ClipboardWrite { text } => text.clone(),
        AgentAction::JsExecute { script } => script.clone(),
        AgentAction::Finish { answer } => answer.clone(),
        AgentAction::GoBack
        | AgentAction::GoForward
        | AgentAction::Reload
        | AgentAction::ReadDom
        | AgentAction::WaitIdle
        | AgentAction::Screenshot
        | AgentAction::ClipboardRead => String::new(),
    }
}

/// C2: the icon shown next to `action`'s step in the agent sidebar's
/// activity feed. Grouped by the same action-class boundaries
/// `ferrite_core::Primitive`'s own taxonomy uses (see `Icon`'s doc
/// comment) rather than one icon per raw `AgentAction` variant, so the
/// icon set stays small and each glyph stays visually distinct at the
/// sidebar's render size. `Finish` reuses `Icon::Approve` (its outcome —
/// the task completing) and `JsExecute` reuses `Icon::Console` (already
/// this crate's "code"/JS glyph, used by the JS-console toggle button) —
/// deliberate reuse, not a placeholder, since both already mean exactly
/// this elsewhere in the same chrome.
fn icon_for_action(action: &AgentAction) -> Icon {
    match action {
        AgentAction::Navigate { .. }
        | AgentAction::GoBack
        | AgentAction::GoForward
        | AgentAction::Reload => Icon::Navigate,
        AgentAction::ReadDom | AgentAction::ReadText { .. } | AgentAction::Query { .. } => {
            Icon::Read
        }
        AgentAction::Click { .. } => Icon::Click,
        AgentAction::TypeText { .. }
        | AgentAction::SelectOption { .. }
        | AgentAction::FillForm { .. } => Icon::Write,
        AgentAction::Scroll { .. }
        | AgentAction::WaitForSelector { .. }
        | AgentAction::WaitIdle
        | AgentAction::Screenshot => Icon::Activity,
        AgentAction::Download { .. } => Icon::Download,
        AgentAction::ClipboardRead | AgentAction::ClipboardWrite { .. } => Icon::Clipboard,
        AgentAction::JsExecute { .. } => Icon::Console,
        AgentAction::Finish { .. } => Icon::Approve,
    }
}

// ---------------------------------------------------------------------------
// Dry-run driver: runs the real agent-decision loop
// (`ferrite_agent::browser_loop::run_agent_loop`) directly against
// `ferrite_ipi::dry_run::DryRunEngine`.
//
// This is the simplification B1's own handoff anticipated (docs/handoffs/
// b01.md, b02.md): `DryRunEngine` (unlike the live `BorrowedServoEngine`)
// holds no `Rc`/thread-local state — it is a plain, `Send`-safe recorder —
// so `run_agent_loop` can drive it directly inside the background tokio
// task the dry run already runs on, with zero bridging code. This replaces
// the old GeminiAgent/`EngineToolExecutor`-bridged `DryRunAgentDriver`.
// ---------------------------------------------------------------------------

struct BrowserLoopDryRunDriver<'a> {
    provider: &'a dyn ModelProvider,
    model_tag: String,
    prompt: String,
}

#[async_trait::async_trait]
impl<'a> ferrite_ipi::dry_run::DryRunDriver for BrowserLoopDryRunDriver<'a> {
    async fn drive(&self, engine: &mut ferrite_ipi::dry_run::DryRunEngine) -> Result<(), String> {
        let clock = ferrite_core::SystemClock;
        let result = run_agent_loop(
            self.provider,
            engine,
            &clock,
            &self.model_tag,
            ModelTier::Main,
            &self.prompt,
            LoopBudget::default(),
        )
        .await;
        // A model error means the dry run genuinely could not decide what
        // to do (e.g. no provider configured/reachable) — surfaced as a
        // real error so the caller does not mistake "the model never
        // answered" for "the plan was clean" (CLAUDE.md's "fail to empty,
        // never a bypass": an empty dry-run record must never be produced
        // by a silently-swallowed model failure). Every other stop reason
        // (finished, a budget exhausted, a repeated action, an unparseable
        // action) is real, honest partial-or-complete data for the
        // orchestrator's own record — not an error.
        match result.stop_reason {
            LoopStopReason::ModelError(e) => Err(e),
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Platform detection — used for keyboard shortcut labels
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
const MOD_LABEL: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
const MOD_LABEL: &str = "Ctrl";

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const ADDRESS_BAR_ID: &str = "ferrite_address_bar";
const JS_INPUT_ID: &str = "ferrite_js_input";

const TOOLBAR_HEIGHT: f32 = 46.0;
const TAB_BAR_HEIGHT: f32 = 36.0;
const BORDER_RADIUS: f32 = 8.0;
const PANEL_PADDING: u16 = 12;

/// Ticks (`ServoFrame`, ~16ms each) to wait after calling
/// `HeadlessServoSession::resize()` before trusting a frame read from that
/// session again — see `FerriteBrowser::resize_settle_ticks`'s doc comment.
/// The actual crash/corruption this workaround was first written for
/// turned out to have a real, different root cause, fixed directly in
/// `HeadlessServoSession::resize()` itself (a redundant, unguarded
/// `rendering_context.resize()` call that both suppressed Servo's own
/// resize-triggered repaint and skipped `make_current()` before touching
/// the surface — see that method's doc comment for the full trace against
/// the pinned `libservo` source). This constant is now a small residual
/// safety margin, not the primary fix, and is kept low (one tick) so it
/// doesn't itself make resizing feel less smooth.
const RESIZE_SETTLE_TICKS: u8 = 1;

/// Icon sizes — the two sizes every `icon()` call site in this crate picks
/// from, so the icon set reads as one consistent scale rather than a grab
/// bag of ad hoc pixel sizes. `ICON_SIZE` is the default (toolbar/tab-bar/
/// panel-toggle chrome); `ICON_SIZE_SM` is for icons paired tightly with
/// text at a smaller point size (consent-item rows, the tab close button).
const ICON_SIZE: f32 = 15.0;
const ICON_SIZE_SM: f32 = 12.0;

/// Advance per `ConsentPanelTick` for `FerriteBrowser::consent_panel_anim` —
/// ticks fire every 16ms (the same cadence `ServoFrame` already uses, see
/// `subscription()`), so this reaches 1.0 in ~200ms: the fast, subtle end of
/// the 150-250ms range typical for this kind of UI entrance transition.
const CONSENT_ANIM_STEP: f32 = 16.0 / 200.0;

/// Number of individually-lit segments in `view()`'s top-of-window loading
/// bar — see [`progress_segment_brightness`]. Wide enough to read as a
/// smooth sweep rather than a few chunky blocks, narrow enough that each
/// segment is still a real, visible width in the toolbar's `PANEL_PADDING`-
/// inset span.
const PROGRESS_SEGMENTS: usize = 12;

// ---------------------------------------------------------------------------
// Colour palette — C3c: real light/dark theme
// ---------------------------------------------------------------------------
//
// Through C3b this crate had exactly one theme: twelve module-level `Color`
// constants (`C_BASE`/`C_SURFACE`/.../`C_DANGER`) referenced by bare ident
// from `view()` and its helpers. `Palette` keeps the same twelve roles
// (lowercased, as struct fields) resolved per [`AppTheme`] instead of fixed
// at compile time — every call site's *meaning* is unchanged, only the
// lookup is now runtime.
//
// Two ways a function gets the right `&'static Palette` for the app's
// current theme, depending on what it already has in scope:
// - `view()`/`new_tab_page()`/`view_agent_sidebar()` already take
//   `state: &FerriteBrowser`, so each binds `let palette = state.palette();`
//   once near the top and every former `C_XXX` reference in that function
//   became `palette.xxx`.
// - The nine `*_style` functions below are passed to `.style(...)` as bare
//   `fn` pointers (e.g. `.style(nav_btn_style)`), so iced itself calls them
//   at render time with whatever `Theme` `launch()`'s `.theme(...)` closure
//   currently returns. `palette_for_theme(theme)` reads the `AppTheme` back
//   out of that `Theme`, so no extra state needs threading through the
//   `.style()` call sites at all — the parameter those functions already
//   had (previously named `_theme` and ignored) is now the one thing they
//   actually need.

/// One resolved colour per semantic role: background depth
/// (`base`/`surface`/`raised`/`divider`/`input`), text
/// (`text`/`text_dim`), the brand accent (`accent`/`accent_bright`), and the
/// three status colours (`safe`/`warn`/`danger`) — exactly the pre-C3c
/// `C_BASE`.../`C_DANGER` constants, as struct fields instead of bare idents.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub base: Color,
    pub surface: Color,
    pub raised: Color,
    pub divider: Color,
    pub text: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub accent_bright: Color,
    pub input: Color,
    pub safe: Color,
    pub warn: Color,
    pub danger: Color,
}

/// The palette this crate shipped through C3b, unchanged value-for-value —
/// still the default theme (see [`AppTheme`]'s `Default` impl), so a user
/// who never touches the new toggle sees exactly what they always have.
const DARK_PALETTE: Palette = Palette {
    base: Color {
        r: 0.08,
        g: 0.08,
        b: 0.10,
        a: 1.0,
    },
    surface: Color {
        r: 0.11,
        g: 0.11,
        b: 0.14,
        a: 1.0,
    },
    raised: Color {
        r: 0.17,
        g: 0.17,
        b: 0.21,
        a: 1.0,
    },
    divider: Color {
        r: 0.20,
        g: 0.20,
        b: 0.25,
        a: 1.0,
    },
    text: Color {
        r: 0.93,
        g: 0.93,
        b: 0.96,
        a: 1.0,
    },
    text_dim: Color {
        r: 0.50,
        g: 0.50,
        b: 0.58,
        a: 1.0,
    },
    accent: Color {
        r: 0.44,
        g: 0.38,
        b: 1.0,
        a: 1.0,
    },
    accent_bright: Color {
        r: 0.56,
        g: 0.50,
        b: 1.0,
        a: 1.0,
    },
    input: Color {
        r: 0.14,
        g: 0.14,
        b: 0.18,
        a: 1.0,
    },
    safe: Color {
        r: 0.20,
        g: 0.84,
        b: 0.54,
        a: 1.0,
    },
    warn: Color {
        r: 0.95,
        g: 0.65,
        b: 0.20,
        a: 1.0,
    },
    danger: Color {
        r: 1.0,
        g: 0.35,
        b: 0.35,
        a: 1.0,
    },
};

/// A considered light palette, not a naive RGB inversion of
/// [`DARK_PALETTE`]: background depth still runs the same direction real
/// browser chrome uses (near-white page/toolbar, a touch grayer tab strip,
/// grayer still for hover/raised state — Chrome's and Firefox's own light
/// themes order it the same way), the accent keeps the same purple hue
/// family but is deepened (`rgb(112,97,255)`/`rgb(143,128,255)` for
/// `accent`/`accent_bright` in the dark palette &rarr; `rgb(89,66,235)`/
/// `rgb(107,82,250)` here) so it still reads as foreground-safe against a
/// light background instead of washing out, and `safe`/`warn`/`danger` are
/// each darkened from the dark theme's saturated, light-on-dark-friendly
/// versions for the same reason — the original mint/amber/coral are all
/// under ~3.2:1 contrast against a near-white background, well under WCAG
/// AA's 4.5:1 for normal text, and this crate uses all three as small-text
/// badge/label colours (e.g. the audit log's kind column), not just as
/// decoration.
///
/// Contrast figures below are hand-computed against the WCAG relative-
/// luminance formula (sRGB-to-linear per channel, then
/// `0.2126R+0.7152G+0.0722B`, then `(L_light+0.05)/(L_dark+0.05)`) — not run
/// through an automated checker, so treat them as "designed with a target
/// in mind" rather than a certified audit: `text` on `base` ≈ 17:1 (AAA),
/// `text_dim` on `base` ≈ 5.4:1, `accent` on `base` ≈ 6:1, `safe`/`warn`/
/// `danger` on `base` ≈ 5.2-5.4:1 — all at or above AA's 4.5:1 for normal
/// text.
const LIGHT_PALETTE: Palette = Palette {
    base: Color {
        r: 0.980,
        g: 0.980,
        b: 0.988,
        a: 1.0,
    },
    surface: Color {
        r: 0.949,
        g: 0.953,
        b: 0.969,
        a: 1.0,
    },
    raised: Color {
        r: 0.910,
        g: 0.910,
        b: 0.941,
        a: 1.0,
    },
    divider: Color {
        r: 0.863,
        g: 0.863,
        b: 0.902,
        a: 1.0,
    },
    text: Color {
        r: 0.090,
        g: 0.090,
        b: 0.122,
        a: 1.0,
    },
    text_dim: Color {
        r: 0.400,
        g: 0.400,
        b: 0.460,
        a: 1.0,
    },
    accent: Color {
        r: 0.349,
        g: 0.259,
        b: 0.922,
        a: 1.0,
    },
    accent_bright: Color {
        r: 0.420,
        g: 0.322,
        b: 0.980,
        a: 1.0,
    },
    input: Color {
        r: 1.000,
        g: 1.000,
        b: 1.000,
        a: 1.0,
    },
    safe: Color {
        r: 0.039,
        g: 0.471,
        b: 0.294,
        a: 1.0,
    },
    warn: Color {
        r: 0.647,
        g: 0.333,
        b: 0.020,
        a: 1.0,
    },
    danger: Color {
        r: 0.784,
        g: 0.149,
        b: 0.149,
        a: 1.0,
    },
};

/// Which of the two shipped palettes the app is currently drawing from.
/// `Default` is `Dark` — this crate's only theme through C3b — so a user who
/// never touches `ToggleTheme` (see `FerriteBrowserMessage`) sees the exact
/// same app they always have; the toggle is opt-in, not a default-behavior
/// change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTheme {
    #[default]
    Dark,
    Light,
}

impl AppTheme {
    /// Flips to the other mode — the entire behavior of `ToggleTheme` (see
    /// `update()`).
    fn toggled(self) -> Self {
        match self {
            AppTheme::Dark => AppTheme::Light,
            AppTheme::Light => AppTheme::Dark,
        }
    }

    /// The resolved colour set for this mode.
    fn palette(self) -> &'static Palette {
        match self {
            AppTheme::Dark => &DARK_PALETTE,
            AppTheme::Light => &LIGHT_PALETTE,
        }
    }

    /// `launch()`'s `.theme(...)` closure reads this to pick iced's own
    /// built-in `Theme::Dark`/`Theme::Light` (which drives iced's default
    /// widget rendering — e.g. `text_input`'s selection/scrollbar chrome
    /// this crate doesn't style itself) — kept in lock-step with `Palette`
    /// selection rather than as independent state, so the two can never
    /// point at different themes.
    fn to_iced_theme(self) -> Theme {
        match self {
            AppTheme::Dark => Theme::Dark,
            AppTheme::Light => Theme::Light,
        }
    }
}

/// The same lookup as [`AppTheme::palette`], keyed by iced's own `Theme`
/// instead of `AppTheme` — for the nine `*_style` functions below, which
/// receive `&Theme` from iced itself at render time (whatever
/// `to_iced_theme()` last returned) rather than a `FerriteBrowser`
/// reference. Anything other than `Theme::Light` resolves to the dark
/// palette, so a hypothetical future `Theme::Custom(...)` (never constructed
/// by this crate today) degrades to the existing look instead of panicking.
fn palette_for_theme(theme: &Theme) -> &'static Palette {
    match theme {
        Theme::Light => &LIGHT_PALETTE,
        _ => &DARK_PALETTE,
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// C2: one entry in the agent sidebar's live activity feed
/// (`FerriteBrowser::agent_log`), replacing the flat `Vec<String>` label
/// list B3 left (`agent_tool_log`) with real per-step structure: an icon,
/// a title, the action's own parameter, and — unlike the old log — the
/// actual result `execute_action` returned, not just the fact that some
/// action ran.
#[derive(Debug, Clone)]
pub enum AgentLogEntry {
    /// A plain status line from a phase that has no individual action to
    /// itemize yet — currently only the dry-run phase's progress note
    /// (`AgentToolLogged`, e.g. "dry run complete — checking for
    /// unexpected activity").
    Note(String),
    /// One action the live loop actually took, or attempted.
    Step {
        icon: Icon,
        label: &'static str,
        detail: String,
        /// The real observation `execute_action` returned — or, if
        /// `blocked` is true, the fixed "blocked by user consent" string
        /// `is_action_rejected` produces instead of ever calling it.
        result: String,
        /// Whether the user's consent decision blocked this action
        /// before it reached the engine, rather than it having actually
        /// executed — styled differently in the sidebar (see
        /// `view_agent_sidebar`) so a blocked step doesn't read as if it
        /// succeeded.
        blocked: bool,
    },
}

/// Why one step's background model call did not produce a usable
/// `AgentAction`, carried over `AgentStepReady` in place of a plain
/// `String` so the handler in `update()` can tell the two failure modes
/// apart and treat them differently.
#[derive(Debug, Clone)]
pub enum StepFailure {
    /// The model provider call itself failed (network, auth, timeout,
    /// empty response, …). Never retried — a broken provider will not
    /// self-correct by being asked again in the same way.
    Model(String),
    /// The response body was not a single, complete, valid JSON
    /// `AgentAction` — most often a `finish.answer` truncated mid-string by
    /// the provider's own output cap, or stray prose/markdown fencing
    /// around the JSON. `AgentStepReady`'s handler retries this, up to
    /// `MAX_CONSECUTIVE_MALFORMED_STEPS` times, by feeding `message` back
    /// to the model as an observation rather than ending the run on what
    /// is often a one-off, self-correctable glitch.
    Malformed { raw: String, message: String },
}

/// State of an in-progress live agent-action loop (post-fingerprint, either
/// bypassed straight through or after a clean/consented dry run). Lives on
/// `FerriteBrowser` between the per-step background model calls
/// (`AgentStepReady`) that drive it — see this file's module docs for why
/// the loop is message-driven rather than a direct `run_agent_loop` call.
pub struct LiveAgentLoop {
    /// Conversation history sent to the model on every step — starts as
    /// `[Message::user(prompt)]`, then grows by one assistant (action) /
    /// user (observation) pair per completed step.
    messages: Vec<Message>,
    /// Every action actually executed so far, in order — used for
    /// step-budget accounting and repeated-action detection, mirroring
    /// `browser_loop::run_agent_loop`'s own bookkeeping exactly.
    actions_taken: Vec<AgentAction>,
    /// When this loop started — checked against `budget.max_wall_clock`
    /// before every step's model call.
    started_at: std::time::Instant,
    budget: LoopBudget,
    /// Tool ids the user rejected in the consent panel, if this loop is the
    /// post-consent real run — empty for a bypassed/clean-dry-run loop that
    /// never needed consent.
    rejected: std::collections::HashSet<ToolId>,
    /// Origins the user rejected in the consent panel, if this loop is the
    /// post-consent real run.
    rejected_origins: std::collections::HashSet<String>,
    /// Consecutive unparseable model responses seen in a row — mirrors
    /// `browser_loop::run_agent_loop`'s own counter of the same name.
    /// Reset to `0` the moment a step parses successfully; once it exceeds
    /// `MAX_CONSECUTIVE_MALFORMED_STEPS`, the run ends with the parse error
    /// instead of retrying again. See `AgentStepReady`'s handler.
    consecutive_malformed: u32,
}

pub struct FerriteBrowser {
    pub tabs: Vec<String>,
    pub active_tab: usize,
    pub address_bar_input: String,
    /// Committed (navigated-to) URL per tab.
    pub tab_urls: Vec<String>,
    pub show_audit_panel: bool,
    pub show_js_console: bool,
    pub audit_entries: Vec<AuditEntry>,
    pub servo_sessions: HashMap<usize, HeadlessServoSession>,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// Phase for the animated loading bar (0..1).
    pub progress_offset: f32,
    pub address_bar_focused: bool,
    pub tab_error: Vec<Option<String>>,
    pub tab_titles: Vec<String>,
    /// The active page's favicon for each tab, same indexing as
    /// `tab_titles`/`tab_urls` — `None` until `HeadlessServoSession::
    /// get_favicon()` reports one (or forever, for a page that never sets
    /// one, e.g. `about:blank`). Synced every `ServoFrame` tick alongside
    /// the page title (see that handler).
    pub tab_favicons: Vec<Option<ImageHandle>>,
    pub new_tab_search_input: String,
    pub js_input: String,
    pub js_output: Vec<(String, String)>,
    /// Most recent cursor position over the Servo content area, in logical
    /// points (the same space `mouse_area::on_move` reports and `view()`
    /// lays widgets out in) — never physical/device pixels. Scaled by
    /// `scale_factor` at the point each is forwarded to
    /// `HeadlessServoSession::send_mouse_*`, which operates in the render
    /// buffer's physical-pixel space (see `scale_factor`'s doc comment).
    pub cursor_pos: (f32, f32),
    /// Physical pixels per logical point for the app's window, fetched once
    /// via `iced::window::get_scale_factor` shortly after launch
    /// (`ScaleFactorReady`) — defaults to `1.0` until that resolves. Needed
    /// because `HeadlessServoSession`'s render buffer and
    /// `send_mouse_*`/`send_scroll`'s coordinates are in physical pixels,
    /// while every iced-reported position (`mouse_area::on_move`,
    /// `content_area_size`) is in logical points; on a HiDPI/Retina display
    /// those differ, and conflating them is exactly what caused the
    /// blurry-frame and hover/click-offset bugs this field's introduction
    /// fixes (C3a).
    pub scale_factor: f32,
    /// Most recently measured logical size of the Servo content area —
    /// written from `view()`'s `responsive` wrapper around the content
    /// element (interior mutability: `view()` takes `&self`, so a `Cell`
    /// is how a read-only render pass can still record what it measured)
    /// and read back by `ServoFrame`'s tick handler to decide whether the
    /// active tab's session buffer needs `resize()`ing to match.
    ///
    /// Replaces the older `content_y_offset`/`ContentAreaResized`
    /// message, which tracked only a hardcoded vertical chrome-height
    /// offset and — verified by grep before this fix — was never actually
    /// wired to a real producer anywhere in `update()`'s message handling,
    /// so it always held its initial constant. `responsive` reports the
    /// container's true available size on every layout pass instead of
    /// requiring this file to hand-replicate the chrome layout's
    /// heights/widths (tab bar + toolbar + progress bar + optional
    /// audit/JS panel + optional agent sidebar) — correct by construction
    /// as that layout changes, not by keeping two copies of it in sync.
    pub content_area_size: Cell<Size>,
    /// The physical-pixel size the active tab's session was last actually
    /// told to `resize()` to — compared against on every `ServoFrame` tick
    /// to decide whether a new `resize()` call is needed at all. Tracked
    /// here rather than re-derived from `HeadlessServoSession::size()`
    /// because that getter reflects `resize()`'s own immediate, synchronous
    /// bookkeeping (it updates `self.width`/`self.height` the instant it's
    /// called), not whether libservo's own compositor has actually
    /// finished reallocating the backing surface — see
    /// `resize_settle_ticks`'s doc comment for why that distinction is the
    /// whole fix here. Initialized to `(1280, 700)`, matching every
    /// session's real starting buffer size, so the very first tick doesn't
    /// treat "unmeasured yet" as "must resize".
    pub last_resized_content_px: (u32, u32),
    /// Ticks remaining before it's safe to read a frame from the session
    /// most recently `resize()`d — see `ServoFrame`'s handler.
    ///
    /// **Corrected history:** this field was first written on the theory
    /// that the segfault/black-screen/stuck-rendering bug was a same-tick
    /// race — `session.resize()` immediately followed by
    /// `session.sync_and_read()` reading a frame before libservo finished
    /// reallocating the surface. That fix alone did **not** resolve the
    /// bug when tested on real hardware; the actual root cause was in
    /// `HeadlessServoSession::resize()` itself (a redundant, unguarded
    /// `rendering_context.resize()` call that made Servo's own
    /// `Painter::resize_rendering_context` change-detection see "no
    /// change" and skip the repaint-at-new-size step entirely, and that
    /// also skipped `make_current()` before touching the surface — see
    /// that method's doc comment for the full trace against the pinned
    /// `libservo` source). That is now fixed directly in `session.rs`.
    /// This field and `RESIZE_SETTLE_TICKS` are kept as a small residual
    /// safety margin (one tick) rather than removed outright, since a
    /// same-tick read immediately after `resize()` is still a real,
    /// separate race in principle even with the primary bug fixed — not
    /// because it was ever the actual cause of what the user observed.
    pub resize_settle_ticks: u8,
    // ── Agent bridge ──────────────────────────────────────────────────────────
    /// The real `ferrite_model::ModelProvider` constructed at startup
    /// (`try_real_model_provider`), or `ferrite_model::MockProvider::new()`
    /// (fail-to-empty) when none is configured/reachable — T-224/T-229.
    pub model_provider: Arc<dyn ferrite_model::ModelProvider>,
    /// Configured tag for `ModelTier::Small` (the fingerprint's `may_use`
    /// prediction) — `"unconfigured"` when `model_provider` is the mock
    /// fallback, harmless since `MockProvider` ignores the tag entirely.
    pub model_tag_small: String,
    /// Configured tag for `ModelTier::Main` (the live agent loop's plan/act
    /// reasoning).
    pub model_tag_main: String,
    /// Handle to the currently in-flight background task (a dry run, or one
    /// live-loop step's model call), if any.
    pub agent_handle: Option<tokio::task::JoinHandle<()>>,
    /// Bumped on every fresh run (`AgentTaskSubmitted`, the post-consent
    /// run) and on every stop (`StopAgent`, `ConsentCancelled`). Each
    /// background step stamps the `run_id` it was spawned under onto its
    /// reply message; `update()` drops a `LiveRunReady`/`AgentStepReady`
    /// whose `run_id` no longer matches, so a step already in flight when
    /// the user hits Stop (or starts a new task) can never execute a live
    /// browser action after that point, even though `JoinHandle::abort()`
    /// cannot guarantee the in-flight future was actually cancelled before
    /// it sent its reply.
    pub run_id: u64,
    /// State of the in-progress live agent-action loop, if one is running —
    /// `None` whenever the agent is idle, in the middle of a dry run, or
    /// waiting on a pending consent decision.
    pub live_loop: Option<LiveAgentLoop>,
    // ── Agent sidebar UI ─────────────────────────────────────────────────────
    pub show_agent_sidebar: bool,
    pub agent_task_input: String,
    /// Live activity feed — one entry per dry-run status note or executed
    /// action, in order. See [`AgentLogEntry`].
    pub agent_log: Vec<AgentLogEntry>,
    pub agent_response: Option<String>,
    pub agent_is_running: bool,
    /// Sender used by spawned agent task to emit progress messages.
    pub agent_event_tx: Option<tokio::sync::mpsc::UnboundedSender<FerriteBrowserMessage>>,
    /// Receiver drained by the agent_event_sub subscription.
    pub agent_event_rx: Option<
        std::sync::Arc<
            tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<FerriteBrowserMessage>>,
        >,
    >,
    // ── IPI consent state ────────────────────────────────────────────────────
    // Every field below is scoped to exactly one pending consent decision and
    // is cleared on *both* ConsentSubmitted and ConsentCancelled — approvals
    // and rejections are per-task, never sticky across tasks. See
    // `consent_state_never_carries_into_a_second_task` for the test that
    // would fail if any of this leaked into a later task.
    /// Set when the dry run finds unexpected activity; cleared after consent
    /// or cancel.
    pub pending_diff: Option<FingerprintDiff>,
    /// The expected fingerprint `pending_diff` was compared against — kept
    /// alongside the diff so the consent panel can describe which origins
    /// *would* have been admitted, not just which ones weren't.
    pub pending_expected: Option<ExpectedFingerprint>,
    /// The dry-run record itself, kept only so the consent panel can show it
    /// on expand ("what did the agent actually do, in what order").
    pub pending_evidence: Option<DryRunRecord>,
    /// Tracks per-item approve/reject decisions while the consent panel is
    /// open, covering both `pending_diff.extra_primitives` and
    /// `pending_diff.out_of_scope_origins` (see `consent_items`).
    pub pending_decision: ConsentDecision,
    /// Original task prompt preserved between dry run and consent
    /// resolution — just the prompt (not a full `IpiTask`): the live loop
    /// always starts a fresh message history from this prompt regardless of
    /// what the dry run itself did, so nothing else needs to be carried.
    pub pending_task: Option<String>,
    /// Whether the dry-run evidence section is expanded.
    pub show_evidence: bool,
    // ── C1 design-system state ───────────────────────────────────────────────
    /// Index of the tab bar entry currently under the pointer, if any —
    /// drives the close-button-on-hover reveal pattern (`view()`'s tab bar).
    /// `None` when the pointer is not over any tab; reset on `CloseTab`
    /// since a close shifts every later tab's index, and a stale hovered
    /// index would otherwise show the close button on the wrong tab.
    pub hovered_tab: Option<usize>,
    /// Fade/slide-in progress for the consent panel, `0.0` (just appeared)
    /// to `1.0` (fully settled) — advanced by `ConsentPanelTick` while
    /// `pending_diff` is `Some` and this is below `1.0` (see
    /// `subscription()`'s `consent_anim_tick`). Reset to `0.0` whenever
    /// `pending_diff` transitions from `None` to `Some` (`ConsentRequired`),
    /// so the panel replays its entrance every time a new one appears. Pure
    /// decoration only — see `view_agent_sidebar`'s consent body for why
    /// this never delays or hides any of the panel's actual content.
    pub consent_panel_anim: f32,
    // ── C3c: theme ────────────────────────────────────────────────────────
    /// Which palette `view()`/`new_tab_page()`/`view_agent_sidebar()`
    /// currently draw from, and which `iced::Theme` `launch()`'s
    /// `.theme(...)` closure returns — see [`AppTheme`]. Toggled by
    /// `ToggleTheme`, e.g. the toolbar's sun/moon button.
    pub theme_mode: AppTheme,
}

impl FerriteBrowser {
    /// The resolved colour set for `self.theme_mode` — the one call every
    /// view function that needs colours makes, once, near its own top (see
    /// this module's "Colour palette" section header for the full
    /// threading story).
    pub fn palette(&self) -> &'static Palette {
        self.theme_mode.palette()
    }
}

impl Default for FerriteBrowser {
    fn default() -> Self {
        let (agent_event_tx, agent_event_rx) =
            tokio::sync::mpsc::unbounded_channel::<FerriteBrowserMessage>();
        // Test-safe by construction (R7, matching
        // `ferrite-eval::harness::try_real_provider()`'s own convention:
        // never called from a `Default`/test-reachable path — only from the
        // real app entry point, `launch()` below, which overwrites these
        // three fields with a real provider when one is configured/reachable
        // *before* the window ever opens). Every `FerriteBrowser::default()`
        // in this crate's own test suite therefore never touches the
        // environment or the OS keyring at all, let alone the network.
        let model_provider: Arc<dyn ferrite_model::ModelProvider> =
            Arc::new(ferrite_model::MockProvider::new());
        let model_tag_small = "unconfigured".to_string();
        let model_tag_main = "unconfigured".to_string();
        Self {
            tabs: vec!["New Tab".to_string()],
            active_tab: 0,
            address_bar_input: String::new(),
            tab_urls: vec!["about:blank".to_string()],
            show_audit_panel: false,
            show_js_console: false,
            audit_entries: vec![],
            servo_sessions: HashMap::new(),
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            progress_offset: 0.0,
            address_bar_focused: false,
            tab_error: vec![None],
            tab_titles: vec!["New Tab".to_string()],
            tab_favicons: vec![None],
            new_tab_search_input: String::new(),
            js_input: String::new(),
            js_output: Vec::new(),
            cursor_pos: (0.0, 0.0),
            scale_factor: 1.0,
            content_area_size: Cell::new(Size::new(1280.0, 700.0)),
            last_resized_content_px: (1280, 700),
            resize_settle_ticks: 0,
            model_provider,
            model_tag_small,
            model_tag_main,
            agent_handle: None,
            run_id: 0,
            live_loop: None,
            show_agent_sidebar: false,
            agent_task_input: String::new(),
            agent_log: Vec::new(),
            agent_response: None,
            agent_is_running: false,
            agent_event_tx: Some(agent_event_tx),
            agent_event_rx: Some(std::sync::Arc::new(tokio::sync::Mutex::new(agent_event_rx))),
            pending_diff: None,
            pending_expected: None,
            pending_evidence: None,
            pending_decision: ConsentDecision::default(),
            pending_task: None,
            show_evidence: false,
            hovered_tab: None,
            consent_panel_anim: 0.0,
            theme_mode: AppTheme::Dark,
        }
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FerriteBrowserMessage {
    AddTab,
    CloseTab(usize),
    SelectTab(usize),
    AddressBarChanged(String),
    NavigateRequested(String),
    ToggleAuditPanel,
    ToggleJsConsole,
    RefreshAuditLog,
    ServoReady,
    ServoFrame,
    GoBack,
    GoForward,
    Reload,
    StopLoading,
    LoadStatusChanged {
        tab: usize,
        status: String,
        url: String,
    },
    FocusAddressBar,
    ClearAddressBarFocus,
    CloseActiveTab,
    EscapePressed,
    NewTabSearchChanged(String),
    JsInputChanged(String),
    JsExecuteRequested,
    JsConsoleClear,
    // Mouse/scroll events forwarded to Servo
    /// Mouse moved over the content area — position is relative to content area origin.
    ServoMouseMove {
        x: f32,
        y: f32,
    },
    /// Mouse button pressed (position taken from last ServoMouseMove).
    ServoMousePress,
    /// Mouse button released (position taken from last ServoMouseMove).
    ServoMouseRelease,
    /// Scroll wheel event.
    ServoScroll {
        delta_x: f32,
        delta_y: f32,
    },
    /// The window's real scale factor (physical px per logical point),
    /// fetched once shortly after launch — see `scale_factor`'s doc
    /// comment on `FerriteBrowser`.
    ScaleFactorReady(f32),
    // ── Live agent loop (T-224) ──────────────────────────────────────────────
    /// The dry run found nothing unexpected (or the defense mode bypassed
    /// it entirely) — begin the live loop from scratch with `prompt`. Not
    /// sent for the post-consent run, which `ConsentSubmitted` starts
    /// directly (it already has the prompt and the user's decision on
    /// hand, no round trip needed).
    LiveRunReady {
        run_id: u64,
        prompt: String,
    },
    /// One step's background model call returned the next `AgentAction` to
    /// take (or `Err` if the model call itself failed or was unparseable
    /// — see `StepFailure`).
    AgentStepReady {
        run_id: u64,
        action: Result<AgentAction, StepFailure>,
    },
    // ── Agent sidebar ─────────────────────────────────────────────────────────
    ToggleAgentSidebar,
    AgentTaskInputChanged(String),
    AgentTaskSubmitted,
    AgentToolLogged(String),
    AgentCompleted(String),
    AgentFailed(String),
    StopAgent,
    // ── IPI consent ───────────────────────────────────────────────────────────
    ConsentRequired {
        diff: FingerprintDiff,
        expected: ExpectedFingerprint,
        // Boxed solely to keep this enum's largest variant small (clippy
        // large_enum_variant) — every other variant is a handful of bytes,
        // and DryRunRecord's several Vec/HashSet fields make it the outlier.
        evidence: Box<DryRunRecord>,
    },
    /// `id` is either a real `ToolId` wire string (an `extra_primitives`
    /// item) or an `origin_item_id`-prefixed synthetic id (an
    /// `out_of_scope_origins` item) — see `consent_items`.
    ApproveTool(String),
    RejectTool(String),
    ToggleEvidence,
    ConsentSubmitted,
    ConsentCancelled,
    // ── C1 design-system messages ────────────────────────────────────────────
    /// Pointer entered tab `usize`'s hit area — drives the tab bar's
    /// close-button-on-hover reveal. Does not change tab selection/order/
    /// closing (`SelectTab`/`CloseTab`/`AddTab` are unchanged).
    TabHoverEnter(usize),
    /// Pointer left tab `usize`'s hit area.
    TabHoverExit(usize),
    /// One animation-subscription tick advancing
    /// `FerriteBrowser::consent_panel_anim` — see that field's docs and
    /// `subscription()`'s `consent_anim_tick`.
    ConsentPanelTick,
    // ── C3c: theme ────────────────────────────────────────────────────────
    /// Flips `FerriteBrowser::theme_mode` between `AppTheme::Dark` and
    /// `AppTheme::Light` — sent by the toolbar's sun/moon button.
    ToggleTheme,
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

pub fn update(
    state: &mut FerriteBrowser,
    message: FerriteBrowserMessage,
) -> Task<FerriteBrowserMessage> {
    match message {
        FerriteBrowserMessage::AddTab => {
            state.tabs.push("New Tab".to_string());
            state.tab_urls.push("about:blank".to_string());
            state.tab_error.push(None);
            state.tab_titles.push("New Tab".to_string());
            state.tab_favicons.push(None);
            let new_idx = state.tabs.len() - 1;
            state.active_tab = new_idx;
            state.address_bar_input = String::new();
            state.is_loading = false;
            state.can_go_back = false;
            state.can_go_forward = false;
            match HeadlessServoSession::new(1280, 700) {
                Ok(session) => {
                    state.servo_sessions.insert(new_idx, session);
                }
                Err(e) => eprintln!("[ferrite-ui] Servo session tab {}: {}", new_idx, e),
            }
        }
        FerriteBrowserMessage::CloseTab(i) => {
            // Every later tab's index shifts by one — a stale hovered index
            // would otherwise show the close-on-hover button on the wrong
            // tab until the next real hover event.
            state.hovered_tab = None;
            if state.tabs.len() > 1 {
                state.tabs.remove(i);
                state.tab_urls.remove(i);
                if i < state.tab_error.len() {
                    state.tab_error.remove(i);
                }
                if i < state.tab_titles.len() {
                    state.tab_titles.remove(i);
                }
                if i < state.tab_favicons.len() {
                    state.tab_favicons.remove(i);
                }
                state.servo_sessions.remove(&i);
                let keys_to_shift: Vec<usize> = state
                    .servo_sessions
                    .keys()
                    .copied()
                    .filter(|&k| k > i)
                    .collect();
                for k in keys_to_shift {
                    if let Some(session) = state.servo_sessions.remove(&k) {
                        state.servo_sessions.insert(k - 1, session);
                    }
                }
            }
            state.active_tab = state.active_tab.min(state.tabs.len().saturating_sub(1));
            state.address_bar_input = state.tab_urls[state.active_tab].clone();
            sync_nav_state(state);
        }
        FerriteBrowserMessage::SelectTab(i) => {
            state.active_tab = i;
            state.address_bar_input = state.tab_urls[i].clone();
            sync_nav_state(state);
        }
        FerriteBrowserMessage::AddressBarChanged(s) => {
            state.address_bar_input = s;
        }
        FerriteBrowserMessage::NavigateRequested(raw) => {
            let url = resolve_url(&raw);
            state.address_bar_input = url.clone();
            state.tab_urls[state.active_tab] = url.clone();
            if state.active_tab < state.tab_error.len() {
                state.tab_error[state.active_tab] = None;
            }
            state.new_tab_search_input = String::new();
            state.is_loading = true;
            state.address_bar_focused = false;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.navigate(&url);
            }
        }
        FerriteBrowserMessage::GoBack => {
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.go_back();
            }
            state.is_loading = true;
        }
        FerriteBrowserMessage::GoForward => {
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.go_forward();
            }
            state.is_loading = true;
        }
        FerriteBrowserMessage::Reload => {
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.reload();
            }
            state.is_loading = true;
        }
        FerriteBrowserMessage::StopLoading => {
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.stop();
            }
            state.is_loading = false;
        }
        FerriteBrowserMessage::LoadStatusChanged { tab, status, url } => {
            state.is_loading = status == "loading";
            if status == "failed" {
                if tab < state.tab_error.len() {
                    state.tab_error[tab] = Some(url.clone());
                }
            } else {
                if tab < state.tab_error.len() {
                    state.tab_error[tab] = None;
                }
                if !url.is_empty() && url != state.address_bar_input {
                    state.address_bar_input = url.clone();
                }
                if tab < state.tab_urls.len() && !url.is_empty() {
                    state.tab_urls[tab] = url;
                }
            }
        }
        FerriteBrowserMessage::ToggleAuditPanel => {
            state.show_audit_panel = !state.show_audit_panel;
            if state.show_audit_panel {
                state.show_js_console = false;
            }
        }
        FerriteBrowserMessage::ToggleJsConsole => {
            state.show_js_console = !state.show_js_console;
            if state.show_js_console {
                state.show_audit_panel = false;
            }
        }
        FerriteBrowserMessage::RefreshAuditLog => {
            let db_path = std::env::temp_dir()
                .join("ferrite_sandbox.db")
                .to_string_lossy()
                .into_owned();
            state.audit_entries = match PersistentAuditLog::load(&db_path) {
                Ok(log) => log.log.entries,
                Err(e) => {
                    eprintln!("[ferrite-ui] audit log load: {}", e);
                    vec![]
                }
            };
        }
        FerriteBrowserMessage::NewTabSearchChanged(s) => {
            state.new_tab_search_input = s;
        }
        FerriteBrowserMessage::FocusAddressBar => {
            state.address_bar_focused = true;
            return text_input::focus(text_input::Id::new(ADDRESS_BAR_ID));
        }
        FerriteBrowserMessage::ClearAddressBarFocus => {
            state.address_bar_focused = false;
        }
        FerriteBrowserMessage::CloseActiveTab => {
            let i = state.active_tab;
            return Task::done(FerriteBrowserMessage::CloseTab(i));
        }
        FerriteBrowserMessage::EscapePressed => {
            if state.is_loading {
                return Task::done(FerriteBrowserMessage::StopLoading);
            } else {
                state.address_bar_focused = false;
            }
        }
        FerriteBrowserMessage::JsInputChanged(s) => {
            state.js_input = s;
        }
        FerriteBrowserMessage::JsConsoleClear => {
            state.js_output.clear();
        }
        FerriteBrowserMessage::JsExecuteRequested => {
            let script = state.js_input.trim().to_string();
            if script.is_empty() {
                return Task::none();
            }
            state.js_input.clear();

            let result = if let Some(session) = state.servo_sessions.get_mut(&state.active_tab) {
                match session.execute_js(&script) {
                    Ok(v) => v,
                    Err(e) => format!("Error: {}", e),
                }
            } else {
                "Error: no active Servo session".to_string()
            };

            let snippet = if script.len() > 60 {
                format!("{}...", &script[..59])
            } else {
                script
            };
            state.js_output.push((snippet, result));
        }
        // ── Servo mouse/scroll events ──────────────────────────────────────
        FerriteBrowserMessage::ServoMouseMove { x, y } => {
            state.cursor_pos = (x, y);
            let scale = state.scale_factor;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                // mouse_area reports logical points; HeadlessServoSession's
                // DevicePoint space is physical pixels — see scale_factor's
                // doc comment. Without this, a HiDPI display (or any window
                // size other than the session's hardcoded original buffer)
                // put the cursor Servo actually sees somewhere other than
                // where it visually is, which is exactly the "have to hover
                // above the link" bug this scaling fixes (C3a).
                session.send_mouse_move(x * scale, y * scale);
            }
        }
        FerriteBrowserMessage::ServoMousePress => {
            let (x, y) = state.cursor_pos;
            let scale = state.scale_factor;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.send_mouse_down(x * scale, y * scale);
            }
        }
        FerriteBrowserMessage::ServoMouseRelease => {
            let (x, y) = state.cursor_pos;
            let scale = state.scale_factor;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                // Up event first, then a synthesised click for hit-testing.
                session.send_mouse_up(x * scale, y * scale);
                session.send_mouse_click(x * scale, y * scale);
            }
        }
        FerriteBrowserMessage::ServoScroll { delta_x, delta_y } => {
            let (x, y) = state.cursor_pos;
            let scale = state.scale_factor;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                // Only the position is scaled to physical pixels, matching
                // every other pointer event above — delta_x/delta_y are
                // already OS-level wheel/trackpad units, independent of
                // display scale, and left as iced reports them.
                session.send_scroll(x * scale, y * scale, delta_x as f64, delta_y as f64);
            }
        }
        FerriteBrowserMessage::ScaleFactorReady(factor) => {
            state.scale_factor = factor;
        }
        // ── Agent sidebar ─────────────────────────────────────────────────────
        FerriteBrowserMessage::ToggleAgentSidebar => {
            state.show_agent_sidebar = !state.show_agent_sidebar;
        }
        FerriteBrowserMessage::AgentTaskInputChanged(s) => {
            state.agent_task_input = s;
        }
        FerriteBrowserMessage::AgentTaskSubmitted => {
            if state.agent_is_running {
                return Task::none();
            }
            state.agent_log.clear();
            state.agent_response = None;
            state.agent_is_running = true;
            state.run_id += 1;
            let run_id = state.run_id;

            let prompt = state.agent_task_input.clone();
            let context_url = state.tab_urls.get(state.active_tab).cloned();
            let ipi_task = IpiTask::new(prompt.clone(), context_url.clone());
            state.pending_task = Some(prompt.clone());

            let provider = state.model_provider.clone();
            let model_tag_small = state.model_tag_small.clone();
            let model_tag_main = state.model_tag_main.clone();
            let event_tx = match state.agent_event_tx.clone() {
                Some(tx) => tx,
                None => return Task::none(),
            };

            let handle = tokio::task::spawn(async move {
                // ── Defense-mode single decision point (Task 18) ──────────────
                // ToolDecisionEngine::new() reads FERRITE_DEFENSE once; On is the
                // unchanged default everywhere. Off skips straight to the real run
                // (no sanitizer, no dry-run, no consent). SanitizerOnly runs the
                // sanitizer but also skips straight to the real run. On runs the
                // sanitizer and continues into the existing fingerprint/dry-run/
                // compare/consent loop, unchanged.
                let engine = ToolDecisionEngine::new();
                let loop_outcome = engine.prepare_task(&ipi_task);
                let defense_mode = match &loop_outcome {
                    LoopOutcome::Bypassed | LoopOutcome::RanSanitizerOnly { .. } => {
                        let _ = event_tx.send(FerriteBrowserMessage::LiveRunReady {
                            run_id,
                            prompt: prompt.clone(),
                        });
                        return;
                    }
                    // LoopOnly and On both continue into the fingerprint/dry-run/
                    // compare/consent loop below. They differ only in whether the
                    // sanitizer ran first (On) or was bypassed (LoopOnly) — a
                    // distinction the dry-run orchestrator now acts on directly
                    // (T-215: set_defense_mode below), so both fall through here.
                    LoopOutcome::RanLoopOnly => DefenseMode::LoopOnly,
                    LoopOutcome::RanFullLoop { .. } => DefenseMode::On,
                };

                // ── IPI dry run ──────────────────────────────────────────────
                // T-224/T-229: `provider` is the real, live-configured
                // ModelProvider constructed at startup (or MockProvider,
                // fail-to-empty, if none is configured/reachable) — no
                // longer a hardcoded MockProvider::new() regardless of what
                // is actually available (T-229's exact fix).
                let fingerprint = engine
                    .fingerprint_from_task(provider.as_ref(), &model_tag_small, &ipi_task)
                    .await;
                let twin_path = std::env::temp_dir().join("ferrite-ipi-twin.enc");
                let mut orch = ferrite_ipi::dry_run::DryRunOrchestrator::new(twin_path);
                // T-215: derive detect_enabled AND strip_enabled together from
                // the mode this dry run is actually running under, instead of
                // leaving both at DryRunOrchestrator::new's defaults
                // (detect-only, strip off) regardless of mode.
                orch.set_defense_mode(defense_mode);
                // BrowserLoopDryRunDriver runs the real
                // `browser_loop::run_agent_loop` directly against the
                // synthetic `DryRunEngine` — see that type's own docs for
                // why this needs no GeminiAgent/EngineToolExecutor bridge
                // now that neither this loop nor the dry run's engine
                // depends on the old vocabulary.
                let driver = BrowserLoopDryRunDriver {
                    provider: provider.as_ref(),
                    model_tag: model_tag_main.clone(),
                    prompt: prompt.clone(),
                };
                let dry_record = match orch.run(&ipi_task, &driver).await {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = event_tx.send(FerriteBrowserMessage::AgentFailed(format!(
                            "dry run failed: {}",
                            e
                        )));
                        return;
                    }
                };
                let _ = event_tx.send(FerriteBrowserMessage::AgentToolLogged(
                    "[dry run complete — checking for unexpected activity]".to_string(),
                ));
                // No per-task per-capability origin-scope authoring exists yet
                // (T-001's live-path bridge, see ferrite_ipi::comparator's
                // module docs). The task's own declared context URL — already
                // threaded in above as `context_url` — narrows every
                // capability to an exact scope on it; task_open (which admits
                // any origin) is used only when no context URL is known at
                // all, never unconditionally.
                let context_origin = context_url
                    .as_deref()
                    .and_then(|url| ferrite_core::Origin::parse(url).ok());
                let expected =
                    ExpectedFingerprint::from_fingerprint(&fingerprint, context_origin.as_ref());
                let diff = compare(&expected, &dry_record);
                if !diff.is_clean() {
                    let _ = event_tx.send(FerriteBrowserMessage::ConsentRequired {
                        diff,
                        expected,
                        evidence: Box::new(dry_record),
                    });
                    return;
                }

                // ── Real run ─────────────────────────────────────────────────
                let _ = event_tx.send(FerriteBrowserMessage::LiveRunReady { run_id, prompt });
            });
            state.agent_handle = Some(handle);
        }
        FerriteBrowserMessage::AgentToolLogged(s) => {
            state.agent_log.push(AgentLogEntry::Note(s));
        }
        FerriteBrowserMessage::AgentCompleted(s) => {
            state.agent_response = Some(s);
            state.agent_is_running = false;
        }
        FerriteBrowserMessage::AgentFailed(s) => {
            state.agent_response = Some(format!("[error] {}", s));
            state.agent_is_running = false;
        }
        FerriteBrowserMessage::StopAgent => {
            state.run_id += 1;
            if let Some(handle) = state.agent_handle.take() {
                handle.abort();
            }
            state.live_loop = None;
            state.agent_is_running = false;
        }
        // ── IPI consent handlers ──────────────────────────────────────────────
        FerriteBrowserMessage::ConsentRequired {
            diff,
            expected,
            evidence,
        } => {
            state.pending_diff = Some(diff);
            state.pending_expected = Some(expected);
            state.pending_evidence = Some(*evidence);
            state.pending_decision = ConsentDecision::default();
            state.show_evidence = false;
            state.agent_is_running = false;
            // pending_diff just transitioned None -> Some: (re)start the
            // panel's entrance animation from the beginning (C1).
            state.consent_panel_anim = 0.0;
        }
        FerriteBrowserMessage::ApproveTool(id) => {
            state.pending_decision.approve(ToolId::new(&id));
        }
        FerriteBrowserMessage::RejectTool(id) => {
            state.pending_decision.reject(ToolId::new(&id));
        }
        FerriteBrowserMessage::ToggleEvidence => {
            state.show_evidence = !state.show_evidence;
        }
        FerriteBrowserMessage::ConsentSubmitted => {
            // Defense in depth: the view only ever emits this message once
            // `consent_is_complete` holds (the Proceed button is disabled
            // otherwise), but this handler re-checks it directly rather than
            // trusting the view — ConsentSubmitted must never be reachable
            // with an undecided item, from any caller.
            let Some(diff) = state.pending_diff.as_ref() else {
                return Task::none();
            };
            if !consent_is_complete(diff, &state.pending_decision) {
                return Task::none();
            }

            let all_rejected = state.pending_decision.rejected.clone();
            let rejected_origins: std::collections::HashSet<String> = all_rejected
                .iter()
                .filter_map(|t| origin_item_origin(t).map(str::to_string))
                .collect();
            let rejected: std::collections::HashSet<ToolId> = all_rejected
                .into_iter()
                .filter(|t| origin_item_origin(t).is_none())
                .collect();

            // Every field scoped to this consent decision is cleared here —
            // approvals/rejections are per-task, never sticky across tasks.
            state.pending_diff = None;
            state.pending_expected = None;
            state.pending_evidence = None;
            state.pending_decision = ConsentDecision::default();
            state.show_evidence = false;

            let prompt = match state.pending_task.take() {
                Some(p) => p,
                None => return Task::none(),
            };
            state.run_id += 1;
            let run_id = state.run_id;
            start_live_loop(state, run_id, prompt, rejected, rejected_origins);
        }
        FerriteBrowserMessage::ConsentCancelled => {
            // Same clearing as ConsentSubmitted — cancelling must leave no
            // trace of this task's pending decision behind either.
            state.pending_diff = None;
            state.pending_expected = None;
            state.pending_evidence = None;
            state.pending_decision = ConsentDecision::default();
            state.pending_task = None;
            state.show_evidence = false;
            state.agent_is_running = false;
            state.run_id += 1;
        }
        // ── Live agent loop (T-224) ──────────────────────────────────────────
        FerriteBrowserMessage::LiveRunReady { run_id, prompt } => {
            if run_id != state.run_id {
                return Task::none();
            }
            start_live_loop(
                state,
                run_id,
                prompt,
                std::collections::HashSet::new(),
                std::collections::HashSet::new(),
            );
        }
        FerriteBrowserMessage::AgentStepReady { run_id, action } => {
            if run_id != state.run_id {
                return Task::none();
            }
            let Some(mut live) = state.live_loop.take() else {
                return Task::none();
            };

            let action = match action {
                Ok(a) => {
                    live.consecutive_malformed = 0;
                    a
                }
                Err(StepFailure::Model(reason)) => {
                    state.agent_response = Some(format!("[error] {reason}"));
                    state.agent_is_running = false;
                    return Task::none();
                }
                Err(StepFailure::Malformed { raw, message }) => {
                    live.consecutive_malformed += 1;
                    if live.consecutive_malformed > MAX_CONSECUTIVE_MALFORMED_STEPS {
                        state.agent_response = Some(format!("[error] {message} (raw: {raw})"));
                        state.agent_is_running = false;
                        return Task::none();
                    }
                    // Give the model a chance to self-correct — the same
                    // retry-with-feedback shape `browser_loop::
                    // run_agent_loop` uses, and for the same reason: a
                    // truncated or malformed response is often a one-off
                    // glitch (e.g. a `finish.answer` cut short by the
                    // provider's output cap) a model recovers from once
                    // told what was wrong, so ending the whole task on the
                    // first one turned a recoverable hiccup into a hard
                    // failure. Not counted against `budget.max_steps` — no
                    // real browser action was taken — but independently
                    // bounded by `MAX_CONSECUTIVE_MALFORMED_STEPS` above.
                    live.messages
                        .push(Message::assistant(compact_observation(raw.trim())));
                    live.messages.push(Message::user(format!(
                        "Observation: your last response could not be parsed as a single, \
                         complete, valid JSON action ({message}). It may have been cut off \
                         or included extra text. Respond with EXACTLY ONE complete, valid \
                         JSON object and nothing else."
                    )));
                    trim_message_history(&mut live.messages);
                    spawn_next_step(state, run_id, live);
                    return Task::none();
                }
            };

            if let AgentAction::Finish { answer } = action {
                state.agent_response = Some(answer);
                state.agent_is_running = false;
                return Task::none();
            }

            // Repeated-identical-action hard stop — mirrors
            // `browser_loop::run_agent_loop`'s own check exactly (fires
            // *before* executing the would-be Nth repeat).
            if live.budget.max_repeated_identical > 0 {
                let window = live.budget.max_repeated_identical - 1;
                if window <= live.actions_taken.len()
                    && live.actions_taken[live.actions_taken.len() - window..]
                        .iter()
                        .all(|a| a == &action)
                {
                    state.agent_response =
                        Some("[stopped: the same action was about to repeat]".to_string());
                    state.agent_is_running = false;
                    return Task::none();
                }
            }

            let blocked = is_action_rejected(&action, &live.rejected, &live.rejected_origins);
            let observation = if blocked {
                "blocked by user consent".to_string()
            } else if let Some(session) = state.servo_sessions.get_mut(&state.active_tab) {
                let mut engine = BorrowedServoEngine::new(session, 1280, 700);
                execute_action(&mut engine, &action)
            } else {
                "error: no active browser session".to_string()
            };

            state.agent_log.push(AgentLogEntry::Step {
                icon: icon_for_action(&action),
                label: action_label(&action),
                detail: action_detail(&action),
                result: observation.clone(),
                blocked,
            });
            live.messages.push(Message::assistant(
                serde_json::to_string(&action).unwrap_or_default(),
            ));
            let compacted_observation = compact_observation(&observation);
            live.messages.push(Message::user(format!(
                "Observation: {compacted_observation}"
            )));
            trim_message_history(&mut live.messages);
            live.actions_taken.push(action);

            spawn_next_step(state, run_id, live);
        }
        // ── Agent bridge ──────────────────────────────────────────────────────
        FerriteBrowserMessage::ServoReady => {}
        FerriteBrowserMessage::ServoFrame => {
            state.progress_offset = (state.progress_offset + 0.02) % 1.0;

            // Keep the active tab's Servo render buffer matched to the real
            // content-area size (see `content_area_size`'s doc comment).
            // Runs every tick (16ms) rather than off a dedicated resize
            // event, so it also picks up a size change caused by toggling
            // the agent sidebar or the audit/JS panel, not only a window
            // resize — those never fire a window-level resize event at
            // all. Skipped entirely while a previous resize is still
            // settling (`resize_settle_ticks > 0`) — see that field's doc
            // comment for the real crash this avoids; `resize()` is never
            // called again until the settle window from the last one has
            // fully elapsed.
            if state.resize_settle_ticks == 0 {
                let logical = state.content_area_size.get();
                let scale = state.scale_factor;
                let desired = (
                    (logical.width * scale).round().max(1.0) as u32,
                    (logical.height * scale).round().max(1.0) as u32,
                );
                if desired != state.last_resized_content_px {
                    if let Some(session) = state.servo_sessions.get_mut(&state.active_tab) {
                        session.resize(desired.0, desired.1);
                        state.last_resized_content_px = desired;
                        state.resize_settle_ticks = RESIZE_SETTLE_TICKS;
                    }
                }
            }

            // Pump engine once, then sync every tab's state and read pixels
            // — unless a resize just fired this tick (see above), in which
            // case every session's frame is skipped for `resize_settle_ticks`
            // more ticks rather than read against a buffer libservo may not
            // have finished reallocating yet. The previous frame stays
            // displayed in the meantime — a few tens of ms of an unchanged
            // image, not a black/corrupted one.
            if let Some(first) = state.servo_sessions.values().next() {
                first.pump_engine();
            }
            if state.resize_settle_ticks > 0 {
                state.resize_settle_ticks -= 1;
            } else {
                for session in state.servo_sessions.values_mut() {
                    session.sync_and_read();
                }
            }
            let active = state.active_tab;
            if let Some(session) = state.servo_sessions.get(&active) {
                state.can_go_back = session.can_go_back();
                state.can_go_forward = session.can_go_forward();
                if let Some(title) = session.page_title() {
                    if active < state.tab_titles.len() && !title.is_empty() {
                        state.tab_titles[active] = title.to_string();
                    }
                }
                if let Some((w, h, bytes)) = session.get_favicon() {
                    if active < state.tab_favicons.len() {
                        state.tab_favicons[active] = Some(ImageHandle::from_rgba(w, h, bytes));
                    }
                }
                let is_now_loading = matches!(session.load_status(), LoadStatus::Loading);
                let new_url = session.current_url().to_string();
                let prev_url = state.tab_urls.get(active).cloned().unwrap_or_default();
                let status_changed = is_now_loading != state.is_loading;
                let url_changed =
                    !new_url.is_empty() && new_url != "about:blank" && new_url != prev_url;
                if status_changed || url_changed {
                    let status = if is_now_loading {
                        "loading"
                    } else {
                        "complete"
                    }
                    .to_string();
                    return Task::done(FerriteBrowserMessage::LoadStatusChanged {
                        tab: active,
                        status,
                        url: new_url,
                    });
                }
            }
        }
        // ── C1 design-system messages ────────────────────────────────────────
        FerriteBrowserMessage::TabHoverEnter(i) => {
            state.hovered_tab = Some(i);
        }
        FerriteBrowserMessage::TabHoverExit(i) => {
            if state.hovered_tab == Some(i) {
                state.hovered_tab = None;
            }
        }
        FerriteBrowserMessage::ConsentPanelTick => {
            state.consent_panel_anim = (state.consent_panel_anim + CONSENT_ANIM_STEP).min(1.0);
        }
        // ── C3c: theme ───────────────────────────────────────────────────────
        FerriteBrowserMessage::ToggleTheme => {
            state.theme_mode = state.theme_mode.toggled();
        }
    }
    Task::none()
}

fn sync_nav_state(state: &mut FerriteBrowser) {
    let active = state.active_tab;
    if let Some(session) = state.servo_sessions.get(&active) {
        state.is_loading = matches!(session.load_status(), LoadStatus::Loading);
        state.can_go_back = session.can_go_back();
        state.can_go_forward = session.can_go_forward();
    } else {
        state.is_loading = false;
        state.can_go_back = false;
        state.can_go_forward = false;
    }
}

// ---------------------------------------------------------------------------
// Consent panel — the security surface. Plain-English item summaries, the
// completeness check that gates ConsentSubmitted, and dry-run evidence
// rendering all live here as pure functions of a `FingerprintDiff` /
// `ExpectedFingerprint` / `DryRunRecord`, so they're testable without
// spinning up Iced at all (see the `tests` module at the bottom of this
// file).
// ---------------------------------------------------------------------------

/// Prefix distinguishing a synthetic `out_of_scope_origins` item id from a
/// real `ToolId` wire string (e.g. `"js.execute"`, `"dom.read"` never contain
/// `"::"`). `ConsentDecision` is keyed by `ToolId` alone (A7's type,
/// unmodified — this charter does not touch `ferrite-ipi`), so an
/// out-of-scope-origin item — which has no `ToolId` of its own, only an
/// origin string — borrows that same key space under this prefix rather than
/// requiring a second, parallel decision-tracking type.
const ORIGIN_ITEM_PREFIX: &str = "origin::";

/// The synthetic item id an out-of-scope-origin flagged item is tracked
/// under in `ConsentDecision::{approved,rejected}`.
fn origin_item_id(origin: &str) -> ToolId {
    ToolId::new(&format!("{ORIGIN_ITEM_PREFIX}{origin}"))
}

/// The reverse of [`origin_item_id`]: `Some(origin)` if `id` is a synthetic
/// origin item id, `None` if it's a real tool id.
fn origin_item_origin(id: &ToolId) -> Option<&str> {
    id.0.strip_prefix(ORIGIN_ITEM_PREFIX)
}

/// One flagged item ready for consent review: a stable identifier (used to
/// key the approve/reject decision, and to key the Iced widget row) and the
/// plain-English summary shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConsentItem {
    id: ToolId,
    summary: String,
}

/// Builds the plain-English per-item summaries for every flagged entry in
/// `diff` — both `extra_primitives` and `out_of_scope_origins` (the gap this
/// session closes: the old rendering only ever iterated `extra_primitives`).
///
/// Iteration order is deterministic (primitives sorted by tool id string,
/// then origins sorted by origin string) so this function's output — and
/// therefore the consent panel's rendering — is stable across runs, which is
/// what makes `consent_summary_snapshot_for_a_mixed_diff` a meaningful
/// regression test rather than a flaky one.
fn consent_items(
    diff: &FingerprintDiff,
    expected: Option<&ExpectedFingerprint>,
) -> Vec<ConsentItem> {
    let mut items = Vec::new();

    let mut extras: Vec<&ToolId> = diff.extra_primitives.iter().collect();
    extras.sort_by(|a, b| a.0.cmp(&b.0));
    for tool in extras {
        let summary = if tool.0 == "js.execute" {
            format!(
                "Used tool: {tool} — arbitrary JavaScript execution is always reviewed by \
                 design (it can synthesize any other action); nothing in your request could \
                 have authorized it."
            )
        } else {
            format!("Used tool: {tool} — nothing in your request authorized this action.")
        };
        items.push(ConsentItem {
            id: tool.clone(),
            summary,
        });
    }

    let mut origins: Vec<&String> = diff.out_of_scope_origins.iter().collect();
    origins.sort();
    for origin in origins {
        let allowed = expected
            .map(describe_authorized_origins)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "nothing in your request authorized any origin".to_string());
        let summary = format!(
            "Contacted {origin} — this origin is not authorized. Your request authorized: \
             {allowed}."
        );
        items.push(ConsentItem {
            id: origin_item_id(origin),
            summary,
        });
    }

    items
}

/// Plain-English description of every origin scope the expected fingerprint
/// carries, across all capabilities — the "which origin(s) would have been
/// fine" half of the directive's required summary.
///
/// This is coarser than per-primitive precision: `FingerprintDiff::out_of_scope_origins`
/// (A7's type, `ferrite_ipi::comparator::diff`) records only the offending
/// *origin* string, not which capability's realization the primitive at that
/// origin belonged to — so this function honestly reports every scope in the
/// expected set, not "the one scope that would have admitted this specific
/// primitive" (which the diff does not carry enough information to
/// determine). Documented as a known limitation in this session's
/// `docs/TO-DO.md` entry rather than papered over.
fn describe_authorized_origins(expected: &ExpectedFingerprint) -> String {
    let mut parts: Vec<String> = expected
        .lowered()
        .into_iter()
        .map(|(_, scope, _)| describe_scope(scope))
        .collect();
    parts.sort();
    parts.dedup();
    parts.join("; ")
}

fn describe_scope(scope: &ferrite_core::scope::OriginScope) -> String {
    use ferrite_core::scope::OriginScope;
    match scope {
        OriginScope::Exact(origins) => origins
            .iter()
            .map(ferrite_core::Origin::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        OriginScope::DomainSuffix(suffixes) => suffixes
            .iter()
            .map(|s| format!("*.{s}"))
            .collect::<Vec<_>>()
            .join(", "),
        OriginScope::TaskOpen { .. } => "any origin (open task)".to_string(),
    }
}

/// Whether every flagged item in `diff` — both buckets — has an explicit
/// approve or reject decision. This is the *only* gate `ConsentSubmitted`
/// may run behind; it replaces the pre-existing `ConsentDecision::is_complete`
/// call (which only ever checked `extra_primitives`) precisely because that
/// was the rendering/enforcement gap this session closes.
fn consent_is_complete(diff: &FingerprintDiff, decision: &ConsentDecision) -> bool {
    let decided = |id: &ToolId| decision.approved.contains(id) || decision.rejected.contains(id);
    diff.extra_primitives.iter().all(decided)
        && diff
            .out_of_scope_origins
            .iter()
            .all(|origin| decided(&origin_item_id(origin)))
}

/// Renders the dry-run evidence — what the agent actually did, in call
/// order — from a real `DryRunRecord`. Honestly scoped to what
/// `DryRunRecord` actually carries: the ordered (tool, origin) call log and
/// a sanitizer-finding count. It does **not** show page content, because
/// `DryRunRecord` does not record page content read — only which tool ran,
/// at which origin, in what order, and which sanitizer patterns fired.
fn dry_run_evidence_lines(record: &DryRunRecord) -> Vec<String> {
    let mut lines: Vec<String> = record
        .events_with_seq()
        .map(|(seq, event)| {
            format!(
                "#{seq}  {tool}  {origin}",
                tool = event.primitive.as_str(),
                origin = event.origin.as_deref().unwrap_or("(no origin recorded)")
            )
        })
        .collect();
    if !record.sanitizer_findings.is_empty() {
        lines.push(format!(
            "{} sanitizer finding(s) recorded during the dry run",
            record.sanitizer_findings.len()
        ));
    }
    if lines.is_empty() {
        lines.push("No tool calls were recorded during the dry run.".to_string());
    }
    lines
}

/// Ease-out-cubic easing curve, mapping linear progress `t` (`0.0..=1.0`,
/// clamped) to eased progress — accelerates out of the start rather than
/// moving at a constant rate, the standard curve for a short UI entrance
/// transition. Used by `view_agent_sidebar`'s consent panel to turn
/// `FerriteBrowser::consent_panel_anim`'s linear tick-driven progress into
/// the panel's actual background-alpha/slide-offset animation.
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

// ---------------------------------------------------------------------------
// C3c: loading-indicator animation
// ---------------------------------------------------------------------------
//
// Both helpers below are driven by the same `FerriteBrowser::progress_offset`
// `ServoFrame` already advances every ~16ms tick (see that handler) —
// consistent with `ease_out_cubic` above, no separate animation clock is
// introduced. Kept as pure `f32 -> f32`/`(usize, usize, f32) -> f32`
// functions rather than inlined into `view()`, the same reasoning
// `ease_out_cubic` documents: testable without spinning up Iced at all.

/// A smooth "something is happening" breathing alpha, `t` in `0.0..=1.0`
/// (`progress_offset`) with `cycles` full breaths per `t`'s wrap-around —
/// pulled out of what were three near-duplicate inline sine expressions
/// (the new-tab loading placeholder's "Fe" logo, the old flat-alpha
/// progress-bar pulse this charter replaces with
/// [`progress_segment_brightness`], and the tab bar's loading dot, which
/// used to just be a static, unanimated `"..."` string — see that call
/// site) into one shared curve.
fn pulse_alpha(t: f32, cycles: f32, base: f32, amplitude: f32) -> f32 {
    base + amplitude * (t * std::f32::consts::TAU * cycles).sin().abs()
}

/// Brightness (`0.0..=1.0`) of progress-bar segment `i` of `segments`, at
/// animation phase `t` (`progress_offset`, `0.0..=1.0`) — a single bright
/// band that sweeps left to right and wraps, rather than the whole bar
/// pulsing in lockstep, which is what makes this read as an actual
/// "in-progress" indicator instead of a flat glow. Replaces the pre-C3c
/// top-of-window progress bar (a single `Length::Fill` strip whose alpha
/// pulsed uniformly via `0.55 + 0.45 * (progress_offset * TAU *
/// 1.5).sin().abs()`) with `view()`'s new `PROGRESS_SEGMENTS`-wide row of
/// individually-lit segments driven by this function.
fn progress_segment_brightness(i: usize, segments: usize, t: f32) -> f32 {
    let n = segments as f32;
    let peak = t.clamp(0.0, 1.0) * n;
    let raw = (i as f32 - peak).abs();
    let wrapped = raw.min(n - raw);
    // How many neighboring segments share the light, in segment widths —
    // a fixed shape constant, not exposed as a parameter: nothing in this
    // crate needs a different sweep width today, and this function's own
    // tests below pin the resulting curve rather than treating it as a free
    // knob.
    const SPREAD: f32 = 1.6;
    (1.0 - wrapped / SPREAD).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Styling helpers
// ---------------------------------------------------------------------------

// Every function below is passed to `.style(...)` as a bare `fn` pointer
// (see the "Colour palette" section header above), so `theme` here is the
// real `iced::Theme` the app is currently rendering with, supplied by iced
// itself — not ignored the way the pre-C3c `_theme` parameter name implied.

fn separator_style(theme: &Theme) -> container::Style {
    let palette = palette_for_theme(theme);
    container::Style {
        background: Some(Background::Color(palette.divider)),
        ..container::Style::default()
    }
}

fn tab_bar_style(theme: &Theme) -> container::Style {
    let palette = palette_for_theme(theme);
    container::Style {
        background: Some(Background::Color(palette.surface)),
        ..container::Style::default()
    }
}

fn close_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = palette_for_theme(theme);
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => Color {
                r: 1.0,
                g: 0.35,
                b: 0.35,
                a: 0.15,
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => palette.danger,
            _ => palette.text_dim,
        },
        border: Border {
            radius: iced::border::Radius::new(4.0),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn nav_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = palette_for_theme(theme);
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => palette.raised,
            button::Status::Pressed => Color {
                r: palette.raised.r * 0.80,
                g: palette.raised.g * 0.80,
                b: palette.raised.b * 0.80,
                a: 1.0,
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: match status {
            button::Status::Disabled => Color {
                a: 0.20,
                ..palette.text_dim
            },
            _ => palette.text,
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn toolbar_style(theme: &Theme) -> container::Style {
    let palette = palette_for_theme(theme);
    container::Style {
        background: Some(Background::Color(palette.base)),
        ..container::Style::default()
    }
}

fn panel_btn_active(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = palette_for_theme(theme);
    button::Style {
        background: Some(Background::Color(palette.accent)),
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn panel_btn_inactive(theme: &Theme, status: button::Status) -> button::Style {
    let palette = palette_for_theme(theme);
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.raised,
            _ => palette.surface,
        })),
        text_color: match status {
            button::Status::Hovered => palette.text,
            _ => palette.text_dim,
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 1.0,
            color: palette.divider,
        },
        ..button::Style::default()
    }
}

fn accent_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = palette_for_theme(theme);
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.accent_bright,
            _ => palette.accent,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn bottom_panel_style(theme: &Theme) -> container::Style {
    let palette = palette_for_theme(theme);
    container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border {
            color: palette.divider,
            width: 1.0,
            radius: iced::border::Radius {
                top_left: BORDER_RADIUS,
                top_right: BORDER_RADIUS,
                bottom_left: 0.0,
                bottom_right: 0.0,
            },
        },
        shadow: iced::Shadow {
            color: Color {
                a: 0.3,
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            offset: iced::Vector::new(0.0, -4.0),
            blur_radius: 14.0,
        },
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// Audit helpers
// ---------------------------------------------------------------------------

/// `palette` supplies three of the five colours directly (`safe`/`warn`/
/// `danger`); "USED"/"EVAL" have no dedicated palette role (an informational
/// blue and a neutral gray, not one of the twelve semantic colours), so they
/// stay literal here — but still theme-aware via the separate `is_light`
/// flag (rather than inferring it by comparing `palette`'s address against
/// `&LIGHT_PALETTE`: both are `const`, not `static`, so the language gives
/// no guarantee two references to the same `const` item share an address —
/// an explicit `bool` the caller already has on hand is the safe way to ask
/// this), since the dark palette's light, saturated versions would fail the
/// light theme's contrast target the same way the pre-theme `C_SAFE`/
/// `C_WARN`/`C_DANGER` values did (see `LIGHT_PALETTE`'s doc comment).
fn kind_label(kind: &AuditEventKind, palette: &Palette, is_light: bool) -> (&'static str, Color) {
    match kind {
        AuditEventKind::CapabilityGranted => ("GRANTED", palette.safe),
        AuditEventKind::CapabilityDenied => ("DENIED", palette.danger),
        AuditEventKind::CapabilityExercised => (
            "USED",
            if is_light {
                Color::from_rgb(0.10, 0.40, 0.75)
            } else {
                Color::from_rgb(0.4, 0.7, 1.0)
            },
        ),
        AuditEventKind::ContentBlocked => ("BLOCKED", palette.warn),
        AuditEventKind::EvalExecutionRecorded => (
            "EVAL",
            if is_light {
                Color::from_rgb(0.42, 0.42, 0.46)
            } else {
                Color::from_rgb(0.6, 0.6, 0.6)
            },
        ),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            &s[..s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len())]
        )
    }
}

// ---------------------------------------------------------------------------
// URL resolution
// ---------------------------------------------------------------------------

/// Smart URL resolver — only adds a scheme when one is absent, and picks
/// https vs search based on whether the input looks like a hostname.
fn resolve_url(input: &str) -> String {
    let trimmed = input.trim();

    // Already has a scheme → pass through unchanged.
    if trimmed.contains("://") {
        return trimmed.to_string();
    }

    // Special pages.
    if trimmed == "about:blank" || trimmed.starts_with("about:") {
        return trimmed.to_string();
    }

    // Looks like a hostname (no spaces, has a dot, no special chars that
    // would be illegal in a hostname).  Prepend https://.
    let no_spaces = !trimmed.contains(' ');
    let has_dot = trimmed.contains('.');
    let path_like = trimmed.starts_with('/');
    if no_spaces && (has_dot || path_like) {
        return format!("https://{}", trimmed);
    }

    // Everything else → DuckDuckGo Lite search.
    let encoded = urlencoding::encode(trimmed);
    format!("https://lite.duckduckgo.com/lite/?q={}", encoded)
}

// ---------------------------------------------------------------------------
// Live agent loop — message-driven step loop shared by the initial
// (bypassed/clean-dry-run) run and the post-consent real run.
//
// See this file's module docs for why this is a step-by-step message loop
// rather than one call to `browser_loop::run_agent_loop`: the live engine
// (`BorrowedServoEngine`, wrapping a real, `!Send` `HeadlessServoSession`)
// cannot cross the `tokio::spawn` boundary a single long-running background
// task would need. Only the model round trip (`&dyn ModelProvider`, `Send +
// Sync`) is ever spawned; every actual browser action executes synchronously
// on the Iced update thread, via the exact same `execute_action` dispatch
// `run_agent_loop` itself uses.
// ---------------------------------------------------------------------------

/// Initializes `state.live_loop` from `prompt` (a fresh message history) —
/// used both for a bypassed/clean-dry-run task (empty `rejected`/
/// `rejected_origins`) and for the post-consent real run (the user's actual
/// decisions) — then spawns the first step's background model call.
fn start_live_loop(
    state: &mut FerriteBrowser,
    run_id: u64,
    prompt: String,
    rejected: std::collections::HashSet<ToolId>,
    rejected_origins: std::collections::HashSet<String>,
) {
    state.agent_is_running = true;
    let live = LiveAgentLoop {
        messages: vec![Message::user(prompt)],
        actions_taken: Vec::new(),
        started_at: std::time::Instant::now(),
        budget: LoopBudget::default(),
        rejected,
        rejected_origins,
        consecutive_malformed: 0,
    };
    spawn_next_step(state, run_id, live);
}

/// Checks the step/wall-clock budget (mirroring
/// `browser_loop::run_agent_loop`'s own pre-request checks exactly), then
/// spawns the background model call for the next step. `live` is stored
/// back onto `state.live_loop` for the resulting `AgentStepReady` to pick
/// up; a budget stop instead ends the run with a benign, visible message —
/// not an error, since a budget cutoff is a designed safety limit, not a
/// failure.
fn spawn_next_step(state: &mut FerriteBrowser, run_id: u64, live: LiveAgentLoop) {
    if live.actions_taken.len() >= live.budget.max_steps {
        state.agent_response = Some("[stopped: step budget exhausted]".to_string());
        state.agent_is_running = false;
        return;
    }
    if live.started_at.elapsed() >= live.budget.max_wall_clock {
        state.agent_response = Some("[stopped: wall-clock budget exhausted]".to_string());
        state.agent_is_running = false;
        return;
    }

    let provider = state.model_provider.clone();
    let model_tag_main = state.model_tag_main.clone();
    let messages = live.messages.clone();
    let event_tx = match state.agent_event_tx.clone() {
        Some(tx) => tx,
        None => return,
    };

    let handle = tokio::task::spawn(async move {
        let request = CompletionRequest::new(model_tag_main, ModelTier::Main, messages)
            .with_system_prompt(SYSTEM_PROMPT, SYSTEM_PROMPT_VERSION)
            .with_options(SamplingOptions::default().with_num_predict(AGENT_LOOP_NUM_PREDICT));
        let action =
            match provider.complete(request).await {
                Ok(response) => serde_json::from_str::<AgentAction>(response.content.trim())
                    .map_err(|e| StepFailure::Malformed {
                        raw: response.content,
                        message: e.to_string(),
                    }),
                Err(e) => Err(StepFailure::Model(e.to_string())),
            };
        let _ = event_tx.send(FerriteBrowserMessage::AgentStepReady { run_id, action });
    });

    state.agent_handle = Some(handle);
    state.live_loop = Some(live);
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    let palette = state.palette();
    let is_light_theme = state.theme_mode == AppTheme::Light;
    let active_tab_idx = state.active_tab;
    let is_loading_active = state.is_loading;

    // ── Tab bar ────────────────────────────────────────────────────────────
    const UNDERLINE_H: f32 = 2.5;
    const TAB_INNER_H: f32 = TAB_BAR_HEIGHT - UNDERLINE_H;
    let can_close = state.tabs.len() > 1;

    let mut tab_elements: Vec<Element<FerriteBrowserMessage>> = state
        .tab_titles
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active_tab_idx;
            let is_hovered = state.hovered_tab == Some(i);
            let spinning = is_loading_active && i == active_tab_idx;

            // Loading state shows a smoothly breathing dot (`pulse_alpha`,
            // C3c) rather than a static "..." string — the pre-C3c version
            // of this comment claimed it already reused the
            // `progress_offset`-driven pattern the agent sidebar's
            // "Working..." indicator uses, but the code above it never
            // actually did (`text(if spinning {"..."} else {"•"})` is a
            // fixed string, not animated at all); fixed here rather than
            // left standing, per this project's own R2 discipline against
            // an inaccurate comment. Once loaded, a real favicon (C3b —
            // `HeadlessServoSession::get_favicon()`) replaces the
            // placeholder dot for any tab that has one; a page that never
            // sets one (about:blank, some errors) keeps the dot, same as
            // before this existed — the accent underline below already
            // carries most of the active/inactive signal, so the dot stays
            // a quiet fallback, not a second competing indicator.
            let favicon_dot = |pulsing: bool| {
                let base_color = if is_active {
                    palette.accent
                } else {
                    palette.text_dim
                };
                let color = if pulsing {
                    Color {
                        a: pulse_alpha(state.progress_offset, 1.2, 0.35, 0.65),
                        ..base_color
                    }
                } else {
                    base_color
                };
                text("•").size(11).color(color).into()
            };
            let favicon: Element<FerriteBrowserMessage> = if spinning {
                favicon_dot(true)
            } else {
                match state.tab_favicons.get(i).and_then(|f| f.as_ref()) {
                    Some(handle) => ServoImage::new(handle.clone())
                        .width(Length::Fixed(14.0))
                        .height(Length::Fixed(14.0))
                        .into(),
                    None => favicon_dot(false),
                }
            };

            let label_elem = text(truncate(label, 22)).size(13).color(if is_active {
                palette.text
            } else {
                palette.text_dim
            });

            // Close-button-on-hover: visible for the active tab (always
            // reachable without a hover) and for whichever tab the pointer
            // is currently over; otherwise a same-size transparent spacer,
            // so the row's width never jumps when the button appears.
            let show_close = can_close && (is_active || is_hovered);
            let close_btn: Element<FerriteBrowserMessage> = if show_close {
                button(icon(Icon::Close, 10.0, palette.text_dim))
                    .padding(4)
                    .width(Length::Fixed(18.0))
                    .height(Length::Fixed(18.0))
                    .style(close_btn_style)
                    .on_press(FerriteBrowserMessage::CloseTab(i))
                    .into()
            } else {
                container(text(""))
                    .width(Length::Fixed(18.0))
                    .height(Length::Fixed(18.0))
                    .into()
            };

            let content_row = container(
                row![favicon, label_elem, close_btn]
                    .spacing(5)
                    .align_y(iced::Alignment::Center),
            )
            .width(Length::Shrink)
            .height(Length::Fixed(TAB_INNER_H))
            .padding([0, 10])
            .align_y(iced::Alignment::Center)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(if is_active {
                    palette.base
                } else if is_hovered {
                    Color {
                        a: 0.5,
                        ..palette.raised
                    }
                } else {
                    Color::TRANSPARENT
                })),
                border: Border {
                    radius: iced::border::Radius {
                        top_left: BORDER_RADIUS,
                        top_right: BORDER_RADIUS,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    },
                    ..Border::default()
                },
                ..container::Style::default()
            });

            let underline = container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(UNDERLINE_H))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(if is_active {
                        palette.accent
                    } else {
                        Color::TRANSPARENT
                    })),
                    ..container::Style::default()
                });

            mouse_area(
                column![content_row, underline]
                    .spacing(0)
                    .width(Length::Shrink)
                    .height(Length::Fixed(TAB_BAR_HEIGHT)),
            )
            .on_press(FerriteBrowserMessage::SelectTab(i))
            .on_enter(FerriteBrowserMessage::TabHoverEnter(i))
            .on_exit(FerriteBrowserMessage::TabHoverExit(i))
            .into()
        })
        .collect();

    // "+" new-tab button — a real `button` (not a bare `mouse_area`) so it
    // gets the same hover/press feedback every other toolbar control has,
    // via the same `nav_btn_style` used by Back/Forward/Reload.
    tab_elements.push(
        button(icon(Icon::Add, ICON_SIZE_SM, palette.text_dim))
            .width(Length::Fixed(TAB_BAR_HEIGHT))
            .height(Length::Fixed(TAB_BAR_HEIGHT))
            .padding(0)
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::AddTab)
            .into(),
    );

    let tab_bar = container(
        scrollable(
            row(tab_elements)
                .spacing(1)
                .align_y(iced::Alignment::Center)
                .padding([0, 8]),
        )
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().margin(0).scroller_width(2),
        )),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TAB_BAR_HEIGHT))
    .align_y(iced::Alignment::Center)
    .style(tab_bar_style);

    // ── Separator ──────────────────────────────────────────────────────────
    let sep_top = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Toolbar ────────────────────────────────────────────────────────────
    // [←] [→] [↺/✕]  [🔒 address bar ...]  [Audit] [JS]

    // Back/Forward/Reload read as icon-only controls (the common browser
    // convention) — disabled state dims the icon itself in addition to
    // `nav_btn_style`'s existing Disabled text-color handling, so the cue
    // survives the switch from text to a tinted glyph.
    let nav_icon_color = |enabled: bool| {
        if enabled {
            palette.text
        } else {
            Color {
                a: 0.25,
                ..palette.text_dim
            }
        }
    };

    let back_btn = button(icon(
        Icon::Back,
        ICON_SIZE,
        nav_icon_color(state.can_go_back),
    ))
    .padding([6, 11])
    .style(nav_btn_style)
    .on_press_maybe(state.can_go_back.then_some(FerriteBrowserMessage::GoBack));

    let fwd_btn = button(icon(
        Icon::Forward,
        ICON_SIZE,
        nav_icon_color(state.can_go_forward),
    ))
    .padding([6, 11])
    .style(nav_btn_style)
    .on_press_maybe(
        state
            .can_go_forward
            .then_some(FerriteBrowserMessage::GoForward),
    );

    let reload_btn: Element<FerriteBrowserMessage> = if state.is_loading {
        button(icon(Icon::Close, ICON_SIZE, palette.text))
            .padding([6, 11])
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::StopLoading)
            .into()
    } else {
        button(icon(Icon::Reload, ICON_SIZE, palette.text))
            .padding([6, 11])
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::Reload)
            .into()
    };

    let current_url = state
        .tab_urls
        .get(state.active_tab)
        .map(String::as_str)
        .unwrap_or("about:blank");
    let is_https = current_url.starts_with("https://");
    let is_http_insecure =
        current_url.starts_with("http://") && !current_url.starts_with("https://");
    let is_about = current_url == "about:blank";

    let security_icon: Element<FerriteBrowserMessage> = if is_about {
        text("").size(13).into()
    } else if is_https {
        text("HTTPS").size(10).color(palette.safe).into()
    } else if is_http_insecure {
        text("HTTP").size(10).color(palette.warn).into()
    } else {
        text("").size(13).into()
    };

    let addr_input = text_input(
        if is_about {
            "Search or type an address"
        } else {
            ""
        },
        &state.address_bar_input,
    )
    .id(text_input::Id::new(ADDRESS_BAR_ID))
    .width(Length::Fill)
    .padding([7, 10])
    .size(13)
    .style(|_: &Theme, status| {
        let focused = matches!(status, text_input::Status::Focused);
        text_input::Style {
            background: Background::Color(palette.input),
            border: Border {
                radius: iced::border::Radius::new(20.0),
                width: if focused { 1.5 } else { 1.0 },
                color: if focused {
                    palette.accent
                } else {
                    palette.divider
                },
            },
            icon: palette.text_dim,
            placeholder: palette.text_dim,
            value: palette.text,
            selection: Color {
                a: 0.30,
                ..palette.accent
            },
        }
    })
    .on_input(FerriteBrowserMessage::AddressBarChanged)
    .on_submit(FerriteBrowserMessage::NavigateRequested(
        state.address_bar_input.clone(),
    ));

    let addr_row = container(
        row![security_icon, addr_input]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([0, 4]);

    // DevTools toggles — an icon that names the panel plus its label; the
    // old leading "v"/"+" glyph is gone, since the button's own
    // active/inactive background (`panel_btn_active`/`panel_btn_inactive`,
    // unchanged) already carries that state, and duplicating it as a second
    // text glyph in front of the icon read as dev-tool clutter.
    let toggle_icon_color = |active: bool| {
        if active {
            Color::WHITE
        } else {
            palette.text_dim
        }
    };

    let audit_btn = button(
        row![
            icon(
                Icon::Audit,
                ICON_SIZE_SM,
                toggle_icon_color(state.show_audit_panel)
            ),
            text("Audit").size(12),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([5, 10])
    .style(if state.show_audit_panel {
        panel_btn_active
    } else {
        panel_btn_inactive
    })
    .on_press(FerriteBrowserMessage::ToggleAuditPanel);

    let js_btn = button(
        row![
            icon(
                Icon::Console,
                ICON_SIZE_SM,
                toggle_icon_color(state.show_js_console)
            ),
            text("JS").size(12),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([5, 10])
    .style(if state.show_js_console {
        panel_btn_active
    } else {
        panel_btn_inactive
    })
    .on_press(FerriteBrowserMessage::ToggleJsConsole);

    let agent_btn = button(
        row![
            icon(
                Icon::Agent,
                ICON_SIZE_SM,
                toggle_icon_color(state.show_agent_sidebar)
            ),
            text("Agent").size(12),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([5, 10])
    .style(if state.show_agent_sidebar {
        panel_btn_active
    } else {
        panel_btn_inactive
    })
    .on_press(FerriteBrowserMessage::ToggleAgentSidebar);

    // C3c: theme toggle — shows the icon of the mode a click switches *to*
    // (sun while dark is active, moon while light is active), the same
    // convention most browser/OS theme switchers use, rather than an icon
    // for the mode currently on screen.
    let theme_btn = button(icon(
        if is_light_theme {
            Icon::Moon
        } else {
            Icon::Sun
        },
        ICON_SIZE,
        palette.text_dim,
    ))
    .padding([6, 11])
    .style(nav_btn_style)
    .on_press(FerriteBrowserMessage::ToggleTheme);

    let toolbar = container(
        row![back_btn, fwd_btn, reload_btn, addr_row, audit_btn, js_btn, agent_btn, theme_btn]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([0, PANEL_PADDING]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .style(toolbar_style);

    // ── Progress bar ───────────────────────────────────────────────────────
    // C3c: a single bright band sweeps left-to-right across
    // `PROGRESS_SEGMENTS` segments (`progress_segment_brightness`) rather
    // than the whole bar pulsing in lockstep — a real animated "in
    // progress" indicator, not a flat breathing strip.
    let maybe_progress: Option<Element<FerriteBrowserMessage>> = if state.is_loading {
        let t = state.progress_offset;
        let segments: Vec<Element<FerriteBrowserMessage>> = (0..PROGRESS_SEGMENTS)
            .map(|i| {
                let brightness = progress_segment_brightness(i, PROGRESS_SEGMENTS, t);
                container(text(""))
                    .width(Length::FillPortion(1))
                    .height(Length::Fixed(2.0))
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.12 + 0.88 * brightness,
                            ..palette.accent_bright
                        })),
                        ..container::Style::default()
                    })
                    .into()
            })
            .collect();
        Some(
            row(segments)
                .spacing(1)
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .into(),
        )
    } else {
        None
    };

    let sep_bottom = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Audit panel ────────────────────────────────────────────────────────
    let audit_panel: Option<Element<FerriteBrowserMessage>> = if state.show_audit_panel {
        let hdr = container(
            row![
                text("  Audit Log")
                    .size(12)
                    .color(palette.text)
                    .width(Length::Fill),
                button(text("Refresh").size(11))
                    .padding([2, 8])
                    .style(panel_btn_inactive)
                    .on_press(FerriteBrowserMessage::RefreshAuditLog),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([5, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(palette.raised)),
            ..container::Style::default()
        });

        let col_hdr = container(
            row![
                text("SEQ").size(11).color(palette.text_dim).width(36),
                text("TIME").size(11).color(palette.text_dim).width(76),
                text("KIND").size(11).color(palette.text_dim).width(72),
                text("PRINCIPAL").size(11).color(palette.text_dim).width(95),
                text("CAPABILITY")
                    .size(11)
                    .color(palette.text_dim)
                    .width(95),
                text("URL")
                    .size(11)
                    .color(palette.text_dim)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .padding([3, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.5,
                ..palette.raised
            })),
            ..container::Style::default()
        });

        let rows: Vec<Element<FerriteBrowserMessage>> = if state.audit_entries.is_empty() {
            vec![container(
                text("No audit entries yet - run the sandbox demo and click Refresh")
                    .size(12)
                    .color(palette.text_dim),
            )
            .width(Length::Fill)
            .padding([12, PANEL_PADDING])
            .into()]
        } else {
            state
                .audit_entries
                .iter()
                .map(|e| {
                    let (ks, kc) = kind_label(&e.kind, palette, is_light_theme);
                    let ts = e.timestamp.format("%H:%M:%S%.3f").to_string();
                    container(
                        row![
                            text(e.sequence.to_string())
                                .size(12)
                                .color(palette.text_dim)
                                .width(36),
                            text(ts).size(12).color(palette.text_dim).width(76),
                            text(ks).size(12).color(kc).width(72),
                            text(truncate(&e.principal_id.to_string(), 8))
                                .size(12)
                                .color(palette.text)
                                .width(95),
                            text(e.capability.as_deref().unwrap_or("-"))
                                .size(12)
                                .color(palette.text)
                                .width(95),
                            text(truncate(e.url.as_deref().unwrap_or("-"), 60))
                                .size(12)
                                .color(palette.text_dim)
                                .width(Length::Fill),
                        ]
                        .spacing(8)
                        .padding([3, PANEL_PADDING]),
                    )
                    .width(Length::Fill)
                    .into()
                })
                .collect()
        };

        Some(
            container(column![
                hdr,
                col_hdr,
                scrollable(column(rows)).height(Length::Fill)
            ])
            .width(Length::Fill)
            .height(220)
            .style(bottom_panel_style)
            .into(),
        )
    } else {
        None
    };

    // ── JS console panel ───────────────────────────────────────────────────
    let js_panel: Option<Element<FerriteBrowserMessage>> = if state.show_js_console {
        let hdr = container(
            row![
                text("  JS Console")
                    .size(12)
                    .color(palette.text)
                    .width(Length::Fill),
                text(format!("({} shortcut)", MOD_LABEL))
                    .size(11)
                    .color(palette.text_dim),
                button(text("Clear").size(11))
                    .padding([2, 8])
                    .style(panel_btn_inactive)
                    .on_press(FerriteBrowserMessage::JsConsoleClear),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([5, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(palette.raised)),
            ..container::Style::default()
        });

        let out_rows: Vec<Element<FerriteBrowserMessage>> = if state.js_output.is_empty() {
            vec![container(
                text("Type an expression and press Enter")
                    .size(12)
                    .color(palette.text_dim),
            )
            .padding([10, PANEL_PADDING])
            .into()]
        } else {
            state
                .js_output
                .iter()
                .flat_map(|(snip, res)| {
                    let err = res.starts_with("Error")
                        || res.starts_with("BLOCKED")
                        || res.starts_with("ERROR");
                    [
                        container(text(format!("> {}", snip)).size(12).color(palette.accent))
                            .padding([2, PANEL_PADDING])
                            .width(Length::Fill)
                            .into(),
                        container(text(format!("  {}", res)).size(12).color(if err {
                            palette.danger
                        } else {
                            palette.safe
                        }))
                        .padding([1, PANEL_PADDING])
                        .width(Length::Fill)
                        .into(),
                    ]
                })
                .collect()
        };

        let run_btn = button(text("Run").size(12))
            .padding([6, 12])
            .style(accent_btn_style)
            .on_press(FerriteBrowserMessage::JsExecuteRequested);

        let js_field = text_input("JavaScript expression...", &state.js_input)
            .id(text_input::Id::new(JS_INPUT_ID))
            .width(Length::Fill)
            .padding([6, 8])
            .size(13)
            .style(|_: &Theme, status| {
                let focused = matches!(status, text_input::Status::Focused);
                text_input::Style {
                    background: Background::Color(palette.input),
                    border: Border {
                        radius: iced::border::Radius::new(6.0),
                        width: if focused { 1.5 } else { 1.0 },
                        color: if focused {
                            palette.accent
                        } else {
                            palette.divider
                        },
                    },
                    icon: palette.text_dim,
                    placeholder: palette.text_dim,
                    value: palette.text,
                    selection: Color {
                        a: 0.30,
                        ..palette.accent
                    },
                }
            })
            .on_input(FerriteBrowserMessage::JsInputChanged)
            .on_submit(FerriteBrowserMessage::JsExecuteRequested);

        let input_row = container(
            row![text(">").size(13).color(palette.accent), js_field, run_btn]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .padding([5, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(palette.base)),
            ..container::Style::default()
        });

        Some(
            container(column![
                hdr,
                scrollable(column(out_rows).spacing(0).width(Length::Fill)).height(Length::Fill),
                input_row,
            ])
            .width(Length::Fill)
            .height(260)
            .style(bottom_panel_style)
            .into(),
        )
    } else {
        None
    };

    // ── Content area ───────────────────────────────────────────────────────
    let active = state.active_tab;

    // Wrapped in `responsive` so `state.content_area_size` always reflects
    // this container's true logical size — whatever else in the layout
    // (window size, the agent sidebar, the audit/JS panel) is currently
    // taking space — rather than a value this function would otherwise
    // have to compute by hand-replicating that layout's arithmetic. See
    // `content_area_size`'s doc comment and `ServoFrame`'s tick handler,
    // which reads this back to keep the Servo render buffer's physical
    // pixel size matched to it.
    let content: Element<FerriteBrowserMessage> = responsive(move |size: Size| {
        state.content_area_size.set(size);
        if let Some(err_msg) = state.tab_error.get(active).and_then(|e| e.as_ref()) {
            // Error page
            let failed_url = state.tab_urls.get(active).map(String::as_str).unwrap_or("");
            container(
                column![
                    text("ERR").size(42).color(palette.danger),
                    container(text("")).height(10),
                    text("Page could not be loaded")
                        .size(22)
                        .color(palette.text),
                    container(text("")).height(6),
                    text(failed_url).size(13).color(palette.text_dim),
                    container(text("")).height(4),
                    text(err_msg.as_str()).size(12).color(palette.text_dim),
                    container(text("")).height(28),
                    row![
                        button(text("Try Again").size(13))
                            .padding([9, 24])
                            .style(accent_btn_style)
                            .on_press(FerriteBrowserMessage::Reload),
                        button(text("New Tab").size(13))
                            .padding([9, 24])
                            .style(nav_btn_style)
                            .on_press(FerriteBrowserMessage::NavigateRequested(
                                "about:blank".to_string(),
                            )),
                    ]
                    .spacing(12),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(palette.base)),
                ..container::Style::default()
            })
            .into()
        } else if state
            .tab_urls
            .get(active)
            .map(|u| u == "about:blank")
            .unwrap_or(true)
        {
            // Home / new-tab page — always shown for about:blank, even if Servo
            // has produced a blank white frame for that URL.
            new_tab_page(state)
        } else if let Some((w, h, bytes)) = state
            .servo_sessions
            .get(&active)
            .and_then(|s| s.get_frame())
        {
            // Live Servo frame — interactive via mouse_area
            let handle = ImageHandle::from_rgba(w, h, bytes);
            let img = ServoImage::new(handle)
                .width(Length::Fill)
                .height(Length::Fill);

            mouse_area(container(img).width(Length::Fill).height(Length::Fill))
                .on_move(|pos| FerriteBrowserMessage::ServoMouseMove { x: pos.x, y: pos.y })
                .on_press(FerriteBrowserMessage::ServoMousePress)
                .on_release(FerriteBrowserMessage::ServoMouseRelease)
                .on_scroll(|delta| {
                    use iced::mouse::ScrollDelta;
                    let (dx, dy) = match delta {
                        ScrollDelta::Lines { x, y } => (x * 60.0, y * 60.0),
                        ScrollDelta::Pixels { x, y } => (x, y),
                    };
                    FerriteBrowserMessage::ServoScroll {
                        delta_x: dx,
                        delta_y: dy,
                    }
                })
                .into()
        } else {
            // Loading placeholder (no frame yet for a non-blank URL)
            let pulse = pulse_alpha(state.progress_offset, 1.0, 0.25, 0.20);
            container(
                column![
                    text("Fe").size(40).color(Color {
                        a: pulse,
                        ..palette.accent
                    }),
                    container(text("")).height(10),
                    text("Loading...").size(14).color(palette.text_dim),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(palette.base)),
                ..container::Style::default()
            })
            .into()
        }
    })
    .into();

    // ── Compose layout ──────────────────────────────────────────────────────
    let mut layout: Vec<Element<FerriteBrowserMessage>> =
        vec![tab_bar.into(), sep_top.into(), toolbar.into()];

    if let Some(bar) = maybe_progress {
        layout.push(bar);
    } else {
        layout.push(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .into(),
        );
    }
    layout.push(sep_bottom.into());

    if let Some(p) = audit_panel {
        layout.push(p);
    } else if let Some(p) = js_panel {
        layout.push(p);
    }

    // Wrap browser viewport + optional agent sidebar in a horizontal row.
    let main_content: Element<FerriteBrowserMessage> = if state.show_agent_sidebar {
        iced::widget::row![content, view_agent_sidebar(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        content
    };
    layout.push(main_content);

    container(column(layout))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(palette.base)),
            ..container::Style::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// New-tab / home page
// ---------------------------------------------------------------------------

fn new_tab_page(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    let palette = state.palette();
    let search_bar = text_input("Search or type an address", &state.new_tab_search_input)
        .width(560)
        .padding([14, 22])
        .size(15)
        .style(|_: &Theme, status| {
            let focused = matches!(status, text_input::Status::Focused);
            text_input::Style {
                background: Background::Color(palette.input),
                border: Border {
                    radius: iced::border::Radius::new(30.0),
                    width: if focused { 1.5 } else { 1.0 },
                    color: if focused {
                        palette.accent
                    } else {
                        palette.divider
                    },
                },
                icon: palette.text_dim,
                placeholder: palette.text_dim,
                value: palette.text,
                selection: Color {
                    a: 0.30,
                    ..palette.accent
                },
            }
        })
        .on_input(FerriteBrowserMessage::NewTabSearchChanged)
        .on_submit(FerriteBrowserMessage::NavigateRequested(resolve_url(
            &state.new_tab_search_input,
        )));

    // Quick-access tiles
    let tiles: Vec<(&str, &str, &str)> = vec![
        ("[D]", "DuckDuckGo", "https://lite.duckduckgo.com"),
        ("[R]", "Rust Docs", "https://doc.rust-lang.org"),
        ("[G]", "GitHub", "https://github.com"),
        ("[S]", "Servo", "https://servo.org"),
        ("[N]", "Hacker News", "https://news.ycombinator.com"),
        ("[W]", "Wikipedia", "https://en.m.wikipedia.org"),
    ];

    let tile_row: Vec<Element<FerriteBrowserMessage>> = tiles
        .iter()
        .map(|(icon, label, url)| {
            let url = url.to_string();
            button(
                column![
                    text(*icon).size(26).color(palette.accent),
                    text(*label).size(12).color(palette.text_dim),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .padding([16, 18])
            .style(|_: &Theme, s| {
                let hov = matches!(s, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(if hov {
                        palette.raised
                    } else {
                        palette.surface
                    })),
                    text_color: palette.text,
                    border: Border {
                        radius: iced::border::Radius::new(12.0),
                        width: 1.0,
                        color: if hov {
                            palette.divider
                        } else {
                            Color {
                                a: 0.35,
                                ..palette.divider
                            }
                        },
                    },
                    shadow: if hov {
                        iced::Shadow {
                            color: Color {
                                a: 0.15,
                                r: 0.44,
                                g: 0.38,
                                b: 1.0,
                            },
                            offset: iced::Vector::new(0.0, 2.0),
                            blur_radius: 8.0,
                        }
                    } else {
                        iced::Shadow::default()
                    },
                }
            })
            .on_press(FerriteBrowserMessage::NavigateRequested(url))
            .into()
        })
        .collect();

    // Keyboard shortcut reference (platform-aware)
    let shortcuts_text = format!(
        "{M}+T  New tab   {M}+W  Close   {M}+L  Address   {M}+R  Reload   {M}+J  JS Console   F12  Audit",
        M = MOD_LABEL,
    );

    container(
        column![
            // Logo
            column![
                text("Fe").size(64).color(palette.accent),
                text("ferrite").size(40).color(palette.text),
                text("capability-governed browser")
                    .size(13)
                    .color(palette.text_dim),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center),
            container(text("")).height(44),
            // Search bar
            search_bar,
            container(text("")).height(36),
            // Quick-access tiles
            row(tile_row).spacing(12).wrap(),
            container(text("")).height(40),
            // Keyboard shortcut hints
            container(text(shortcuts_text).size(11).color(palette.text_dim),)
                .padding([8, 16])
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(palette.surface)),
                    border: Border {
                        radius: iced::border::Radius::new(8.0),
                        width: 1.0,
                        color: palette.divider,
                    },
                    ..container::Style::default()
                }),
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(palette.base)),
        ..container::Style::default()
    })
    .into()
}

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

fn handle_key_press(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<FerriteBrowserMessage> {
    use keyboard::key::Named;

    // On macOS the modifier is the Super (Cmd) key; everywhere else it's Ctrl.
    #[cfg(target_os = "macos")]
    let mod_active = modifiers.command();
    #[cfg(not(target_os = "macos"))]
    let mod_active = modifiers.control();

    match key {
        keyboard::Key::Character(c) if mod_active => match c.as_str() {
            "t" => Some(FerriteBrowserMessage::AddTab),
            "w" => Some(FerriteBrowserMessage::CloseActiveTab),
            "r" => Some(FerriteBrowserMessage::Reload),
            "l" => Some(FerriteBrowserMessage::FocusAddressBar),
            "j" => Some(FerriteBrowserMessage::ToggleJsConsole),
            _ => None,
        },
        keyboard::Key::Named(Named::F5) => Some(FerriteBrowserMessage::Reload),
        keyboard::Key::Named(Named::F12) => Some(FerriteBrowserMessage::ToggleAuditPanel),
        keyboard::Key::Named(Named::ArrowLeft) if modifiers.alt() => {
            Some(FerriteBrowserMessage::GoBack)
        }
        keyboard::Key::Named(Named::ArrowRight) if modifiers.alt() => {
            Some(FerriteBrowserMessage::GoForward)
        }
        keyboard::Key::Named(Named::Escape) => Some(FerriteBrowserMessage::EscapePressed),
        _ => None,
    }
}

pub fn subscription(state: &FerriteBrowser) -> Subscription<FerriteBrowserMessage> {
    let keyboard_sub = keyboard::on_key_press(handle_key_press);

    let servo_tick = if !state.servo_sessions.is_empty() {
        time::every(std::time::Duration::from_millis(16)).map(|_| FerriteBrowserMessage::ServoFrame)
    } else {
        Subscription::none()
    };

    // Consent-panel entrance animation (C1) — same 16ms tick shape as
    // `servo_tick` above, gated so it only ever runs while the panel is
    // actually animating in, not for the rest of the app's lifetime.
    let consent_anim_tick = if state.pending_diff.is_some() && state.consent_panel_anim < 1.0 {
        time::every(std::time::Duration::from_millis(16))
            .map(|_| FerriteBrowserMessage::ConsentPanelTick)
    } else {
        Subscription::none()
    };

    // Drain agent progress messages (AgentToolLogged, AgentCompleted, AgentFailed).
    struct AgentEventChannel;
    let agent_event_sub: Subscription<FerriteBrowserMessage> =
        if let Some(rx_arc) = &state.agent_event_rx {
            let rx_arc = rx_arc.clone();
            Subscription::run_with_id(
                std::any::TypeId::of::<AgentEventChannel>(),
                iced::stream::channel(64, move |mut sender| async move {
                    use iced::futures::SinkExt;
                    loop {
                        let msg = rx_arc.lock().await.recv().await;
                        match msg {
                            Some(msg) => {
                                let _ = sender.send(msg).await;
                            }
                            None => {
                                std::future::pending::<()>().await;
                            }
                        }
                    }
                }),
            )
        } else {
            Subscription::none()
        };

    Subscription::batch([keyboard_sub, servo_tick, agent_event_sub, consent_anim_tick])
}

// ---------------------------------------------------------------------------
// Agent sidebar view
// ---------------------------------------------------------------------------

fn view_agent_sidebar(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    let palette = state.palette();
    // Header: "Agent" label + optional step-progress indicator + Stop
    // button. `live_loop` is only `Some` for the message-driven live run
    // (not the dry-run phase, which has no per-step budget to show yet).
    let mut header_items: Vec<Element<FerriteBrowserMessage>> = vec![text("Agent")
        .size(16)
        .color(palette.text)
        .width(Length::Fill)
        .into()];
    if let Some(live) = &state.live_loop {
        header_items.push(
            text(format!(
                "Step {}/{}",
                live.actions_taken.len() + 1,
                live.budget.max_steps
            ))
            .size(11)
            .color(palette.text_dim)
            .into(),
        );
    }
    if state.agent_is_running {
        header_items.push(
            button(
                row![
                    icon(Icon::Stop, ICON_SIZE_SM, Color::WHITE),
                    text("Stop").size(12)
                ]
                .spacing(5)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .style(|_: &Theme, _| button::Style {
                background: Some(Background::Color(palette.danger)),
                text_color: Color::WHITE,
                border: Border {
                    radius: iced::border::Radius::new(BORDER_RADIUS),
                    ..Border::default()
                },
                ..button::Style::default()
            })
            .on_press(FerriteBrowserMessage::StopAgent)
            .into(),
        );
    }
    let header = container(
        row(header_items)
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([8, 12]),
    )
    .width(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(palette.raised)),
        ..container::Style::default()
    });

    let sep = || {
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(separator_style)
    };

    // Task input — disabled (greyed container) when running.
    let task_input: Element<FerriteBrowserMessage> = if state.agent_is_running {
        container(
            text(state.agent_task_input.as_str())
                .size(13)
                .color(palette.text_dim),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.6,
                ..palette.input
            })),
            border: Border {
                radius: iced::border::Radius::new(6.0),
                width: 1.0,
                color: palette.divider,
            },
            ..container::Style::default()
        })
        .into()
    } else {
        text_input("Enter a task...", &state.agent_task_input)
            .width(Length::Fill)
            .padding([7, 10])
            .size(13)
            .style(|_: &Theme, status| {
                let focused = matches!(status, text_input::Status::Focused);
                text_input::Style {
                    background: Background::Color(palette.input),
                    border: Border {
                        radius: iced::border::Radius::new(6.0),
                        width: if focused { 1.5 } else { 1.0 },
                        color: if focused {
                            palette.accent
                        } else {
                            palette.divider
                        },
                    },
                    icon: palette.text_dim,
                    placeholder: palette.text_dim,
                    value: palette.text,
                    selection: Color {
                        a: 0.30,
                        ..palette.accent
                    },
                }
            })
            .on_input(FerriteBrowserMessage::AgentTaskInputChanged)
            .on_submit(FerriteBrowserMessage::AgentTaskSubmitted)
            .into()
    };

    let run_disabled = state.agent_is_running || state.agent_task_input.trim().is_empty();
    let run_btn = button(
        row![
            icon(
                Icon::Play,
                ICON_SIZE_SM,
                if run_disabled {
                    palette.text_dim
                } else {
                    Color::WHITE
                }
            ),
            text("Run Task").size(13),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([7, 0])
    .width(Length::Fill)
    .style(if run_disabled {
        panel_btn_inactive
    } else {
        accent_btn_style
    })
    .on_press_maybe((!run_disabled).then_some(FerriteBrowserMessage::AgentTaskSubmitted));

    // ── Consent panel (shown instead of log+response when diff is pending) ──
    //
    // This is the security surface (`docs/REBUILD_DIRECTIVE.md` §6/A10): it
    // is built entirely from `iced_widget` native widgets, laid out by this
    // function from Rust values (`diff`/`expected`/`evidence` — plain Rust
    // structs, never page-supplied markup). Servo's page content only ever
    // reaches this process as a decoded pixel buffer (`session.get_frame()`,
    // rendered elsewhere as an `iced_widget::image`) — there is no code path
    // by which a page's HTML/CSS/text is parsed into a style, position, or
    // z-order for *this* panel. See `page_content_cannot_reach_the_consent_panels_inputs`
    // for the structural argument this session verified, not merely assumed.
    let body: Element<FerriteBrowserMessage> = if let Some(diff) = &state.pending_diff {
        let items = consent_items(diff, state.pending_expected.as_ref());

        let mut item_rows: Vec<Element<FerriteBrowserMessage>> = items
            .iter()
            .map(|item| {
                let approved = state.pending_decision.approved.contains(&item.id);
                let rejected = state.pending_decision.rejected.contains(&item.id);

                let approve_style = move |_: &Theme, _| button::Style {
                    background: Some(Background::Color(if approved {
                        palette.safe
                    } else {
                        Color {
                            a: 0.25,
                            ..palette.safe
                        }
                    })),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: iced::border::Radius::new(4.0),
                        ..Border::default()
                    },
                    ..button::Style::default()
                };
                let reject_style = move |_: &Theme, _| button::Style {
                    background: Some(Background::Color(if rejected {
                        palette.danger
                    } else {
                        Color {
                            a: 0.25,
                            ..palette.danger
                        }
                    })),
                    text_color: Color::WHITE,
                    border: Border {
                        radius: iced::border::Radius::new(4.0),
                        ..Border::default()
                    },
                    ..button::Style::default()
                };

                let approve_id = item.id.to_string();
                let reject_id = item.id.to_string();
                // Reject is listed and styled first: iced 0.13's `button`
                // widget does not implement the `Focusable` operation (only
                // `text_input`/`text_editor` do — verified against
                // `iced_core::widget::operation::focusable`), so a literal
                // keyboard-focus-ring default onto Reject is not achievable
                // against this pinned version's public API. This is the
                // available equivalent: reject reads first, and — the
                // property that actually matters — `consent_is_complete`
                // never lets Proceed fire while any item, including this
                // one, is undecided, so there is no path to a silent
                // approve-by-default.
                // An out-of-scope-origin item gets a small external-link
                // glyph ahead of its summary — the one place this crate
                // renders `Icon::Origin`, distinguishing "contacted an
                // unauthorized origin" rows from "used an unexpected tool"
                // rows at a glance, on top of the text difference already
                // in `item.summary` itself.
                let summary_row: Element<FerriteBrowserMessage> =
                    if origin_item_origin(&item.id).is_some() {
                        row![
                            icon(Icon::Origin, ICON_SIZE_SM, palette.text_dim),
                            text(item.summary.clone())
                                .size(12)
                                .color(palette.text)
                                .width(Length::Fill),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Start)
                        .into()
                    } else {
                        text(item.summary.clone())
                            .size(12)
                            .color(palette.text)
                            .width(Length::Fill)
                            .into()
                    };

                column![
                    summary_row,
                    row![
                        button(
                            row![
                                icon(Icon::Reject, 11.0, Color::WHITE),
                                text("Reject").size(11)
                            ]
                            .spacing(4)
                            .align_y(iced::Alignment::Center)
                        )
                        .padding([3, 7])
                        .style(reject_style)
                        .on_press(FerriteBrowserMessage::RejectTool(reject_id)),
                        button(
                            row![
                                icon(Icon::Approve, 11.0, Color::WHITE),
                                text("Approve").size(11)
                            ]
                            .spacing(4)
                            .align_y(iced::Alignment::Center)
                        )
                        .padding([3, 7])
                        .style(approve_style)
                        .on_press(FerriteBrowserMessage::ApproveTool(approve_id)),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                ]
                .spacing(4)
                .width(Length::Fill)
                .into()
            })
            .collect();

        let complete = consent_is_complete(diff, &state.pending_decision);
        let proceed_btn = button(text("Proceed with approved").size(12))
            .padding([7, 10])
            .width(Length::Fill)
            .style(if complete {
                accent_btn_style
            } else {
                panel_btn_inactive
            })
            .on_press_maybe(complete.then_some(FerriteBrowserMessage::ConsentSubmitted));

        let cancel_btn = button(text("Cancel").size(12))
            .padding([7, 10])
            .width(Length::Fill)
            .style(|_: &Theme, _| button::Style {
                background: Some(Background::Color(palette.raised)),
                text_color: palette.text_dim,
                border: Border {
                    radius: iced::border::Radius::new(BORDER_RADIUS),
                    width: 1.0,
                    color: palette.divider,
                },
                ..button::Style::default()
            })
            .on_press(FerriteBrowserMessage::ConsentCancelled);

        let mut panel_items: Vec<Element<FerriteBrowserMessage>> = vec![
            row![
                icon(Icon::Warning, ICON_SIZE, palette.danger),
                text("Unexpected Activity Detected")
                    .size(14)
                    .color(palette.danger)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
            text(diff.summary()).size(12).color(palette.text_dim).into(),
            sep().into(),
            text("Review each item:")
                .size(12)
                .color(palette.text)
                .into(),
        ];
        panel_items.append(&mut item_rows);
        panel_items.push(sep().into());

        // ── Dry-run evidence, collapsed by default ──────────────────────
        let evidence_toggle = button(
            text(if state.show_evidence {
                "v Hide dry-run evidence"
            } else {
                "> Show dry-run evidence"
            })
            .size(11),
        )
        .padding([3, 7])
        .style(panel_btn_inactive)
        .on_press(FerriteBrowserMessage::ToggleEvidence);
        panel_items.push(evidence_toggle.into());
        if state.show_evidence {
            if let Some(evidence) = &state.pending_evidence {
                let lines: Vec<Element<FerriteBrowserMessage>> = dry_run_evidence_lines(evidence)
                    .into_iter()
                    .map(|line| text(line).size(11).color(palette.text_dim).into())
                    .collect();
                panel_items.push(
                    container(column(lines).spacing(2))
                        .padding([6, 8])
                        .width(Length::Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(Background::Color(palette.input)),
                            border: Border {
                                radius: iced::border::Radius::new(6.0),
                                width: 1.0,
                                color: palette.divider,
                            },
                            ..container::Style::default()
                        })
                        .into(),
                );
            }
        }

        panel_items.push(sep().into());
        panel_items.push(proceed_btn.into());
        panel_items.push(cancel_btn.into());

        // ── Entrance transition (C1) ─────────────────────────────────────
        // A brief slide-in-and-settle plus a background-tint fade, driven
        // by `consent_panel_anim` (advanced 16ms at a time by
        // `ConsentPanelTick`, see `subscription()`) through `ease_out_cubic`.
        // Purely decorative: every item's own text above is already at full
        // opacity/its final position from the very first frame — only this
        // outer wrapper's background tint and top inset animate, so nothing
        // about what the user is being asked to approve is ever delayed,
        // dimmed, or obscured while this plays out (~200ms total).
        let anim_t = ease_out_cubic(state.consent_panel_anim);
        let slide_offset = (1.0 - anim_t) * 16.0;

        scrollable(
            container(column(panel_items).spacing(8).padding(Padding {
                top: 8.0 + slide_offset,
                right: 12.0,
                bottom: 8.0,
                left: 12.0,
            }))
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: palette.warn.r,
                    g: palette.warn.g,
                    b: palette.warn.b,
                    a: 0.08 * anim_t,
                })),
                ..container::Style::default()
            }),
        )
        .height(Length::Fill)
        .into()
    } else {
        // ── Live activity feed (C2) ─────────────────────────────────────────────
        // One card per `AgentLogEntry::Step` — icon (see `icon_for_action`),
        // title, the action's own parameter, and the real result
        // `execute_action` returned — not just the fact that some action
        // ran, which is all the pre-C2 flat-string log showed. A blocked
        // step (the user rejected it in consent) is styled in the danger
        // color throughout, so it reads as "this did NOT happen" rather
        // than blending in with a normal completed step.
        let dots = match ((state.progress_offset * 3.0) as usize) % 4 {
            0 => "",
            1 => ".",
            2 => "..",
            _ => "...",
        };
        let log_items: Vec<Element<FerriteBrowserMessage>> =
            if state.agent_log.is_empty() && !state.agent_is_running {
                vec![text("No active session.")
                    .size(12)
                    .color(palette.text_dim)
                    .into()]
            } else {
                let mut items: Vec<Element<_>> = state
                    .agent_log
                    .iter()
                    .map(|entry| match entry {
                        AgentLogEntry::Note(s) => container(
                            row![
                                text("->").size(12).color(palette.accent),
                                text(s.as_str()).size(12).color(palette.text_dim),
                            ]
                            .spacing(4),
                        )
                        .padding([2, 0])
                        .width(Length::Fill)
                        .into(),
                        AgentLogEntry::Step {
                            icon: step_icon,
                            label,
                            detail,
                            result,
                            blocked,
                        } => {
                            let accent = if *blocked {
                                palette.danger
                            } else {
                                palette.accent
                            };
                            let mut lines: Vec<Element<FerriteBrowserMessage>> = vec![row![
                                icon(*step_icon, ICON_SIZE_SM, accent),
                                text(*label).size(12).color(palette.text),
                            ]
                            .spacing(6)
                            .align_y(iced::Alignment::Center)
                            .into()];
                            if !detail.is_empty() {
                                lines.push(
                                    text(truncate(detail, 70))
                                        .size(11)
                                        .color(palette.text_dim)
                                        .into(),
                                );
                            }
                            lines.push(
                                text(truncate(result, 90))
                                    .size(11)
                                    .color(if *blocked {
                                        palette.danger
                                    } else {
                                        palette.text_dim
                                    })
                                    .into(),
                            );
                            container(column(lines).spacing(2))
                                .padding([5, 0])
                                .width(Length::Fill)
                                .into()
                        }
                    })
                    .collect();
                if state.agent_is_running {
                    items.push(
                        text(format!("Working{}", dots))
                            .size(12)
                            .color(palette.text_dim)
                            .into(),
                    );
                }
                items
            };

        let mut log_col_items = log_items;
        if let Some(response) = &state.agent_response {
            log_col_items.push(sep().into());
            log_col_items.push(
                container(
                    column![
                        text("Answer").size(13).color(palette.text_dim),
                        text(response.as_str()).size(13).color(palette.text),
                    ]
                    .spacing(4)
                    .padding([8, 12]),
                )
                .width(Length::Fill)
                .into(),
            );
        }

        scrollable(
            column(log_col_items)
                .spacing(2)
                .width(Length::Fill)
                .padding([4, 8]),
        )
        .height(Length::Fill)
        .into()
    };

    // Assemble column.
    let col_items: Vec<Element<FerriteBrowserMessage>> = vec![
        header.into(),
        sep().into(),
        container(column![task_input, run_btn].spacing(6).padding([8, 12]))
            .width(Length::Fill)
            .into(),
        sep().into(),
        body,
    ];

    container(column(col_items).width(Length::Fill))
        .width(Length::Fixed(320.0))
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(palette.surface)),
            border: Border {
                color: palette.divider,
                width: 1.0,
                radius: iced::border::Radius::new(0.0),
            },
            ..container::Style::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

pub fn launch() -> iced::Result {
    iced::application("Ferrite", update, view)
        .window_size(Size::new(1280.0, 800.0))
        .centered()
        .theme(|state: &FerriteBrowser| state.theme_mode.to_iced_theme())
        .subscription(subscription)
        .run_with(|| {
            // C3a: `.window_size(...).centered()` above is only the frame
            // before the window manager has a chance to place it — the
            // startup task below immediately requests maximized, so the
            // app opens filling the display rather than the small
            // fixed-size centered window it used to. `get_scale_factor`
            // in the same chain is what makes the Servo render buffer and
            // every pointer-event coordinate correct on a HiDPI/Retina
            // display — see `scale_factor`'s doc comment on
            // `FerriteBrowser` for why both bugs (blurry frame, hover/
            // click landing in the wrong place) traced back to this
            // never being queried at all.
            let startup_window_task = window::get_latest().then(|id| match id {
                Some(id) => Task::batch([
                    window::maximize(id, true),
                    window::get_scale_factor(id).map(FerriteBrowserMessage::ScaleFactorReady),
                ]),
                None => Task::none(),
            });

            let mut state = FerriteBrowser::default();
            // T-224/T-229: construct the real ModelProvider here, at actual
            // app startup — never inside `FerriteBrowser::default()` itself
            // (kept test-safe/R7, see that impl's own comment) — and only
            // once, since `try_real_model_provider()` does a real OS-keyring
            // lookup that a test must never trigger even indirectly.
            if let Some((provider, config)) = try_real_model_provider() {
                state.model_tag_small = config.tag(ModelTier::Small).to_string();
                state.model_tag_main = config.tag(ModelTier::Main).to_string();
                state.model_provider = provider;
            } else {
                eprintln!(
                    "[ferrite-ui] no ModelProvider configured/reachable (FERRITE_MODEL_SMALL/\
                     FERRITE_MODEL_MAIN unset, or no Ollama/Gemini credential found) — the \
                     fingerprint's may_use layer and the live agent loop will both fail to \
                     empty/fail closed rather than run against a real model"
                );
            }
            match HeadlessServoSession::new(1280, 700) {
                Ok(session) => {
                    state.servo_sessions.insert(0, session);
                    (
                        state,
                        Task::batch([
                            Task::done(FerriteBrowserMessage::ServoReady),
                            startup_window_task,
                        ]),
                    )
                }
                Err(e) => {
                    eprintln!("[ferrite-ui] Servo unavailable: {}", e);
                    (state, startup_window_task)
                }
            }
        })
}

// ---------------------------------------------------------------------------
// Tests — the consent panel's state machine, summary rendering, and real
// enforcement, per `docs/REBUILD_DIRECTIVE.md` §6/A10's test requirements.
//
// No test here constructs a `HeadlessServoSession` (R7/no live resources) —
// `FerriteBrowser::default()` never does either, so `update()` is directly
// testable with a plain `FerriteBrowser` and no window, matching the
// directive's "no rendering needed" requirement. `FerriteBrowser::default()`
// also never calls `try_real_model_provider()` (that only happens in
// `launch()`, the real app entry point — see that impl's own comment), so
// every test's `model_provider` is `MockProvider`, scripted with nothing —
// R7 holds even for the tests below that do trigger a `tokio::task::spawn`.
//
// `#[tokio::test]` is used wherever a message handler calls
// `tokio::task::spawn` (`ConsentSubmitted`/`LiveRunReady`/`AgentStepReady`)
// — spawning requires an active Tokio runtime context or it panics, even
// though the test never awaits the spawned future into existence. Since
// none of these tests `.await` anything after triggering the spawn, the
// current-thread test runtime never actually polls the spawned task before
// the test ends, so the step's `ModelProvider::complete` call inside it
// never executes at all — and even if it somehow did, `MockProvider` with
// nothing scripted only ever returns a typed `ModelError`, never reaches a
// socket. No live call is made, per R7.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // ── Fixtures ─────────────────────────────────────────────────────────

    fn sample_expected() -> ExpectedFingerprint {
        ExpectedFingerprint::from_capabilities(
            ferrite_core::ExpectedCapabilitySet::new(vec![ferrite_core::ExpectedCapability::new(
                ferrite_core::Capability::WebRead,
                ferrite_core::scope::OriginScope::exact([ferrite_core::Origin::parse(
                    "https://example.com",
                )
                .unwrap()])
                .unwrap(),
            )])
            .unwrap(),
        )
    }

    fn diff_with_extra_primitive() -> FingerprintDiff {
        let mut diff = FingerprintDiff::default();
        diff.extra_primitives.insert(ToolId::new("js.execute"));
        diff
    }

    fn diff_with_out_of_scope_origin() -> FingerprintDiff {
        let mut diff = FingerprintDiff::default();
        diff.out_of_scope_origins
            .insert("https://attacker.example".to_string());
        diff
    }

    fn mixed_diff() -> FingerprintDiff {
        let mut diff = FingerprintDiff::default();
        diff.extra_primitives.insert(ToolId::new("js.execute"));
        diff.out_of_scope_origins
            .insert("https://attacker.example".to_string());
        diff
    }

    /// Builds an empty `DryRunRecord` tied to `task`'s ids, without needing a
    /// direct `uuid` dependency in this crate.
    fn evidence_for(task: &IpiTask) -> DryRunRecord {
        DryRunRecord::new(task.session_id, task.task_id)
    }

    // ── R7: no automated test call reaches a live ModelProvider ──────────

    /// Mirrors `ferrite-eval::harness::no_automated_test_calls_try_real_provider`
    /// (same technique, scoped to this crate's one source file): scans this
    /// module's own source for every non-comment, non-definition call site
    /// of `try_real_model_provider(` and fails if more than the one real
    /// caller (`launch()`) exists. `FerriteBrowser::default()` itself never
    /// names the function at all (it constructs `MockProvider` inline —
    /// see that impl), so this test's job is only to catch a future edit
    /// that accidentally adds a second call site somewhere a test could
    /// reach.
    #[test]
    fn no_automated_test_calls_try_real_model_provider_outside_launch() {
        const NEEDLE: &str = "try_real_model_provider(";
        let src = include_str!("lib.rs");
        let mut real_call_sites = 0;
        for (i, line) in src.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            let mut search_from = 0;
            while let Some(rel) = line[search_from..].find(NEEDLE) {
                let idx = search_from + rel;
                let preceded_by_ident_char = line[..idx]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                let is_definition = line[..idx].trim_end().ends_with("fn");
                let in_string_literal = line[..idx].matches('"').count() % 2 == 1;
                if !preceded_by_ident_char && !is_definition && !in_string_literal {
                    real_call_sites += 1;
                    assert!(
                        line.contains("try_real_model_provider()"),
                        "line {}: unexpected call shape: {line}",
                        i + 1
                    );
                }
                search_from = idx + NEEDLE.len();
            }
        }
        assert_eq!(
            real_call_sites, 1,
            "expected exactly one real call site (launch()) — found {real_call_sites}; a new \
             one would risk a live OS-keyring lookup reaching cargo test (R7)"
        );
    }

    /// A fresh `LiveAgentLoop` with the default budget and no consent
    /// rejections — the shape `start_live_loop` builds for a
    /// bypassed/clean-dry-run task.
    fn fresh_live_loop() -> LiveAgentLoop {
        LiveAgentLoop {
            messages: vec![Message::user("go")],
            actions_taken: Vec::new(),
            started_at: std::time::Instant::now(),
            budget: LoopBudget::default(),
            rejected: Default::default(),
            rejected_origins: Default::default(),
            consecutive_malformed: 0,
        }
    }

    // ── consent_items: the plain-English summary snapshot ───────────────

    #[test]
    fn consent_summary_snapshot_for_a_mixed_diff() {
        let diff = mixed_diff();
        let expected = sample_expected();

        let items = consent_items(&diff, Some(&expected));
        assert_eq!(items.len(), 2, "{items:?}");

        assert_eq!(items[0].id, ToolId::new("js.execute"));
        assert_eq!(
            items[0].summary,
            "Used tool: js.execute — arbitrary JavaScript execution is always reviewed by \
             design (it can synthesize any other action); nothing in your request could have \
             authorized it."
        );

        assert_eq!(items[1].id, origin_item_id("https://attacker.example"));
        assert_eq!(
            items[1].summary,
            "Contacted https://attacker.example — this origin is not authorized. Your request \
             authorized: https://example.com."
        );
    }

    #[test]
    fn consent_summary_for_a_non_js_extra_primitive_says_nothing_authorized_it() {
        let mut diff = FingerprintDiff::default();
        diff.extra_primitives.insert(ToolId::new("dom.write"));
        let items = consent_items(&diff, None);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].summary,
            "Used tool: dom.write — nothing in your request authorized this action."
        );
    }

    #[test]
    fn consent_summary_for_an_out_of_scope_origin_with_no_expected_fingerprint_is_honest() {
        let diff = diff_with_out_of_scope_origin();
        let items = consent_items(&diff, None);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].summary,
            "Contacted https://attacker.example — this origin is not authorized. Your request \
             authorized: nothing in your request authorized any origin."
        );
    }

    #[test]
    fn consent_is_complete_requires_a_decision_on_both_bucket_kinds() {
        let diff = mixed_diff();
        let mut decision = ConsentDecision::default();
        assert!(!consent_is_complete(&diff, &decision));

        decision.reject(ToolId::new("js.execute"));
        assert!(
            !consent_is_complete(&diff, &decision),
            "the out-of-scope-origin item is still undecided"
        );

        decision.approve(origin_item_id("https://attacker.example"));
        assert!(consent_is_complete(&diff, &decision));
    }

    // ── update() state-machine tests ─────────────────────────────────────

    #[test]
    fn consent_required_populates_pending_state_fresh_and_clears_agent_running() {
        let mut state = FerriteBrowser {
            agent_is_running: true,
            ..FerriteBrowser::default()
        };
        let task = IpiTask::new("check my inbox", Some("https://example.com".to_string()));
        let diff = diff_with_extra_primitive();
        let expected = sample_expected();

        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff.clone(),
                expected: expected.clone(),
                evidence: Box::new(evidence_for(&task)),
            },
        );

        assert_eq!(state.pending_diff, Some(diff));
        assert_eq!(state.pending_expected, Some(expected));
        assert!(state.pending_evidence.is_some());
        assert!(state.pending_decision.approved.is_empty());
        assert!(state.pending_decision.rejected.is_empty());
        assert!(!state.agent_is_running);
    }

    #[tokio::test]
    async fn consent_submitted_is_a_no_op_while_any_item_is_undecided() {
        let mut state = FerriteBrowser::default();
        let task = IpiTask::new("t", None);
        state.pending_task = Some(task.prompt.clone());
        let diff = diff_with_extra_primitive();
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff.clone(),
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task)),
            },
        );

        // Nothing decided yet — must be a no-op even though the message was
        // sent directly, bypassing the view's disabled Proceed button. This
        // is the defense-in-depth guard: ConsentSubmitted must never be
        // reachable with an undecided item, from any caller.
        let _ = update(&mut state, FerriteBrowserMessage::ConsentSubmitted);
        assert!(
            state.pending_diff.is_some(),
            "must not proceed while undecided"
        );
        assert!(state.pending_task.is_some());
        assert!(state.agent_handle.is_none());
    }

    #[tokio::test]
    async fn consent_flow_extra_primitive_only_reject_then_submit_clears_all_pending_state() {
        let mut state = FerriteBrowser::default();
        let task = IpiTask::new("t", None);
        state.pending_task = Some(task.prompt.clone());
        let diff = diff_with_extra_primitive();
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff.clone(),
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task)),
            },
        );

        let _ = update(
            &mut state,
            FerriteBrowserMessage::RejectTool("js.execute".to_string()),
        );
        assert!(consent_is_complete(&diff, &state.pending_decision));

        let _ = update(&mut state, FerriteBrowserMessage::ConsentSubmitted);

        assert!(state.pending_diff.is_none());
        assert!(state.pending_expected.is_none());
        assert!(state.pending_evidence.is_none());
        assert!(state.pending_decision.approved.is_empty());
        assert!(state.pending_decision.rejected.is_empty());
        assert!(state.pending_task.is_none());
        assert!(!state.show_evidence);
        assert!(
            state.agent_handle.is_some(),
            "an approved-decision run must actually be spawned"
        );
    }

    #[tokio::test]
    async fn consent_flow_out_of_scope_origin_only_approve_then_submit() {
        let mut state = FerriteBrowser::default();
        let task = IpiTask::new("t", None);
        state.pending_task = Some(task.prompt.clone());
        let diff = diff_with_out_of_scope_origin();
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff.clone(),
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task)),
            },
        );

        let item_id = origin_item_id("https://attacker.example").to_string();
        let _ = update(&mut state, FerriteBrowserMessage::ApproveTool(item_id));
        assert!(consent_is_complete(&diff, &state.pending_decision));

        let _ = update(&mut state, FerriteBrowserMessage::ConsentSubmitted);
        assert!(state.pending_diff.is_none());
        assert!(state.agent_handle.is_some());
    }

    #[tokio::test]
    async fn consent_flow_mixed_diff_requires_both_items_decided_before_submit_succeeds() {
        let mut state = FerriteBrowser::default();
        let task = IpiTask::new("t", None);
        state.pending_task = Some(task.prompt.clone());
        let diff = mixed_diff();
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff.clone(),
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task)),
            },
        );

        let _ = update(
            &mut state,
            FerriteBrowserMessage::RejectTool("js.execute".to_string()),
        );
        let _ = update(&mut state, FerriteBrowserMessage::ConsentSubmitted);
        assert!(
            state.pending_diff.is_some(),
            "one undecided item (the out-of-scope origin) must still block submission"
        );

        let origin_id = origin_item_id("https://attacker.example").to_string();
        let _ = update(&mut state, FerriteBrowserMessage::RejectTool(origin_id));
        let _ = update(&mut state, FerriteBrowserMessage::ConsentSubmitted);
        assert!(state.pending_diff.is_none());
    }

    #[test]
    fn consent_cancelled_clears_all_pending_state_without_spawning_a_run() {
        let mut state = FerriteBrowser::default();
        let task = IpiTask::new("t", None);
        state.pending_task = Some(task.prompt.clone());
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff_with_extra_primitive(),
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task)),
            },
        );
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ApproveTool("js.execute".to_string()),
        );

        let _ = update(&mut state, FerriteBrowserMessage::ConsentCancelled);

        assert!(state.pending_diff.is_none());
        assert!(state.pending_expected.is_none());
        assert!(state.pending_evidence.is_none());
        assert!(state.pending_task.is_none());
        assert!(state.pending_decision.approved.is_empty());
        assert!(!state.show_evidence);
        assert!(!state.agent_is_running);
        assert!(
            state.agent_handle.is_none(),
            "cancel must never spawn a run"
        );
    }

    #[tokio::test]
    async fn second_tasks_consent_decision_never_carries_over_from_the_first() {
        let mut state = FerriteBrowser::default();

        // Task 1: flagged with js.execute, approved, submitted.
        let task1 = IpiTask::new("task one", None);
        state.pending_task = Some(task1.prompt.clone());
        let diff1 = diff_with_extra_primitive();
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff1,
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task1)),
            },
        );
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ApproveTool("js.execute".to_string()),
        );
        let _ = update(&mut state, FerriteBrowserMessage::ConsentSubmitted);
        assert!(state.pending_decision.approved.is_empty());

        // Task 2: flagged with the SAME tool id, but never decided this time
        // — if approval leaked across tasks this would wrongly read as
        // already-decided.
        let task2 = IpiTask::new("task two", None);
        state.pending_task = Some(task2.prompt.clone());
        let diff2 = diff_with_extra_primitive();
        let _ = update(
            &mut state,
            FerriteBrowserMessage::ConsentRequired {
                diff: diff2.clone(),
                expected: sample_expected(),
                evidence: Box::new(evidence_for(&task2)),
            },
        );

        assert!(
            !consent_is_complete(&diff2, &state.pending_decision),
            "task 2's identical tool id must NOT be pre-approved from task 1's decision"
        );
        assert!(state.pending_decision.approved.is_empty());
        assert!(state.pending_decision.rejected.is_empty());
    }

    // ── is_action_rejected: the pure enforcement predicate ────────────────

    #[test]
    fn is_action_rejected_matches_on_tool_id() {
        let rejected: std::collections::HashSet<ToolId> =
            [ToolId::new("js.execute")].into_iter().collect();
        let action = AgentAction::JsExecute {
            script: "1+1".to_string(),
        };
        assert!(is_action_rejected(&action, &rejected, &Default::default()));
    }

    #[test]
    fn is_action_rejected_matches_on_navigate_origin() {
        let rejected_origins: std::collections::HashSet<String> =
            ["https://attacker.example".to_string()]
                .into_iter()
                .collect();
        let action = AgentAction::Navigate {
            url: "https://attacker.example/payload".to_string(),
        };
        assert!(is_action_rejected(
            &action,
            &Default::default(),
            &rejected_origins
        ));
    }

    #[test]
    fn is_action_rejected_is_false_for_an_undecided_action() {
        let action = AgentAction::ReadDom;
        assert!(!is_action_rejected(
            &action,
            &Default::default(),
            &Default::default()
        ));
    }

    // ── Real enforcement: a rejected live-loop step is actually blocked,
    // never reaching `execute_action`/the engine ──────────────────────────
    //
    // The automated suite has no real Servo session (R7 — see
    // `FerriteBrowser::default()`'s `servo_sessions: HashMap::new()`, never
    // populated outside a real `AddTab` against a real `servo` feature
    // build), so `AgentStepReady`'s handler always falls through to its own
    // "no active browser session" branch once past the rejection check.
    // These tests still prove real enforcement, not just the pure
    // predicate above: the handler's branch order means the observation
    // string can only say "blocked by user consent" if the rejection
    // branch fired *before* the no-session branch — "no active browser
    // session" is a structurally distinct string the handler would have
    // produced instead had the action not been rejected.

    #[tokio::test]
    async fn a_rejected_tool_id_blocks_the_action_before_it_reaches_the_engine() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let mut live = fresh_live_loop();
        live.rejected = [ToolId::new("js.execute")].into_iter().collect();
        state.live_loop = Some(live);

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::JsExecute {
                    script: "1+1".to_string(),
                }),
            },
        );

        let live = state
            .live_loop
            .expect("the loop continues after a blocked, non-Finish action");
        let last = live.messages.last().expect("an observation was recorded");
        assert!(
            last.content.contains("blocked by user consent"),
            "rejected tool id must be blocked, not executed: {last:?}"
        );
    }

    #[tokio::test]
    async fn a_rejected_origin_blocks_a_navigate_before_it_reaches_the_engine() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let mut live = fresh_live_loop();
        live.rejected_origins = ["https://attacker.example".to_string()]
            .into_iter()
            .collect();
        state.live_loop = Some(live);

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::Navigate {
                    url: "https://attacker.example/payload".to_string(),
                }),
            },
        );

        let live = state.live_loop.expect("the loop continues");
        let last = live.messages.last().expect("an observation was recorded");
        assert!(
            last.content.contains("blocked by user consent"),
            "rejected-origin navigate must be blocked, not executed: {last:?}"
        );
    }

    #[tokio::test]
    async fn a_non_rejected_action_reaches_the_no_session_fallback_not_the_consent_block() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::ReadDom),
            },
        );

        let live = state.live_loop.expect("the loop continues");
        let last = live.messages.last().expect("an observation was recorded");
        assert!(
            !last.content.contains("blocked by user consent"),
            "a non-rejected action must not be reported as consent-blocked: {last:?}"
        );
    }

    // ── C2: agent_log — real per-step icon/label/detail/result ────────────

    #[test]
    fn action_label_and_detail_are_distinct_per_action_kind() {
        assert_eq!(
            action_label(&AgentAction::Navigate {
                url: "https://a.example".to_string()
            }),
            "Navigate"
        );
        assert_eq!(
            action_detail(&AgentAction::Navigate {
                url: "https://a.example".to_string()
            }),
            "https://a.example"
        );
        assert_eq!(
            action_label(&AgentAction::Click {
                selector: "#go".to_string()
            }),
            "Click"
        );
        assert_eq!(
            action_detail(&AgentAction::Click {
                selector: "#go".to_string()
            }),
            "#go"
        );
        assert_eq!(
            action_detail(&AgentAction::TypeText {
                selector: "#q".to_string(),
                text: "hello".to_string(),
            }),
            "#q \u{2192} \"hello\""
        );
        assert_eq!(action_label(&AgentAction::WaitIdle), "Wait");
        assert_eq!(action_detail(&AgentAction::WaitIdle), "");
    }

    #[test]
    fn icon_for_action_groups_by_action_class_not_one_icon_per_variant() {
        // Navigate group shares one icon regardless of which navigation
        // variant fired.
        assert_eq!(
            icon_for_action(&AgentAction::Navigate { url: String::new() }),
            icon_for_action(&AgentAction::GoBack)
        );
        assert_eq!(
            icon_for_action(&AgentAction::GoBack),
            icon_for_action(&AgentAction::Reload)
        );
        // But a genuinely different class gets a different icon.
        assert_ne!(
            icon_for_action(&AgentAction::GoBack),
            icon_for_action(&AgentAction::Click {
                selector: String::new()
            })
        );
        // Finish/JsExecute deliberately reuse an existing chrome icon
        // rather than a dedicated one — pinned so that reuse stays
        // intentional, not silently regressed to something else later.
        assert_eq!(
            icon_for_action(&AgentAction::Finish {
                answer: String::new()
            }),
            Icon::Approve
        );
        assert_eq!(
            icon_for_action(&AgentAction::JsExecute {
                script: String::new()
            }),
            Icon::Console
        );
    }

    #[tokio::test]
    async fn agent_step_ready_records_a_real_step_with_its_actual_result() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::Click {
                    selector: "#submit".to_string(),
                }),
            },
        );

        assert_eq!(state.agent_log.len(), 1);
        match &state.agent_log[0] {
            AgentLogEntry::Step {
                icon: step_icon,
                label,
                detail,
                result,
                blocked,
            } => {
                assert_eq!(*step_icon, Icon::Click);
                assert_eq!(*label, "Click");
                assert_eq!(detail, "#submit");
                assert!(!blocked);
                // No active Servo session in this test fixture (R7 — no
                // real session is ever constructed outside launch()), so
                // the real fallback string, not a fabricated one, must
                // land in the step's own result field.
                assert_eq!(result, "error: no active browser session");
            }
            other => panic!("expected a Step entry, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn agent_step_ready_records_a_blocked_step_as_blocked_not_a_silent_success() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let mut live = fresh_live_loop();
        live.rejected = [ToolId::new("js.execute")].into_iter().collect();
        state.live_loop = Some(live);

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::JsExecute {
                    script: "1+1".to_string(),
                }),
            },
        );

        assert_eq!(state.agent_log.len(), 1);
        match &state.agent_log[0] {
            AgentLogEntry::Step {
                blocked, result, ..
            } => {
                assert!(
                    *blocked,
                    "a rejected action's step must record blocked = true"
                );
                assert_eq!(result, "blocked by user consent");
            }
            other => panic!("expected a Step entry, got {other:?}"),
        }
    }

    // ── ServoFrame: resize-settle window (real segfault/corruption fix) ───

    #[test]
    fn servo_frame_decrements_resize_settle_ticks_and_stops_at_zero() {
        let mut state = FerriteBrowser {
            resize_settle_ticks: 2,
            ..FerriteBrowser::default()
        };
        let _ = update(&mut state, FerriteBrowserMessage::ServoFrame);
        assert_eq!(state.resize_settle_ticks, 1);
        let _ = update(&mut state, FerriteBrowserMessage::ServoFrame);
        assert_eq!(state.resize_settle_ticks, 0);
        let _ = update(&mut state, FerriteBrowserMessage::ServoFrame);
        assert_eq!(
            state.resize_settle_ticks, 0,
            "must not underflow past zero on a later tick"
        );
    }

    #[test]
    fn servo_frame_does_not_arm_the_resize_settle_window_without_a_session_to_resize() {
        // R7: no HeadlessServoSession is ever constructed in this test
        // build (the stub's own `new()` always returns `Err`), so
        // `servo_sessions` is always empty here — this proves the settle
        // window only ever arms as a *result* of an actual `resize()`
        // call, not speculatively just because the measured content area
        // differs from `last_resized_content_px`.
        let mut state = FerriteBrowser {
            content_area_size: Cell::new(Size::new(999.0, 999.0)),
            ..FerriteBrowser::default()
        };
        assert_eq!(state.last_resized_content_px, (1280, 700));

        let _ = update(&mut state, FerriteBrowserMessage::ServoFrame);

        assert_eq!(
            state.resize_settle_ticks, 0,
            "no session existed to resize, so nothing should have armed the settle window"
        );
        assert_eq!(
            state.last_resized_content_px,
            (1280, 700),
            "must not be updated unless a resize() call actually happened"
        );
    }

    // ── AgentStepReady/LiveRunReady: message-driven step-loop plumbing ────

    #[test]
    fn a_stale_run_id_is_ignored_by_live_run_ready() {
        let mut state = FerriteBrowser {
            run_id: 2,
            ..FerriteBrowser::default()
        };
        let _ = update(
            &mut state,
            FerriteBrowserMessage::LiveRunReady {
                run_id: 1,
                prompt: "go".to_string(),
            },
        );
        assert!(
            state.live_loop.is_none(),
            "a stale run_id must never start a live loop"
        );
        assert!(!state.agent_is_running);
    }

    #[test]
    fn a_stale_run_id_is_ignored_by_agent_step_ready() {
        let mut state = FerriteBrowser {
            run_id: 2,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());
        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::Finish {
                    answer: "should never apply".to_string(),
                }),
            },
        );
        assert!(
            state.live_loop.is_some(),
            "a stale run_id's step must not be allowed to finish the current loop"
        );
        assert!(state.agent_response.is_none());
    }

    #[tokio::test]
    async fn live_run_ready_with_a_current_run_id_starts_the_loop() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let _ = update(
            &mut state,
            FerriteBrowserMessage::LiveRunReady {
                run_id: 1,
                prompt: "go".to_string(),
            },
        );
        assert!(state.live_loop.is_some());
        assert!(state.agent_is_running);
    }

    #[test]
    fn agent_step_ready_with_finish_completes_the_task() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());
        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::Finish {
                    answer: "done".to_string(),
                }),
            },
        );
        assert_eq!(state.agent_response.as_deref(), Some("done"));
        assert!(!state.agent_is_running);
        assert!(state.live_loop.is_none());
    }

    #[test]
    fn agent_step_ready_with_a_model_error_fails_the_task() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());
        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Err(StepFailure::Model("no provider configured".to_string())),
            },
        );
        assert_eq!(
            state.agent_response.as_deref(),
            Some("[error] no provider configured")
        );
        assert!(!state.agent_is_running);
    }

    #[tokio::test]
    async fn agent_step_ready_with_a_malformed_action_retries_instead_of_failing_immediately() {
        // Real, reproduced failure mode: a `finish.answer` truncated
        // mid-string by the provider's output cap comes back as
        // unparseable JSON. A single such glitch must not end the whole
        // task the way it did before this fix — it must be fed back to
        // the model as a retry, with the run still alive afterward.
        let mut state = FerriteBrowser {
            run_id: 1,
            agent_is_running: true,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());
        let messages_before = state.live_loop.as_ref().unwrap().messages.len();

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Err(StepFailure::Malformed {
                    raw: r#"{"action":"finish","answer":"This page is a"#.to_string(),
                    message: "EOF while parsing a string at line 1 column 44".to_string(),
                }),
            },
        );

        assert!(
            state.agent_is_running,
            "a single malformed response must not end the run while retries remain"
        );
        assert!(
            state.agent_response.is_none(),
            "no raw parser error should ever reach the user while a retry is still possible"
        );
        let live = state
            .live_loop
            .expect("the loop must still be alive, retrying the step");
        assert_eq!(live.consecutive_malformed, 1);
        assert_eq!(
            live.messages.len(),
            messages_before + 2,
            "a retry-with-feedback pair (the raw response, then a request to retry) must \
             have been appended to the conversation"
        );
    }

    #[test]
    fn agent_step_ready_gives_up_after_max_consecutive_malformed_responses() {
        // A model stuck producing garbage must still fail fast rather than
        // retry forever — bounded by MAX_CONSECUTIVE_MALFORMED_STEPS, not
        // by the step budget (a retry never counts as a step).
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let mut live = fresh_live_loop();
        live.consecutive_malformed = MAX_CONSECUTIVE_MALFORMED_STEPS;
        state.live_loop = Some(live);

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Err(StepFailure::Malformed {
                    raw: "still garbage".to_string(),
                    message: "EOF while parsing a string".to_string(),
                }),
            },
        );

        assert!(
            state
                .agent_response
                .as_deref()
                .unwrap_or_default()
                .contains("EOF while parsing a string"),
            "the real parse error must surface once retries are exhausted: {:?}",
            state.agent_response
        );
        assert!(!state.agent_is_running);
        assert!(state.live_loop.is_none());
    }

    #[test]
    fn repeated_identical_actions_stop_the_live_loop_before_a_third_execution() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let action = AgentAction::Click {
            selector: "#retry".to_string(),
        };
        let mut live = fresh_live_loop();
        live.actions_taken = vec![action.clone(), action.clone()];
        live.budget.max_repeated_identical = 3;
        state.live_loop = Some(live);

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(action),
            },
        );

        assert!(
            state.live_loop.is_none(),
            "the loop must stop, not repeat a third time"
        );
        assert!(!state.agent_is_running);
        assert!(state
            .agent_response
            .as_deref()
            .unwrap_or_default()
            .contains("repeat"));
    }

    #[tokio::test]
    async fn step_budget_exhausted_stops_the_loop_instead_of_spawning_another_step() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        let mut live = fresh_live_loop();
        live.budget.max_steps = 1;
        state.live_loop = Some(live);

        let _ = update(
            &mut state,
            FerriteBrowserMessage::AgentStepReady {
                run_id: 1,
                action: Ok(AgentAction::ReadDom),
            },
        );

        assert!(
            state.live_loop.is_none(),
            "the loop must stop once its one allowed step has executed"
        );
        assert!(!state.agent_is_running);
        assert!(state
            .agent_response
            .as_deref()
            .unwrap_or_default()
            .contains("step budget"));
    }

    #[test]
    fn stop_agent_bumps_run_id_and_clears_the_live_loop() {
        let mut state = FerriteBrowser {
            run_id: 1,
            ..FerriteBrowser::default()
        };
        state.live_loop = Some(fresh_live_loop());
        state.agent_is_running = true;

        let _ = update(&mut state, FerriteBrowserMessage::StopAgent);

        assert_eq!(state.run_id, 2);
        assert!(state.live_loop.is_none());
        assert!(!state.agent_is_running);
    }

    // ── Dry-run evidence rendering ────────────────────────────────────────

    #[test]
    fn dry_run_evidence_lines_render_ordered_call_log() {
        let task = IpiTask::new("t", None);
        let mut record = evidence_for(&task);
        record.record_tool(
            ferrite_core::Primitive::Navigate,
            Some("https://example.com".to_string()),
        );
        record.record_tool(
            ferrite_core::Primitive::DomRead,
            Some("https://example.com".to_string()),
        );
        record.record_tool(ferrite_core::Primitive::JsExecute, None);

        let lines = dry_run_evidence_lines(&record);
        assert_eq!(
            lines,
            vec![
                "#0  navigate  https://example.com",
                "#1  dom.read  https://example.com",
                "#2  js.execute  (no origin recorded)",
            ]
        );
    }

    #[test]
    fn dry_run_evidence_lines_says_so_honestly_when_nothing_was_recorded() {
        let task = IpiTask::new("t", None);
        let record = evidence_for(&task);
        assert_eq!(
            dry_run_evidence_lines(&record),
            vec!["No tool calls were recorded during the dry run."]
        );
    }

    // ── ease_out_cubic: the consent panel's entrance-transition curve ────

    #[test]
    fn ease_out_cubic_starts_at_zero_and_ends_at_one() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
    }

    #[test]
    fn ease_out_cubic_clamps_out_of_range_input() {
        assert_eq!(ease_out_cubic(-1.0), 0.0);
        assert_eq!(ease_out_cubic(2.0), 1.0);
    }

    #[test]
    fn ease_out_cubic_is_monotonically_non_decreasing() {
        let mut prev = ease_out_cubic(0.0);
        let mut t = 0.0_f32;
        while t <= 1.0 {
            let cur = ease_out_cubic(t);
            assert!(cur >= prev, "not monotonic at t={t}: {cur} < {prev}");
            prev = cur;
            t += 0.05;
        }
    }

    #[test]
    fn ease_out_cubic_is_ahead_of_linear_partway_through_an_ease_out_curve() {
        // The defining property of ease-*out*: it front-loads progress, so
        // at the midpoint it's already past halfway (unlike a linear or
        // ease-in curve).
        assert!(ease_out_cubic(0.5) > 0.5);
    }

    // ── C3c loading-indicator helpers ─────────────────────────────────────

    #[test]
    fn pulse_alpha_stays_within_base_and_base_plus_amplitude() {
        let mut t = 0.0_f32;
        while t <= 1.0 {
            let a = pulse_alpha(t, 1.7, 0.3, 0.5);
            assert!((0.3..=0.8 + f32::EPSILON).contains(&a), "t={t}: alpha={a}");
            t += 0.03;
        }
    }

    #[test]
    fn pulse_alpha_at_zero_is_exactly_base() {
        // sin(0) == 0, so the breath starts at its dimmest — matches the
        // "no light yet" reading at the very start of a loading state.
        assert_eq!(pulse_alpha(0.0, 1.0, 0.25, 0.20), 0.25);
    }

    #[test]
    fn progress_segment_brightness_peaks_at_the_segment_under_the_sweep() {
        // t=0.0 -> peak = 0 -> segment 0 is the brightest of the ten.
        let brightness_at: Vec<f32> = (0..10)
            .map(|i| progress_segment_brightness(i, 10, 0.0))
            .collect();
        let (brightest_i, _) = brightness_at
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        assert_eq!(brightest_i, 0);
    }

    #[test]
    fn progress_segment_brightness_wraps_around_the_segment_ring() {
        // t just before wrapping back to 0.0: the sweep's peak (~9.99 of
        // 10) sits between the last segment and the first. Without
        // wraparound, segment 0 would read as maximally *far* from the
        // peak (a hard drop at the seam, `raw = 9.99`); with it, segment 0
        // is close to the peak on the short way around (`wrapped ≈ 0.01`)
        // and reads brighter than a segment on the far side of the ring.
        let t = 0.999;
        let brightness_first = progress_segment_brightness(0, 10, t);
        let brightness_far_side = progress_segment_brightness(5, 10, t);
        assert!(
            brightness_first > brightness_far_side,
            "expected wraparound to keep segment 0 bright near the seam: \
             first={brightness_first} far_side={brightness_far_side}"
        );
    }

    #[test]
    fn progress_segment_brightness_is_bounded_zero_to_one() {
        for i in 0..PROGRESS_SEGMENTS {
            let mut t = 0.0_f32;
            while t <= 1.0 {
                let b = progress_segment_brightness(i, PROGRESS_SEGMENTS, t);
                assert!((0.0..=1.0).contains(&b), "i={i} t={t}: brightness={b}");
                t += 0.1;
            }
        }
    }

    // ── C3c theme toggle ───────────────────────────────────────────────────

    #[test]
    fn default_theme_is_dark_matching_pre_c3c_behavior() {
        let state = FerriteBrowser::default();
        assert_eq!(state.theme_mode, AppTheme::Dark);
    }

    #[test]
    fn toggle_theme_flips_dark_to_light_and_back() {
        let mut state = FerriteBrowser::default();
        let _ = update(&mut state, FerriteBrowserMessage::ToggleTheme);
        assert_eq!(state.theme_mode, AppTheme::Light);
        let _ = update(&mut state, FerriteBrowserMessage::ToggleTheme);
        assert_eq!(state.theme_mode, AppTheme::Dark);
    }

    #[test]
    fn each_theme_resolves_to_its_own_distinct_palette() {
        let mut state = FerriteBrowser::default();
        let dark_accent = state.palette().accent;
        let _ = update(&mut state, FerriteBrowserMessage::ToggleTheme);
        let light_accent = state.palette().accent;
        assert_ne!(
            (dark_accent.r, dark_accent.g, dark_accent.b),
            (light_accent.r, light_accent.g, light_accent.b),
            "toggling theme did not change the resolved accent colour"
        );
    }

    #[test]
    fn light_and_dark_palettes_each_keep_text_readable_on_base() {
        // Not a full contrast-ratio check (see `LIGHT_PALETTE`'s doc
        // comment for the hand-computed figures) — just the structural
        // property a palette must have to be usable at all: primary text
        // is not the same colour as the background it sits on, in either
        // theme.
        for palette in [&DARK_PALETTE, &LIGHT_PALETTE] {
            assert_ne!(
                (palette.text.r, palette.text.g, palette.text.b),
                (palette.base.r, palette.base.g, palette.base.b)
            );
        }
    }

    #[test]
    fn to_iced_theme_matches_app_theme() {
        assert_eq!(AppTheme::Dark.to_iced_theme(), Theme::Dark);
        assert_eq!(AppTheme::Light.to_iced_theme(), Theme::Light);
    }

    #[test]
    fn palette_for_theme_falls_back_to_dark_for_a_non_light_iced_theme() {
        // `palette_for_theme` is what the nine `.style(...)`-callback
        // functions use; it must resolve every `Theme` variant to
        // *something*; anything other than `Theme::Light` should be the
        // dark palette (see that function's own doc comment).
        let p = palette_for_theme(&Theme::Dark);
        assert_eq!(
            (p.base.r, p.base.g, p.base.b),
            (
                DARK_PALETTE.base.r,
                DARK_PALETTE.base.g,
                DARK_PALETTE.base.b
            )
        );
    }

    // ── Page-content decoupling: structural argument, not merely asserted ─

    /// This is not a runtime assertion so much as a compile-time witness:
    /// `view()`'s signature takes `&FerriteBrowser` and returns
    /// `Element<'_, FerriteBrowserMessage>` built exclusively from
    /// `iced_widget` constructors over plain Rust data
    /// (`String`/`FingerprintDiff`/`ExpectedFingerprint`/`DryRunRecord`).
    /// Page content never appears in any of those types — the only place
    /// Servo's page content enters this crate at all is
    /// `HeadlessServoSession::get_frame()`, which returns a decoded
    /// `(width, height, Vec<u8> RGBA bytes)` pixel buffer (see the `content`
    /// arm of `view()` for the one call site), rendered via
    /// `iced_widget::image::Image` — a bitmap, not markup Iced parses for
    /// style/position/layout. There is therefore no code path by which a
    /// page's HTML/CSS/text could set a style, position, or z-order on the
    /// consent panel: nothing in `FingerprintDiff`, `ExpectedFingerprint`,
    /// `DryRunRecord`, or `ConsentDecision` is ever populated from raw page
    /// markup, and even the one place page bytes DO reach this crate
    /// (`get_frame()`) they arrive as opaque pixels, never as a string Iced's
    /// widget tree would interpret. This test exists so that claim is
    /// pinned to a real function signature rather than left as a comment
    /// someone could invalidate without any test noticing.
    #[test]
    fn page_content_cannot_reach_the_consent_panels_inputs() {
        fn assert_view_signature(_f: fn(&FerriteBrowser) -> Element<'_, FerriteBrowserMessage>) {}
        assert_view_signature(view);

        // The only types a pending consent decision is built from — none of
        // them is, or contains, raw page markup.
        fn assert_consent_panel_inputs_are_plain_data(
            _diff: &FingerprintDiff,
            _expected: &ExpectedFingerprint,
            _evidence: &DryRunRecord,
            _decision: &ConsentDecision,
        ) {
        }
        let diff = FingerprintDiff::default();
        let expected = ExpectedFingerprint::empty();
        let task = IpiTask::new("t", None);
        let evidence = evidence_for(&task);
        let decision = ConsentDecision::default();
        assert_consent_panel_inputs_are_plain_data(&diff, &expected, &evidence, &decision);
    }
}
