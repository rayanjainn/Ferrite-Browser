// ServoShell — winit window host + Servo engine integration.
//
// ## API note (v0.0.5)
//
// The Servo v0.0.5 embedding API does NOT use a `WindowMethods` trait or
// methods like `get_gl_context()` / `get_coordinates()` — those are from an
// older embedder prototype.  The actual public API (researched against the
// upstream repo) is:
//
//   - `ServoBuilder::default()` — builder; configure with `.event_loop_waker()`,
//     `.opts()`, `.preferences()`, `.protocol_registry()`
//   - `Servo::set_delegate(ServoDelegate)` — global browser callbacks
//   - WebView created and managed by Servo internally; accessed via the
//     `WebViewDelegate` callbacks
//   - `webview.load(ServoUrl)` — navigate to a URL
//   - `webview.resize(Size2D)` — resize the render surface
//   - `webview.paint()` — trigger rendering
//   - `servo.spin_event_loop()` — run one turn of Servo's internal event loop
//
// ## Network interception API
//
// Servo v0.0.5 exposes `WebViewDelegate::load_web_resource` for intercepting
// all outgoing network requests.  It fires for every fetch (navigation,
// sub-resource, XHR, fetch()) before the request hits the network.
//
//   fn load_web_resource(&self, _webview: WebView, load: WebResourceLoad)
//
// `WebResourceLoad` carries a `WebResourceRequest` (with `.url: Url`).
// Dropping `load` without calling `.intercept()` lets the request proceed.
// Calling `load.intercept(response).cancel()` aborts the request with a
// network error — that is the block path used by the capability broker.
//
// There is no lower-level `ResourceThread` hook exposed in v0.0.5 — all
// interception must go through `load_web_resource`.
//
// All Servo-specific code is gated behind the `servo` Cargo feature:
//   cargo build -p ferrite-servo --features servo
//
// Dependency fix: the package is `libservo` (not `servo`) because the servo
// repo root is a workspace-only manifest.  The lib name inside is `servo`, so
// Rust imports still use `use servo::...`.
//
// Without the feature the winit shell compiles and opens a bare window.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[cfg_attr(not(feature = "servo"), allow(unused_imports))]
use ferrite_capability_broker::{
    BrokerDecision, CapabilityBroker, CapabilityType, Principal, PrincipalKind,
};
use ferrite_audit_log::{AuditEventKind, PersistentAuditLog};
use uuid::Uuid;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

// ---------------------------------------------------------------------------
// Servo delegate stubs (compiled only with the `servo` feature)
// ---------------------------------------------------------------------------

/// Implements `servo::WebViewDelegate` — receives per-tab callbacks from Servo.
///
/// Holds shared references to the `CapabilityBroker` and `PersistentAuditLog`
/// so that `load_web_resource` can check and record every outgoing request
/// before it reaches the network.
#[cfg(feature = "servo")]
struct FerriteWebViewDelegate {
    /// Shared broker — `RefCell` gives interior mutability so the delegate
    /// (behind `Rc<dyn WebViewDelegate>`) can call `revoke`.
    broker: Rc<RefCell<CapabilityBroker>>,
    /// Token minted at startup that authorises all network fetches.
    network_token_id: Uuid,
    /// `Uuid` of the servo-engine principal, written into audit entries.
    principal_id: Uuid,
    /// Shared audit log — written on every grant or denial.
    audit_log: Rc<RefCell<PersistentAuditLog>>,
}

#[cfg(feature = "servo")]
impl servo::WebViewDelegate for FerriteWebViewDelegate {
    /// Called whenever Servo has a new composited frame ready to display.
    fn notify_new_frame_ready(&self, webview: servo::WebView) {
        webview.paint();
    }

    /// Called when the load status of the WebView changes.
    fn notify_load_status_changed(&self, webview: servo::WebView, status: servo::LoadStatus) {
        if status == servo::LoadStatus::Complete {
            let url = webview
                .url()
                .map(|u| u.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            println!("[ferrite] page load complete: {}", url);
        }
    }

    /// Called for every outgoing network request — navigation, sub-resource,
    /// XHR, fetch().  This is the Servo v0.0.5 fetch interception point.
    ///
    /// Behaviour:
    ///   - Ask the broker whether `network_token_id` covers this URL.
    ///   - `Granted`  → append `CapabilityGranted` to audit log; drop `load`
    ///                  so Servo proceeds normally.
    ///   - `Denied`   → log the block; append `CapabilityDenied` to audit log;
    ///                  intercept + cancel so no bytes leave the process.
    fn load_web_resource(&self, _webview: servo::WebView, load: servo::WebResourceLoad) {
        let url = load.request().url.to_string();
        let decision = self.broker.borrow().check(self.network_token_id, &url);

        match decision {
            BrokerDecision::Granted { .. } => {
                if let Err(e) = self.audit_log.borrow_mut().append(
                    AuditEventKind::CapabilityGranted,
                    self.principal_id,
                    Some("network.fetch".to_string()),
                    Some(url),
                ) {
                    eprintln!("[ferrite] audit write error (granted): {}", e);
                }
                // Drop `load` without intercepting — Servo fetches normally.
            }
            BrokerDecision::Denied { reason } => {
                println!("[ferrite] BLOCKED: {} reason: {:?}", url, reason);
                if let Err(e) = self.audit_log.borrow_mut().append(
                    AuditEventKind::CapabilityDenied,
                    self.principal_id,
                    Some("network.fetch".to_string()),
                    Some(url.clone()),
                ) {
                    eprintln!("[ferrite] audit write error (denied): {}", e);
                }
                // Intercept with a stub response, then cancel.  This aborts
                // the request with a network error before any bytes are sent.
                let stub = servo::WebResourceResponse::new(url::Url::parse(&url).unwrap());
                load.intercept(stub).cancel();
            }
        }
    }

    // All other delegate methods use the default no-op implementations
    // provided by the trait.  Add overrides here as Ferrite gains features.
}

/// Implements `servo::ServoDelegate` — receives global browser-level callbacks.
#[cfg(feature = "servo")]
struct FerriteServoDelegate;

#[cfg(feature = "servo")]
impl servo::ServoDelegate for FerriteServoDelegate {
    // All methods default to no-ops.  Wire in audit logging here once
    // ferrite-audit is integrated (Month 2).
}

// ---------------------------------------------------------------------------
// Winit ApplicationHandler
// ---------------------------------------------------------------------------

struct AppHandler {
    window: Option<Arc<Window>>,

    /// Shared broker — also held by `FerriteWebViewDelegate`.
    #[cfg_attr(not(feature = "servo"), allow(dead_code))]
    broker: Rc<RefCell<CapabilityBroker>>,

    /// Token authorising all network fetches for the Servo engine principal.
    #[cfg_attr(not(feature = "servo"), allow(dead_code))]
    network_token_id: Uuid,

    /// `Uuid` of the servo-engine principal passed into audit entries.
    #[cfg_attr(not(feature = "servo"), allow(dead_code))]
    principal_id: Uuid,

    /// Shared audit log — also held by `FerriteWebViewDelegate`.
    audit_log: Rc<RefCell<PersistentAuditLog>>,

    /// Servo browser engine instance.  `None` until `resumed()` fires.
    #[cfg(feature = "servo")]
    servo: Option<servo::Servo>,

    /// Handle to the single top-level WebView.
    #[cfg(feature = "servo")]
    webview: Option<servo::WebView>,
}

impl AppHandler {
    /// Print the audit chain verification result and grant/denial summary.
    /// Called on both Escape and CloseRequested so either exit path logs it.
    fn print_exit_summary(&self) {
        let log = self.audit_log.borrow();
        let entries = &log.log.entries;

        if log.log.verify_chain() {
            println!(
                "[ferrite] audit chain verified: {} entries",
                entries.len()
            );
        } else {
            println!("[ferrite] AUDIT CHAIN BROKEN — investigate immediately");
        }

        let grants = entries
            .iter()
            .filter(|e| e.kind == AuditEventKind::CapabilityGranted)
            .count();
        let denials = entries
            .iter()
            .filter(|e| e.kind == AuditEventKind::CapabilityDenied)
            .count();
        println!(
            "[ferrite] session summary: {} grants, {} denials",
            grants, denials
        );
    }
}

impl ApplicationHandler for AppHandler {
    /// Called once the event loop is ready to accept windows.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Ferrite Browser")
            .with_inner_size(LogicalSize::new(1280.0_f64, 800.0_f64));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );
        self.window = Some(window.clone());

        // ── Servo initialisation ────────────────────────────────────────────
        #[cfg(feature = "servo")]
        {
            use servo::{ServoBuilder, WebViewBuilder};

            // Build the Servo engine.
            let servo = ServoBuilder::default()
                // .event_loop_waker(waker)   // TODO: wire winit EventLoopProxy
                // .opts(opts)                // TODO: forward CLI opts
                // .preferences(prefs)       // TODO: load user prefs
                .build();

            // Register global delegate (logging, devtools, etc.)
            servo.set_delegate(Rc::new(FerriteServoDelegate));

            // Create the initial WebView and load https://example.com.
            //
            // WebViewBuilder::new() requires a WindowRenderingContext (GL/surfman
            // surface).  Construction sequence once wired in Block 4:
            //
            //   use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
            //   let display = window.display_handle().unwrap();
            //   let whandle = window.window_handle().unwrap();
            //   let size    = window.inner_size();
            //   let rc = Rc::new(
            //       servo::WindowRenderingContext::new(display, whandle, size)
            //           .expect("rendering context"),
            //   );
            //   let webview = WebViewBuilder::new(&servo, rc)
            //       .delegate(Rc::new(FerriteWebViewDelegate {
            //           broker: self.broker.clone(),
            //           network_token_id: self.network_token_id,
            //           principal_id: self.principal_id,
            //           audit_log: self.audit_log.clone(),
            //       }))
            //       .url(url::Url::parse("https://example.com").unwrap())
            //       .build();
            //   webview.resize(window.inner_size());
            //
            // For now leave webview as None until the rendering context is
            // wired in (Block 4 — surfman/GL setup).

            // Spin once so Servo starts up its internal machinery.
            servo.spin_event_loop();

            self.servo = Some(servo);
            // self.webview remains None until Block 4 (RenderingContext setup)
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.print_exit_summary();
                event_loop.exit();
            }

            WindowEvent::KeyboardInput { event: key_event, .. }
                if key_event.state == ElementState::Pressed
                    && key_event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                self.print_exit_summary();
                event_loop.exit();
            }

            WindowEvent::RedrawRequested => {
                // ── Drive Servo and composite ───────────────────────────────
                #[cfg(feature = "servo")]
                if let (Some(servo), Some(webview)) =
                    (&mut self.servo, &mut self.webview)
                {
                    servo.spin_event_loop();
                    webview.paint();
                }

                // Request the next frame so the loop keeps running.
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            // All other events are pass-through for now.
            _ => {}
        }
    }

    /// Called once all pending window events have been dispatched — equivalent
    /// to the old `MainEventsCleared`.  Drive Servo's internal event loop here
    /// so it makes progress regardless of whether a redraw was requested.
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(feature = "servo")]
        if let Some(servo) = &mut self.servo {
            servo.spin_event_loop();
        }
    }
}

// ---------------------------------------------------------------------------
// ServoShell public API
// ---------------------------------------------------------------------------

/// Owns the winit `EventLoop`, the `CapabilityBroker`, the `PersistentAuditLog`,
/// and drives the browser window.
///
/// The broker and audit log are shared (via `Rc<RefCell<>>`) with the
/// `FerriteWebViewDelegate` so that every outgoing network request is checked
/// and recorded before it reaches the network.
pub struct ServoShell {
    event_loop: EventLoop<()>,
    broker: Rc<RefCell<CapabilityBroker>>,
    network_token_id: Uuid,
    principal_id: Uuid,
    audit_log: Rc<RefCell<PersistentAuditLog>>,
}

impl ServoShell {
    /// Creates the winit `EventLoop`, mints the default-allow network token,
    /// and opens the audit log at `$TMPDIR/ferrite_servo.db`.
    ///
    /// A `NetworkFetch` token scoped to `"*"` (wildcard origin) with a 3600-second
    /// TTL is minted for the Servo engine principal.  This token acts as the
    /// "default allow" grant that permits Servo to load pages normally.
    /// Revoking it (via `broker().borrow_mut().revoke(token_id)`) blocks all
    /// subsequent fetches through the broker check in `load_web_resource`.
    pub fn new() -> Self {
        let mut broker = CapabilityBroker::new();

        let principal_id = Uuid::new_v4();
        let servo_principal = Principal {
            id: principal_id,
            kind: PrincipalKind::WebContent,
            label: "servo-engine".to_string(),
        };

        // Wildcard origin — covers all URLs Servo may fetch.
        let network_token_id = broker.mint_token(
            servo_principal,
            CapabilityType::NetworkFetch,
            "*".to_string(), // origin_scope: wildcard
            vec![],          // url_allowlist: empty = no additional filtering
            None,            // rate_limit: uncapped for now
            3600,            // TTL: 1 hour
        );

        let db_path = std::env::temp_dir()
            .join("ferrite_servo.db")
            .to_string_lossy()
            .into_owned();
        let audit_log = PersistentAuditLog::new(&db_path)
            .expect("failed to open audit log");

        let event_loop = EventLoop::new().expect("failed to create event loop");
        ServoShell {
            event_loop,
            broker: Rc::new(RefCell::new(broker)),
            network_token_id,
            principal_id,
            audit_log: Rc::new(RefCell::new(audit_log)),
        }
    }

    /// Returns the token ID for the default network-allow token.
    ///
    /// Callers can pass this to `broker().borrow_mut().revoke(id)` to stop all
    /// Servo network fetches — useful for testing the block path.
    pub fn network_token_id(&self) -> Uuid {
        self.network_token_id
    }

    /// Returns a shared reference to the `CapabilityBroker`.
    pub fn broker(&self) -> Rc<RefCell<CapabilityBroker>> {
        self.broker.clone()
    }

    /// Starts the event loop.  Blocks until the window is closed.
    pub fn run(self) {
        let mut app = AppHandler {
            window: None,
            broker: self.broker,
            network_token_id: self.network_token_id,
            principal_id: self.principal_id,
            audit_log: self.audit_log,
            #[cfg(feature = "servo")]
            servo: None,
            #[cfg(feature = "servo")]
            webview: None,
        };
        self.event_loop
            .run_app(&mut app)
            .expect("event loop terminated with error");
    }
}

impl Default for ServoShell {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the default-allow token lets requests through, and that
    /// revoking it blocks subsequent checks.
    ///
    /// This is the unit-test equivalent of the integration scenario:
    ///   1. Page loads via the wildcard token (Granted).
    ///   2. Token is revoked.
    ///   3. Navigate to a new URL — broker check returns Denied.
    ///
    /// The end-to-end version (revoke mid-session then observe the block log
    /// line in the window) requires the RenderingContext to be wired (Block 4)
    /// before it can run against a live Servo instance.
    #[test]
    fn revoke_blocks_subsequent_requests() {
        let mut broker = CapabilityBroker::new();

        let principal = Principal {
            id: Uuid::new_v4(),
            kind: PrincipalKind::WebContent,
            label: "servo-engine".to_string(),
        };

        let token_id = broker.mint_token(
            principal,
            CapabilityType::NetworkFetch,
            "*".to_string(),
            vec![],
            None,
            3600,
        );

        // Before revoke — any URL should be Granted.
        assert!(
            matches!(
                broker.check(token_id, "https://example.com/test"),
                BrokerDecision::Granted { .. }
            ),
            "expected Granted before revoke"
        );

        // Revoke the token.
        broker.revoke(token_id);

        // After revoke — the same URL should be Denied (UnknownPrincipal,
        // because the token entry no longer exists in the map).
        assert!(
            matches!(
                broker.check(token_id, "https://example.com/test"),
                BrokerDecision::Denied { .. }
            ),
            "expected Denied after revoke"
        );
    }

    /// Verifies that audit entries written on grant/denial produce a valid
    /// hash chain, and that grants vs denials are correctly counted.
    #[test]
    fn audit_log_records_grants_and_denials() {
        let db_path = std::env::temp_dir()
            .join(format!("ferrite_test_{}.db", Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();

        let mut audit_log = PersistentAuditLog::new(&db_path)
            .expect("failed to open test audit log");

        let principal_id = Uuid::new_v4();

        audit_log
            .append(
                AuditEventKind::CapabilityGranted,
                principal_id,
                Some("network.fetch".to_string()),
                Some("https://example.com".to_string()),
            )
            .unwrap();

        audit_log
            .append(
                AuditEventKind::CapabilityDenied,
                principal_id,
                Some("network.fetch".to_string()),
                Some("https://blocked.example".to_string()),
            )
            .unwrap();

        audit_log
            .append(
                AuditEventKind::CapabilityGranted,
                principal_id,
                Some("network.fetch".to_string()),
                Some("https://example.com/page2".to_string()),
            )
            .unwrap();

        assert!(audit_log.log.verify_chain(), "chain should be valid");

        let grants = audit_log
            .log
            .entries
            .iter()
            .filter(|e| e.kind == AuditEventKind::CapabilityGranted)
            .count();
        let denials = audit_log
            .log
            .entries
            .iter()
            .filter(|e| e.kind == AuditEventKind::CapabilityDenied)
            .count();

        assert_eq!(grants, 2, "expected 2 grants");
        assert_eq!(denials, 1, "expected 1 denial");

        // Clean up.
        let _ = std::fs::remove_file(&db_path);
    }
}
