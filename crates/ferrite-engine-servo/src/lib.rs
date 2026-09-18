//! `ServoEngine`: the real, Servo-backed [`BrowserEngine`] embedding
//! (`docs/REBUILD_DIRECTIVE.md` §6/A9, T-109).
//!
//! This crate wraps `ferrite_servo::session::HeadlessServoSession` — an
//! **existing**, feature-gated, real Servo integration — rather than
//! reimplementing raw `libservo`/winit plumbing. `HeadlessServoSession`
//! already compiles to a real Servo embedding behind `ferrite-servo`'s own
//! `servo` Cargo feature, and to an inert, always-succeeding stub when that
//! feature is off. This crate's own `engine-servo` feature does nothing
//! more than turn `ferrite-servo/servo` on:
//!
//! ```toml
//! [features]
//! engine-servo = ["ferrite-servo/servo"]
//! ```
//!
//! so `ServoEngine` compiles unconditionally (as part of a normal
//! `cargo build --workspace`, with zero Servo dependency cost — directive
//! §7.1), and only becomes a *real* browser when built with
//! `--features engine-servo` (`just build-servo`).
//!
//! # Mapping decisions — read before assuming a method "just works"
//!
//! `HeadlessServoSession` was built for `ferrite-shell`'s windowed UI, not
//! for an agent action surface, so several `BrowserEngine` methods are
//! implemented on top of primitives that were not designed for this
//! purpose. Documented plainly here, once, rather than scattered as
//! per-method surprises:
//!
//! - **`dom_snapshot` / `query` / `read_text` / `click` / `type_text` /
//!   `select_option`** are all implemented by injecting a small JavaScript
//!   snippet via `execute_js` that itself calls `JSON.stringify(...)`, and
//!   parsing the returned JSON. This depends on the page's JS engine being
//!   available and reflects only what page JS can see — it will not see
//!   content a page's own script could not see either (e.g. cross-origin
//!   iframe internals), and it fails if the page has disabled or not yet
//!   initialized its JS context. There is no separate, JS-independent
//!   accessibility-tree accessor exposed by `HeadlessServoSession` today.
//! - **`execute_js`'s `Ok` branch is a `Debug` rendering** of whatever
//!   value type Servo's `evaluate_javascript` callback produces, not the
//!   raw JS value (see `HeadlessServoSession::execute_js`'s own doc
//!   comment in `ferrite-servo`). For the `dom_snapshot`/`query`/
//!   `read_text` scripts above (which always return a JS string via
//!   `JSON.stringify`), this crate's [`unwrap_js_string_result`] strips the
//!   observed `String("...")` wrapping shape and unescapes the interior
//!   with `serde_json`'s string parser (JSON and Rust `Debug` string
//!   escaping coincide for the common ASCII/BMP case). This is a real,
//!   working mechanism, not a placeholder — but it depends on
//!   `execute_js`'s current, untyped return channel, and breaks if that
//!   channel's underlying type ever changes. It could not be exercised
//!   against a real running `WebView` in this session (see
//!   `docs/handoffs/a09.md` for why); `unwrap_js_string_result`'s own unit
//!   tests cover the shape this session's reading of `ferrite-servo`'s
//!   source predicts.
//! - **`js_execute` itself does *not* attempt this unwrapping.** A
//!   caller's arbitrary script can return any JS value type, each
//!   Debug-formatted differently, and guessing at all of them is out of
//!   scope here. `js_execute` returns `execute_js`'s raw string verbatim;
//!   a caller wanting a clean value should have their own script call
//!   `JSON.stringify()`, exactly as this crate's own internal scripts do.
//! - **`download` is unimplemented**: `HeadlessServoSession` has no
//!   download manager. Returns [`EngineError::Unsupported`].
//! - **`clipboard_read`/`clipboard_write` are unimplemented**:
//!   `HeadlessServoSession` exposes no synchronous clipboard API, and
//!   `navigator.clipboard` is Promise-based and permission-gated in a way
//!   `execute_js`'s single-shot synchronous callback cannot resolve.
//!   Returns [`EngineError::Unsupported`].
//! - **`cookies_read`/`storage_read` only succeed for the *currently
//!   loaded* page's own origin.** Both are implemented via `execute_js`
//!   reading `document.cookie`/`localStorage`, which only ever sees the
//!   active page's own origin — exactly like a same-origin script would.
//!   Rather than navigate away to satisfy a request for a different
//!   origin's data as a side effect of a "read" (a real, surprising
//!   behavior change this engine will not perform silently), a `scope`
//!   that does not match the active page's origin is
//!   [`EngineError::Unsupported`]. This is a real scoping restriction, not
//!   a missing feature dressed up as one: it makes reading another
//!   origin's cookies/storage impossible through this path, which is the
//!   security property `cookies_read`/`storage_read` scoping exists for.
//! - **Tabs are real**: each open tab is its own
//!   `HeadlessServoSession`, sharing the same process-wide Servo engine
//!   (`ferrite-servo`'s own `get_or_init_servo` singleton) — this mirrors
//!   how `ferrite-shell` already runs multiple tabs.
//! - **A fresh tab starts at `about:blank`**, a real, opaque origin — see
//!   `ferrite_engine`'s module docs for why this is a typed
//!   [`EngineError::OpaqueOrigin`] here, unlike `MockEngine`'s synthetic
//!   placeholder home origin.

#![deny(missing_docs)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ferrite_core::Origin;
use ferrite_engine::{
    BrowserEngine, Cookie, DomSnapshot, ElementHandle, EngineError, Frame, TabId, WaitCondition,
};
use ferrite_servo::session::{HeadlessServoSession, LoadStatus};

const NAV_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(16);

struct Tab {
    session: HeadlessServoSession,
}

/// The real, Servo-backed [`BrowserEngine`].
///
/// See the [module docs](self) for the full method-by-method mapping onto
/// `HeadlessServoSession`, including the methods this engine honestly
/// cannot support today (`download`, `clipboard_*`, cross-origin
/// `cookies_read`/`storage_read`).
pub struct ServoEngine {
    tabs: BTreeMap<TabId, Tab>,
    active: TabId,
    next_tab: u64,
    width: u32,
    height: u32,
}

impl std::fmt::Debug for ServoEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServoEngine")
            .field("tab_count", &self.tabs.len())
            .field("active", &self.active)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl ServoEngine {
    /// Creates a new engine with one tab open at `about:blank`, sized
    /// `width × height`.
    ///
    /// # Errors
    ///
    /// [`EngineError::Internal`] if `HeadlessServoSession::new` fails (e.g.
    /// the `servo` feature is compiled in but Servo's native init panics —
    /// `HeadlessServoSession::new` already converts that panic into an
    /// `Err` rather than aborting the process).
    pub fn new(width: u32, height: u32) -> Result<Self, EngineError> {
        let session = HeadlessServoSession::new(width, height).map_err(EngineError::Internal)?;
        let mut tabs = BTreeMap::new();
        let home = TabId(0);
        tabs.insert(home, Tab { session });
        Ok(Self {
            tabs,
            active: home,
            next_tab: 1,
            width,
            height,
        })
    }

    fn active_tab(&self) -> Result<&Tab, EngineError> {
        self.tabs
            .get(&self.active)
            .ok_or(EngineError::NoSuchTab(self.active))
    }

    fn active_tab_mut(&mut self) -> Result<&mut Tab, EngineError> {
        self.tabs
            .get_mut(&self.active)
            .ok_or(EngineError::NoSuchTab(self.active))
    }

    fn current_origin(&self) -> Result<Origin, EngineError> {
        let url = self.active_tab()?.session.current_url().to_string();
        Origin::parse(&url).map_err(|e| EngineError::OpaqueOrigin(format!("{url}: {e}")))
    }

    /// Pumps the active tab's session until it reports a settled load
    /// state or `NAV_TIMEOUT` elapses — mirrors
    /// `HeadlessServoSession::test_js_compat`'s own drive loop.
    fn drive_until_loaded(&mut self) -> Result<(), EngineError> {
        let deadline = Instant::now() + NAV_TIMEOUT;
        loop {
            let done = {
                let tab = self.active_tab_mut()?;
                tab.session.spin();
                matches!(
                    tab.session.load_status(),
                    LoadStatus::Complete | LoadStatus::Failed(_)
                )
            };
            if done || Instant::now() >= deadline {
                return Ok(());
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Runs `script` (which must itself call `JSON.stringify(...)`) and
    /// returns the unwrapped, unescaped JSON text it produced.
    fn run_json_js(&mut self, script: &str) -> Result<String, EngineError> {
        let raw = self
            .active_tab_mut()?
            .session
            .execute_js(script)
            .map_err(EngineError::Internal)?;
        unwrap_js_string_result(&raw)
    }
}

/// Best-effort unwrap of [`HeadlessServoSession::execute_js`]'s
/// `Debug`-rendered result back into the raw string a `JSON.stringify(...)`
/// script actually returned — see the [module docs](self) for the full
/// reasoning and its honest limits.
fn unwrap_js_string_result(raw: &str) -> Result<String, EngineError> {
    let trimmed = raw.trim();
    let quoted = trimmed
        .strip_prefix("String(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(trimmed);
    serde_json::from_str::<String>(quoted).map_err(|e| {
        EngineError::Internal(format!("could not unwrap execute_js result {raw:?}: {e}"))
    })
}

fn js_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

const SNAPSHOT_SCRIPT: &str = r#"(function(){
  function nodeInfo(el, depth) {
    if (!el || depth > 8) return null;
    var role = el.getAttribute('role') ||
      ({A:'link',BUTTON:'button',INPUT:'textbox',SELECT:'combobox',IMG:'img',
        H1:'heading',H2:'heading',H3:'heading'}[el.tagName]) || 'generic';
    var label = el.getAttribute('aria-label') || el.getAttribute('alt') || null;
    var text = null;
    for (var i = 0; i < el.childNodes.length; i++) {
      var n = el.childNodes[i];
      if (n.nodeType === 3 && n.textContent && n.textContent.trim()) {
        text = (text || '') + n.textContent.trim();
      }
    }
    var rect = el.getBoundingClientRect ? el.getBoundingClientRect() : null;
    var children = [];
    for (var i = 0; i < el.children.length; i++) {
      var c = nodeInfo(el.children[i], depth + 1);
      if (c) children.push(c);
    }
    return {
      role: role, label: label, text: text,
      selector: el.id ? ('#' + el.id) : null,
      bounds: rect ? {x: rect.x, y: rect.y, width: rect.width, height: rect.height} : null,
      children: children
    };
  }
  return JSON.stringify({root: nodeInfo(document.documentElement, 0) || {role:'generic',children:[]}});
})()"#;

impl BrowserEngine for ServoEngine {
    fn navigate(&mut self, url: &str) -> Result<((), Origin), EngineError> {
        self.active_tab_mut()?.session.navigate(url);
        self.drive_until_loaded()?;
        Ok(((), self.current_origin()?))
    }

    fn go_back(&mut self) -> Result<((), Origin), EngineError> {
        self.active_tab_mut()?.session.go_back();
        self.drive_until_loaded()?;
        Ok(((), self.current_origin()?))
    }

    fn go_forward(&mut self) -> Result<((), Origin), EngineError> {
        self.active_tab_mut()?.session.go_forward();
        self.drive_until_loaded()?;
        Ok(((), self.current_origin()?))
    }

    fn reload(&mut self) -> Result<((), Origin), EngineError> {
        self.active_tab_mut()?.session.reload();
        self.drive_until_loaded()?;
        Ok(((), self.current_origin()?))
    }

    fn current_url(&mut self) -> Result<(String, Origin), EngineError> {
        let url = self.active_tab()?.session.current_url().to_string();
        let origin = self.current_origin()?;
        Ok((url, origin))
    }

    fn dom_snapshot(&mut self) -> Result<(DomSnapshot, Origin), EngineError> {
        let json = self.run_json_js(SNAPSHOT_SCRIPT)?;
        let snapshot: DomSnapshot = serde_json::from_str(&json).map_err(|e| {
            EngineError::Internal(format!(
                "dom_snapshot: malformed JSON from page script: {e}"
            ))
        })?;
        Ok((snapshot, self.current_origin()?))
    }

    fn query(&mut self, selector: &str) -> Result<(Vec<ElementHandle>, Origin), EngineError> {
        let script = format!(
            "(function(sel){{ var out=[]; document.querySelectorAll(sel).forEach(function(el,i){{ \
             out.push({{selector: sel+':nth-match('+i+')', role: el.getAttribute('role')||el.tagName.toLowerCase(), \
             text: el.textContent?el.textContent.trim():null}}); }}); return JSON.stringify(out); }})({})",
            js_string_literal(selector)
        );
        let json = self.run_json_js(&script)?;
        let handles: Vec<ElementHandle> = serde_json::from_str(&json).map_err(|e| {
            EngineError::Internal(format!("query: malformed JSON from page script: {e}"))
        })?;
        Ok((handles, self.current_origin()?))
    }

    fn read_text(&mut self, selector: &str) -> Result<(String, Origin), EngineError> {
        let script = format!(
            "(function(sel){{ var el=document.querySelector(sel); return JSON.stringify(el ? (el.textContent||'').trim() : null); }})({})",
            js_string_literal(selector)
        );
        let json = self.run_json_js(&script)?;
        let text: Option<String> = serde_json::from_str(&json).map_err(|e| {
            EngineError::Internal(format!("read_text: malformed JSON from page script: {e}"))
        })?;
        match text {
            Some(t) => Ok((t, self.current_origin()?)),
            None => Err(EngineError::ElementNotFound(selector.to_string())),
        }
    }

    fn click(&mut self, selector: &str) -> Result<((), Origin), EngineError> {
        let script = format!(
            "(function(sel){{ var el=document.querySelector(sel); if(!el) return JSON.stringify(false); el.click(); return JSON.stringify(true); }})({})",
            js_string_literal(selector)
        );
        let json = self.run_json_js(&script)?;
        let found: bool = serde_json::from_str(&json).unwrap_or(false);
        if !found {
            return Err(EngineError::ElementNotFound(selector.to_string()));
        }
        Ok(((), self.current_origin()?))
    }

    fn type_text(&mut self, selector: &str, text: &str) -> Result<((), Origin), EngineError> {
        let script = format!(
            "(function(sel,val){{ var el=document.querySelector(sel); if(!el) return JSON.stringify(false); \
             el.value=val; el.dispatchEvent(new Event('input',{{bubbles:true}})); return JSON.stringify(true); }})({},{})",
            js_string_literal(selector),
            js_string_literal(text)
        );
        let json = self.run_json_js(&script)?;
        let found: bool = serde_json::from_str(&json).unwrap_or(false);
        if !found {
            return Err(EngineError::ElementNotFound(selector.to_string()));
        }
        Ok(((), self.current_origin()?))
    }

    fn fill_form(&mut self, fields: &[(String, String)]) -> Result<((), Origin), EngineError> {
        for (selector, value) in fields {
            self.type_text(selector, value)?;
        }
        Ok(((), self.current_origin()?))
    }

    fn select_option(&mut self, selector: &str, value: &str) -> Result<((), Origin), EngineError> {
        let script = format!(
            "(function(sel,val){{ var el=document.querySelector(sel); if(!el) return JSON.stringify(false); \
             el.value=val; el.dispatchEvent(new Event('change',{{bubbles:true}})); return JSON.stringify(true); }})({},{})",
            js_string_literal(selector),
            js_string_literal(value)
        );
        let json = self.run_json_js(&script)?;
        let found: bool = serde_json::from_str(&json).unwrap_or(false);
        if !found {
            return Err(EngineError::ElementNotFound(selector.to_string()));
        }
        Ok(((), self.current_origin()?))
    }

    fn scroll(&mut self, dx: i64, dy: i64) -> Result<((), Origin), EngineError> {
        let (cx, cy) = (self.width as f32 / 2.0, self.height as f32 / 2.0);
        self.active_tab_mut()?
            .session
            .send_scroll(cx, cy, dx as f64, dy as f64);
        self.active_tab_mut()?.session.spin();
        Ok(((), self.current_origin()?))
    }

    fn wait_for(&mut self, condition: WaitCondition) -> Result<((), Origin), EngineError> {
        match condition {
            WaitCondition::Idle => {
                self.drive_until_loaded()?;
                Ok(((), self.current_origin()?))
            }
            WaitCondition::Timeout(duration) => {
                let deadline = Instant::now() + duration;
                while Instant::now() < deadline {
                    self.active_tab_mut()?.session.spin();
                    std::thread::sleep(POLL_INTERVAL.min(duration));
                }
                Ok(((), self.current_origin()?))
            }
            WaitCondition::Selector(selector) => {
                let deadline = Instant::now() + NAV_TIMEOUT;
                loop {
                    if self
                        .query(&selector)
                        .map(|(h, _)| !h.is_empty())
                        .unwrap_or(false)
                    {
                        return Ok(((), self.current_origin()?));
                    }
                    if Instant::now() >= deadline {
                        return Err(EngineError::WaitTimedOut);
                    }
                    self.active_tab_mut()?.session.spin();
                    std::thread::sleep(POLL_INTERVAL);
                }
            }
        }
    }

    fn screenshot(&mut self) -> Result<(Frame, Origin), EngineError> {
        let tab = self.active_tab_mut()?;
        tab.session.spin();
        let frame = tab
            .session
            .get_frame()
            .ok_or_else(|| EngineError::Internal("no frame has been rendered yet".to_string()))?;
        Ok((frame, self.current_origin()?))
    }

    fn download(&mut self, _url: &str) -> Result<(String, Origin), EngineError> {
        Err(EngineError::Unsupported(
            "download: HeadlessServoSession has no download manager wired up",
        ))
    }

    fn open_tab(&mut self, url: Option<&str>) -> Result<(TabId, Origin), EngineError> {
        let session =
            HeadlessServoSession::new(self.width, self.height).map_err(EngineError::Internal)?;
        let id = TabId(self.next_tab);
        self.next_tab += 1;
        self.tabs.insert(id, Tab { session });
        self.active = id;
        if let Some(url) = url {
            self.active_tab_mut()?.session.navigate(url);
            self.drive_until_loaded()?;
        }
        Ok((id, self.current_origin()?))
    }

    fn close_tab(&mut self, tab: TabId) -> Result<((), Origin), EngineError> {
        if self.tabs.remove(&tab).is_none() {
            return Err(EngineError::NoSuchTab(tab));
        }
        if self.active == tab {
            if let Some(&next) = self.tabs.keys().next() {
                self.active = next;
            } else {
                let session = HeadlessServoSession::new(self.width, self.height)
                    .map_err(EngineError::Internal)?;
                let id = TabId(self.next_tab);
                self.next_tab += 1;
                self.tabs.insert(id, Tab { session });
                self.active = id;
            }
        }
        Ok(((), self.current_origin()?))
    }

    fn switch_tab(&mut self, tab: TabId) -> Result<((), Origin), EngineError> {
        if !self.tabs.contains_key(&tab) {
            return Err(EngineError::NoSuchTab(tab));
        }
        self.active = tab;
        Ok(((), self.current_origin()?))
    }

    fn cookies_read(&mut self, scope: &Origin) -> Result<(Vec<Cookie>, Origin), EngineError> {
        let origin = self.current_origin()?;
        if scope != &origin {
            return Err(EngineError::Unsupported(
                "cookies_read: scope must be the currently loaded page's own origin",
            ));
        }
        let json = self.run_json_js("(function(){ return JSON.stringify(document.cookie); })()")?;
        let raw: String = serde_json::from_str(&json).map_err(|e| {
            EngineError::Internal(format!(
                "cookies_read: malformed JSON from page script: {e}"
            ))
        })?;
        let cookies = raw
            .split(';')
            .filter_map(|pair| {
                let pair = pair.trim();
                if pair.is_empty() {
                    return None;
                }
                let (name, value) = pair.split_once('=')?;
                Some(Cookie {
                    name: name.trim().to_string(),
                    value: value.trim().to_string(),
                })
            })
            .collect();
        Ok((cookies, origin))
    }

    fn storage_read(
        &mut self,
        scope: &Origin,
    ) -> Result<(Vec<(String, String)>, Origin), EngineError> {
        let origin = self.current_origin()?;
        if scope != &origin {
            return Err(EngineError::Unsupported(
                "storage_read: scope must be the currently loaded page's own origin",
            ));
        }
        let json = self.run_json_js(
            "(function(){ var out=[]; for (var i=0;i<localStorage.length;i++){ \
             var k=localStorage.key(i); out.push([k, localStorage.getItem(k)]); } \
             return JSON.stringify(out); })()",
        )?;
        let kv: Vec<(String, String)> = serde_json::from_str(&json).map_err(|e| {
            EngineError::Internal(format!(
                "storage_read: malformed JSON from page script: {e}"
            ))
        })?;
        Ok((kv, origin))
    }

    fn clipboard_read(&mut self) -> Result<(String, Origin), EngineError> {
        Err(EngineError::Unsupported(
            "clipboard_read: HeadlessServoSession exposes no synchronous clipboard API",
        ))
    }

    fn clipboard_write(&mut self, _text: &str) -> Result<((), Origin), EngineError> {
        Err(EngineError::Unsupported(
            "clipboard_write: HeadlessServoSession exposes no synchronous clipboard API",
        ))
    }

    fn js_execute(&mut self, script: &str) -> Result<(String, Origin), EngineError> {
        let raw = self
            .active_tab_mut()?
            .session
            .execute_js(script)
            .map_err(EngineError::Internal)?;
        Ok((raw, self.current_origin()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_js_string_result_strips_the_observed_string_wrapper() {
        let raw = r#"String("{\"root\":{\"role\":\"generic\"}}")"#;
        let unwrapped = unwrap_js_string_result(raw).expect("must unwrap");
        assert_eq!(unwrapped, r#"{"root":{"role":"generic"}}"#);
    }

    #[test]
    fn unwrap_js_string_result_falls_back_to_parsing_the_whole_thing() {
        // If execute_js's channel ever returns an already-unwrapped JSON
        // string with no `String(...)` envelope, this must still work
        // rather than silently misparsing.
        let raw = r#""already unwrapped""#;
        let unwrapped = unwrap_js_string_result(raw).expect("must unwrap");
        assert_eq!(unwrapped, "already unwrapped");
    }

    #[test]
    fn unwrap_js_string_result_reports_a_typed_error_on_garbage() {
        let err = unwrap_js_string_result("not json at all").unwrap_err();
        assert!(matches!(err, EngineError::Internal(_)));
    }

    #[test]
    fn js_string_literal_escapes_quotes_and_backslashes() {
        assert_eq!(js_string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    // ── Engine construction without the `servo` feature ──
    //
    // With `engine-servo` off, `HeadlessServoSession::new` is the stub from
    // `ferrite-servo`, which always returns `Err`. This is exercised here
    // (not `#[ignore]`d) precisely because it needs no real Servo build —
    // it proves ServoEngine::new surfaces that stub failure as a typed
    // EngineError rather than panicking, which is the one thing about this
    // crate that *is* testable in every environment.
    #[cfg(not(feature = "engine-servo"))]
    #[test]
    fn without_the_engine_servo_feature_construction_fails_cleanly() {
        let err = ServoEngine::new(800, 600).unwrap_err();
        assert!(matches!(err, EngineError::Internal(_)));
    }
}
