//! The dry run: execute the agent's plan against synthetic data instead of
//! the real browser, and hand back an ordered record of what it actually
//! did (`docs/REBUILD_DIRECTIVE.md` §6/A6).
//!
//! This is the middle third of the `predict → dry-run → compare → consent`
//! loop: [`crate::fingerprint`] predicts what the agent *should* need;
//! this module runs the agent for real against a
//! [`content::DryRunContent`] the case author scripted, so
//! [`crate::comparator`] (A7, not touched by this module) has something
//! concrete to compare the prediction against.
//!
//! # Shape
//!
//! - [`record`] — [`record::ToolEvent`]/[`record::DryRunRecord`]: the
//!   ordered event log itself. See that module's docs for why `ToolEvent`
//!   keeps the pre-existing `tool: ToolId` field (plus a new `seq: u64`)
//!   rather than the directive's illustrative `primitive: Primitive` name —
//!   short version: `crate::comparator` (off-limits this session) already
//!   depends on the old field, and [`record::ToolEvent::primitive`] is the
//!   tested bridge to `ferrite_core::Primitive` for A7 to adopt directly.
//! - [`content`] — [`content::DryRunContent`]/[`content::ReplyChannel`]:
//!   per-origin ordered reply queues, so a second read from the same origin
//!   can be scripted to return different content than the first (delayed
//!   payload / redirect chain).
//! - [`executor`] — [`executor::RecordingExecutor`]<sup>†</sup>: the
//!   `ToolExecutor` that serves scripted content, records every call, and
//!   never performs real I/O. See its module docs for the containment
//!   argument (T-007/D7).
//! - [`orchestrator`] — [`orchestrator::DryRunOrchestrator`]: wires a twin,
//!   content, and the executor around one agent turn, with a whole-turn
//!   timeout that returns the partial record rather than discarding it.
//!
//! <sup>†</sup> `RecordingExecutor` itself is crate-private
//! (`pub(crate)`) — the public surface is `DryRunOrchestrator::run`, which
//! is the only thing that needs to construct one.
//!
//! # Containment is by construction (closes `docs/TO-DO.md` T-007 / D7)
//!
//! The pre-rebuild crate had a `containment` module: an application-layer
//! request interceptor plus a Linux-only network-namespace creator, both
//! activated around every dry run. D7 named the actual state of things
//! precisely: the interceptor's `intercept_request()` was unreachable
//! outside its own unit test, because [`executor::RecordingExecutor`] never
//! made a real network call for it to intercept in the first place —
//! containment was already achieved by the executor never touching the
//! network, not by anything the deleted module did. Keeping tested-but-dead
//! interception code around reads as active protection when it is inert,
//! which `docs/REBUILD_DIRECTIVE.md` calls out as worse than having nothing
//! there. This session deleted that module outright (option (a) of the two
//! the directive offers, its stated default) rather than keeping it for a
//! hypothetical future real-executor dry-run backend — no such backend is
//! in the current plan; see `executor`'s module docs for the specific check
//! of the current tree, and `docs/handoffs/a06.md` for where to look again
//! if that plan ever changes.

pub mod content;
pub mod executor;
pub mod orchestrator;
pub mod record;

pub use content::{DryRunContent, DryRunReply, ReplyChannel};
pub use orchestrator::DryRunOrchestrator;
pub use record::{DryRunRecord, FindingCarrier, RecordedFinding, ToolEvent};
