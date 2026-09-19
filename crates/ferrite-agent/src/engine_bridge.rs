//! [`EngineToolExecutor`]: bridges the pre-existing `BrowserTool`/
//! `AgentRuntime`/`ToolExecutor` vocabulary onto any
//! `ferrite_engine::BrowserEngine` implementation.
//!
//! Added alongside `ferrite-ipi`'s B1 charter (`docs/TO-DO.md` T-221), which
//! rebuilt `ferrite-ipi::dry_run` on `ferrite_engine::BrowserEngine` directly
//! and removed `ferrite-ipi`'s dependency on this crate. That left a real
//! gap: `ferrite-eval`'s `WorstCaseAgent` and the live `GeminiAgent` (both
//! `AgentRuntime` implementors, both still live and untouched by B1 — see
//! `docs/DECISIONS.md`) need a way to drive `ferrite-ipi`'s new
//! `DryRunEngine` (or any other `BrowserEngine`) without either side being
//! rewritten. This module is that bridge, purely additive: nothing existing
//! in this crate is modified, deleted, or behaviorally changed.
//!
//! This crate already depends on `ferrite_engine` (for [`browser_loop`]), so
//! adding this module introduces no new dependency edge.
//!
//! # Why generic over a concrete `E: BrowserEngine + Send`, not `&mut dyn BrowserEngine`
//!
//! `ToolExecutor: Send + Sync` (this crate's existing bound — `AgentRuntime`
//! is driven across `async`/potential-thread boundaries). `BrowserEngine`
//! deliberately has **no** `Send` bound of its own (`ferrite_engine`'s module
//! docs: the real `ServoEngine` holds `Rc`-based state and cannot be `Send`).
//! A trait object `&mut dyn BrowserEngine` therefore cannot be proven `Send`
//! in general, even when the concrete type behind it happens to be. Being
//! generic over a concrete `E: BrowserEngine + Send` (as `ferrite-ipi`'s own
//! `DryRunEngine` is — plain owned fields, no `Rc`) sidesteps this
//! correctly: the bound is checked once, at the call site that knows the
//! concrete type, rather than asked of every possible engine.
//!
//! # Usage
//!
//! ```ignore
//! let mut engine = /* a ferrite_engine::BrowserEngine, e.g. ferrite_ipi::dry_run::DryRunEngine */;
//! let executor = ferrite_agent::engine_bridge::EngineToolExecutor::new(&mut engine);
//! let turn = existing_agent.run_turn(&task, &history, &executor).await?;
//! ```

use std::sync::Mutex;

use ferrite_engine::{BrowserEngine, EngineError};

use crate::{AgentToolCall, AgentToolResult, BrowserTool, ToolExecutor};

/// Adapts `&mut E` (any [`BrowserEngine`]) into a [`ToolExecutor`], so an
/// existing [`crate::AgentRuntime`] implementor can drive it without either
/// side changing its own vocabulary. See the [module docs](self).
pub struct EngineToolExecutor<'a, E: BrowserEngine + Send> {
    engine: Mutex<&'a mut E>,
}

impl<'a, E: BrowserEngine + Send> EngineToolExecutor<'a, E> {
    #[must_use]
    pub fn new(engine: &'a mut E) -> Self {
        Self {
            engine: Mutex::new(engine),
        }
    }
}

/// One-to-one mapping from the pre-existing [`BrowserTool`] vocabulary onto
/// [`BrowserEngine`]'s methods. `FillForm { selector, value }` (a single
/// field) maps to [`BrowserEngine::fill_form`]'s one-pair form rather than
/// [`BrowserEngine::type_text`] — `fill_form` is the semantically closer
/// action (entering a value into a named field), matching the tool's own
/// name.
fn dispatch<E: BrowserEngine>(engine: &mut E, tool: &BrowserTool) -> Result<String, EngineError> {
    match tool {
        BrowserTool::Navigate(url) => engine.navigate(url).map(|((), o)| o.as_str().to_string()),
        BrowserTool::ReadPage => engine
            .dom_snapshot()
            .map(|(snap, _)| snap.root.text.unwrap_or_default()),
        BrowserTool::ClickElement(selector) => {
            engine.click(selector).map(|((), o)| o.as_str().to_string())
        }
        BrowserTool::FillForm { selector, value } => engine
            .fill_form(&[(selector.clone(), value.clone())])
            .map(|((), o)| o.as_str().to_string()),
        BrowserTool::ExtractData(selector) => engine.read_text(selector).map(|(text, _)| text),
        BrowserTool::ReadClipboard => engine.clipboard_read().map(|(text, _)| text),
        BrowserTool::WriteClipboard(text) => engine
            .clipboard_write(text)
            .map(|((), o)| o.as_str().to_string()),
        BrowserTool::ExecuteJs(script) => engine.js_execute(script).map(|(result, _)| result),
        BrowserTool::DownloadFile(url) => engine.download(url).map(|(path, _)| path),
    }
}

#[async_trait::async_trait]
impl<'a, E: BrowserEngine + Send> ToolExecutor for EngineToolExecutor<'a, E> {
    async fn execute(&self, call: &AgentToolCall) -> AgentToolResult {
        // BrowserEngine's methods are synchronous (no real I/O in any
        // synthetic implementation, and the real ServoEngine's own
        // synchronization is internal to it) — the lock is held only for
        // the duration of one dispatch, never across an await point.
        let mut engine = self.engine.lock().unwrap();
        match dispatch(*engine, &call.tool) {
            Ok(value) => AgentToolResult::ok(call.call_id, value),
            Err(e) => AgentToolResult::err(call.call_id, e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_engine::MockEngine;

    #[tokio::test]
    async fn navigate_dispatches_to_the_engine_and_reports_success() {
        let mut engine = MockEngine::new();
        let executor = EngineToolExecutor::new(&mut engine);
        let call = AgentToolCall::new(BrowserTool::Navigate("https://a.example".to_string()));
        let result = executor.execute(&call).await;
        assert!(result.success, "{result:?}");
    }

    #[tokio::test]
    async fn read_page_serves_the_engines_dom_snapshot_text() {
        let mut engine = MockEngine::new();
        let origin = ferrite_core::Origin::parse(ferrite_engine::MOCK_HOME).unwrap();
        engine.seed_dom_snapshot(
            &origin,
            ferrite_engine::DomSnapshot {
                root: ferrite_engine::DomNode {
                    role: "document".to_string(),
                    text: Some("hello from the engine".to_string()),
                    ..Default::default()
                },
            },
        );
        let executor = EngineToolExecutor::new(&mut engine);
        let result = executor
            .execute(&AgentToolCall::new(BrowserTool::ReadPage))
            .await;
        assert!(result.success);
        assert_eq!(result.data.as_str(), Some("hello from the engine"));
    }

    #[tokio::test]
    async fn js_execute_reaches_the_engine_and_returns_its_result() {
        let mut engine = MockEngine::new();
        engine.seed_js("1+1", Ok("2".to_string()));
        let executor = EngineToolExecutor::new(&mut engine);
        let result = executor
            .execute(&AgentToolCall::new(BrowserTool::ExecuteJs(
                "1+1".to_string(),
            )))
            .await;
        assert!(result.success);
        assert_eq!(result.data.as_str(), Some("2"));
    }

    #[tokio::test]
    async fn an_engine_error_is_reported_as_a_failed_tool_result_not_a_panic() {
        let mut engine = MockEngine::new();
        let executor = EngineToolExecutor::new(&mut engine);
        // MockEngine::read_text returns Err(ElementNotFound) for a selector
        // with no scripted response — a real EngineError, not a stub.
        let result = executor
            .execute(&AgentToolCall::new(BrowserTool::ExtractData(
                "#never-scripted".to_string(),
            )))
            .await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("never-scripted"));
    }
}
