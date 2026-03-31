// session.rs — Headless Servo session for embedding in Iced.
//
// `HeadlessServoSession` wraps a `SoftwareRenderingContext` + a single Servo
// `WebView`.  It does NOT own a winit event loop — the caller (Iced) drives it
// by calling `spin()` on every tick and reads rendered frames via `get_frame()`.
//
// This module is only compiled when the `servo` Cargo feature is enabled:
//   cargo build -p ferrite-servo --features servo
//
// ## Usage in Iced
//
//   let mut session = HeadlessServoSession::new(1280, 800)?;
//   session.navigate("https://example.com");
//
//   // On each Iced subscription tick:
//   session.spin();
//   if let Some((w, h, bytes)) = session.get_frame() {
//       let handle = image::Handle::from_rgba(w, h, bytes);
//       // display with iced::widget::image(handle)
//   }

/// Result of a JS compatibility probe run by [`HeadlessServoSession::test_js_compat`].
#[derive(Debug, Clone)]
pub struct JSCompatResult {
    /// The URL that was probed.
    pub url: String,
    /// `true` if any JavaScript ran — inferred from the page title being set by JS.
    pub js_executed: bool,
    /// JavaScript console errors collected during the load (requires the
    /// `servo` feature; always empty in stub builds).
    pub console_errors: Vec<String>,
    /// Final page title as set by JS (or `None` if the page never set one).
    pub page_title: Option<String>,
}

/// Load state of the active WebView.
///
/// Produced by [`HeadlessServoSession::load_status()`] and synced from the
/// `WebViewDelegate` callbacks on every [`HeadlessServoSession::spin()`] call.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadStatus {
    /// A navigation is in progress.
    Loading,
    /// The page has finished loading successfully.
    Complete,
    /// The page failed to load. Contains an error description.
    Failed(String),
}

#[cfg(feature = "servo")]
pub use inner::HeadlessServoSession;

#[cfg(feature = "servo")]
mod inner {
    use std::cell::RefCell;
    use std::rc::Rc;

    use rustls::crypto::aws_lc_rs;
    use servo::{
        DevicePoint, DeviceVector2D, InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent,
        MouseMoveEvent, RenderingContext, Scroll, Servo, ServoBuilder, ServoDelegate,
        SoftwareRenderingContext, WebViewBuilder, WebViewDelegate, WebViewPoint, WheelDelta,
        WheelEvent, WheelMode,
    };
    use winit::dpi::PhysicalSize;

    // Servo's `opts` module uses a global singleton that panics if initialised
    // more than once per process.  We therefore create the `Servo` engine once
    // and share it (via `Clone`, which is a cheap `Rc` bump) across all tabs.
    thread_local! {
        static SERVO_ENGINE: RefCell<Option<Servo>> = const { RefCell::new(None) };
    }

    /// Return (or lazily create) the process-wide `Servo` engine.
    ///
    /// The first call builds the engine; every subsequent call clones the `Rc`
    /// wrapper, so `opts::initialize_options` is only ever called once.
    fn get_or_init_servo() -> Servo {
        SERVO_ENGINE.with(|cell| {
            let mut guard = cell.borrow_mut();
            if guard.is_none() {
                *guard = Some(ServoBuilder::default().build());
            }
            guard.as_ref().unwrap().clone()
        })
    }

    use ferrite_audit_log::{AuditEventKind, PersistentAuditLog};
    use ferrite_capability_broker::{
        BrokerDecision, CapabilityBroker, CapabilityType, Principal, PrincipalKind,
    };
    use uuid::Uuid;

    use super::LoadStatus;

    // -------------------------------------------------------------------------
    // Servo delegate (global browser-level callbacks — all no-ops)
    // -------------------------------------------------------------------------

    struct HeadlessServoDelegate;
    impl ServoDelegate for HeadlessServoDelegate {}

    // -------------------------------------------------------------------------
    // WebView delegate
    // -------------------------------------------------------------------------

    struct HeadlessDelegate {
        broker: Rc<std::cell::RefCell<CapabilityBroker>>,
        network_token_id: Uuid,
        principal_id: Uuid,
        audit_log: Rc<std::cell::RefCell<PersistentAuditLog>>,
        /// Shared load status — written by delegate callbacks, read by session in `spin()`.
        load_status: Rc<std::cell::RefCell<LoadStatus>>,
        /// Shared current URL — written by delegate callbacks, read by session in `spin()`.
        current_url: Rc<std::cell::RefCell<String>>,
        /// Navigation count — incremented on each `Complete`; used for `can_go_back()`.
        nav_count: Rc<std::cell::RefCell<u32>>,
        /// Shared page title — written by `notify_page_title_changed`, read in `spin()`.
        page_title: Rc<std::cell::RefCell<Option<String>>>,
        /// Accumulated JS console errors — appended by `notify_console_message`, drained by
        /// `HeadlessServoSession::take_console_errors()`.
        console_errors: Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl WebViewDelegate for HeadlessDelegate {
        fn notify_new_frame_ready(&self, webview: servo::WebView) {
            webview.paint();
        }

        fn notify_load_status_changed(&self, webview: servo::WebView, status: servo::LoadStatus) {
            match status {
                servo::LoadStatus::Complete => {
                    let url = webview
                        .url()
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    *self.current_url.borrow_mut() = url;
                    *self.load_status.borrow_mut() = LoadStatus::Complete;
                    *self.nav_count.borrow_mut() += 1;
                }
                _ => {
                    // Treat all non-Complete statuses (Loading, Failed, etc.) as Loading.
                    *self.load_status.borrow_mut() = LoadStatus::Loading;
                }
            }
        }

        fn notify_page_title_changed(&self, _webview: servo::WebView, title: Option<String>) {
            *self.page_title.borrow_mut() = title;
        }

        fn show_console_message(
            &self,
            _webview: servo::WebView,
            level: servo::ConsoleLogLevel,
            message: String,
        ) {
            // Only capture error-level messages to keep the list focused on
            // actionable JS failures.
            if matches!(level, servo::ConsoleLogLevel::Error) {
                self.console_errors.borrow_mut().push(message);
            }
        }

        fn load_web_resource(&self, _webview: servo::WebView, load: servo::WebResourceLoad) {
            let url = load.request().url.to_string();
            let decision = self.broker.borrow().check(self.network_token_id, &url);

            match decision {
                BrokerDecision::Granted { .. } => {
                    let _ = self.audit_log.borrow_mut().append(
                        AuditEventKind::CapabilityGranted,
                        self.principal_id,
                        Some("network.fetch".to_string()),
                        Some(url),
                    );
                }
                BrokerDecision::Denied { reason } => {
                    println!("[ferrite-session] BLOCKED: {} reason: {:?}", url, reason);
                    let _ = self.audit_log.borrow_mut().append(
                        AuditEventKind::CapabilityDenied,
                        self.principal_id,
                        Some("network.fetch".to_string()),
                        Some(url.clone()),
                    );
                    let stub = servo::WebResourceResponse::new(url::Url::parse(&url).unwrap());
                    load.intercept(stub).cancel();
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // HeadlessServoSession
    // -------------------------------------------------------------------------

    /// A self-contained, headless Servo browsing session.
    ///
    /// Rendering uses `SoftwareRenderingContext` (CPU rasteriser, no GPU or
    /// window handle required).  Callers drive it by calling `spin()` each tick
    /// and read rendered frames via `get_frame()`.
    ///
    /// Load status and current URL are synced from `WebViewDelegate` callbacks
    /// on each `spin()` call and exposed via `load_status()` and `current_url()`.
    pub struct HeadlessServoSession {
        servo: servo::Servo,
        webview: servo::WebView,
        rendering_context: Rc<SoftwareRenderingContext>,
        width: u32,
        height: u32,
        /// Cached last frame as raw RGBA bytes (width × height × 4).
        last_frame: Option<Vec<u8>>,
        /// Most recently synced load status (updated in `spin()`).
        last_load_status: LoadStatus,
        /// Most recently synced current URL (updated in `spin()`).
        current_url: String,
        /// Shared load status cell — written by `HeadlessDelegate`, read in `spin()`.
        shared_load_status: Rc<std::cell::RefCell<LoadStatus>>,
        /// Shared URL cell — written by `HeadlessDelegate`, read in `spin()`.
        shared_url: Rc<std::cell::RefCell<String>>,
        /// Navigation count shared with `HeadlessDelegate`.
        shared_nav_count: Rc<std::cell::RefCell<u32>>,
        /// Shared page title cell — written by `HeadlessDelegate`, read in `spin()`.
        shared_page_title: Rc<std::cell::RefCell<Option<String>>>,
        /// Most recently synced page title (updated in `spin()`).
        last_page_title: Option<String>,
        /// JS console errors shared with `HeadlessDelegate` — accumulated until drained.
        shared_console_errors: Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl HeadlessServoSession {
        /// Create a new headless session with a `width × height` render surface.
        ///
        /// Opens `$TMPDIR/ferrite_servo_session.db` for the audit log and mints
        /// a wildcard `NetworkFetch` token (3600 s TTL).
        ///
        /// On Windows, Servo's EGL/surfman backend requires ANGLE (`libEGL.dll`,
        /// `libGLESv2.dll`) in the executable's directory or on PATH.  If those
        /// DLLs are absent the EGL bindings panic at function-pointer load time.
        /// This constructor wraps the entire initialisation in `catch_unwind` so
        /// the panic is converted to an `Err` rather than crashing the process —
        /// the caller can then fall back to a no-Servo UI gracefully.
        pub fn new(width: u32, height: u32) -> Result<Self, String> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Self::new_inner(width, height)
            }))
            .unwrap_or_else(|payload| {
                let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                    format!("Servo init panic: {}", s)
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    format!("Servo init panic: {}", s)
                } else {
                    "Servo init panic: EGL not available (ANGLE DLLs missing on Windows?)".to_string()
                };
                Err(msg)
            })
        }

        fn new_inner(width: u32, height: u32) -> Result<Self, String> {
            // ── rustls crypto provider ─────────────────────────────────────
            let _ = aws_lc_rs::default_provider().install_default();

            // ── Audit log ──────────────────────────────────────────────────
            let db_path = std::env::temp_dir()
                .join("ferrite_servo_session.db")
                .to_string_lossy()
                .into_owned();
            let _ = std::fs::remove_file(&db_path);
            let audit_log = PersistentAuditLog::new(&db_path).map_err(|e| e.to_string())?;

            // ── Broker + token ─────────────────────────────────────────────
            let mut broker = CapabilityBroker::new();
            let principal_id = Uuid::new_v4();
            let network_token_id = broker.mint_token(
                Principal {
                    id: principal_id,
                    kind: PrincipalKind::WebContent,
                    label: "servo-session".to_string(),
                },
                CapabilityType::NetworkFetch,
                "*".to_string(),
                vec![],
                None,
                3600,
            );

            let broker = Rc::new(std::cell::RefCell::new(broker));
            let audit_log = Rc::new(std::cell::RefCell::new(audit_log));

            // ── Shared delegate ↔ session state ────────────────────────────
            let shared_load_status = Rc::new(std::cell::RefCell::new(LoadStatus::Loading));
            let shared_url = Rc::new(std::cell::RefCell::new("about:blank".to_string()));
            let shared_nav_count = Rc::new(std::cell::RefCell::new(0u32));
            let shared_page_title: Rc<std::cell::RefCell<Option<String>>> =
                Rc::new(std::cell::RefCell::new(None));
            let shared_console_errors: Rc<std::cell::RefCell<Vec<String>>> =
                Rc::new(std::cell::RefCell::new(Vec::new()));

            // ── Rendering context ──────────────────────────────────────────
            let rendering_context = Rc::new(
                SoftwareRenderingContext::new(PhysicalSize { width, height })
                    .map_err(|e| format!("SoftwareRenderingContext: {:?}", e))?,
            );
            rendering_context
                .make_current()
                .map_err(|e| format!("make_current: {:?}", e))?;

            // ── Servo engine ───────────────────────────────────────────────
            // Reuse the process-wide Servo instance (opts can only be
            // initialised once; subsequent tabs clone the Rc handle).
            let servo = get_or_init_servo();
            servo.set_delegate(Rc::new(HeadlessServoDelegate));

            // ── WebView ───────────────────────────────────────────────────
            let delegate = Rc::new(HeadlessDelegate {
                broker,
                network_token_id,
                principal_id,
                audit_log,
                load_status: shared_load_status.clone(),
                current_url: shared_url.clone(),
                nav_count: shared_nav_count.clone(),
                page_title: shared_page_title.clone(),
                console_errors: shared_console_errors.clone(),
            });
            let webview = WebViewBuilder::new(&servo, rendering_context.clone())
                .delegate(delegate)
                .url(url::Url::parse("about:blank").unwrap())
                .build();

            webview.resize(PhysicalSize { width, height });
            servo.spin_event_loop();

            Ok(Self {
                servo,
                webview,
                rendering_context,
                width,
                height,
                last_frame: None,
                last_load_status: LoadStatus::Loading,
                current_url: "about:blank".to_string(),
                shared_load_status,
                shared_url,
                shared_nav_count,
                shared_page_title,
                last_page_title: None,
                shared_console_errors,
            })
        }

        /// Navigate the active WebView to `url`.  No-op if `url` is not a
        /// valid absolute URL (logs a warning instead of panicking).
        pub fn navigate(&self, url: &str) {
            match url::Url::parse(url) {
                Ok(parsed) => self.webview.load(parsed),
                Err(e) => eprintln!("[ferrite-session] invalid URL '{}': {}", url, e),
            }
        }

        /// Go back one step in the navigation history.
        ///
        /// Requires servo v0.0.5 `WebView::go_back()`.  If the method does not
        /// exist in your build, replace this call with an appropriate alternative.
        pub fn go_back(&self) {
            self.webview.go_back(1);
        }

        /// Go forward one step in the navigation history.
        ///
        /// Requires servo v0.0.5 `WebView::go_forward()`.
        pub fn go_forward(&self) {
            self.webview.go_forward(1);
        }

        /// Reload the current page.
        ///
        /// Falls back to re-navigating to `current_url()` if `WebView::reload()`
        /// is not available in this servo build.
        pub fn reload(&self) {
            self.webview.reload();
        }

        /// Stop the current page load.
        ///
        /// `WebView::stop()` is not available in this servo build; this is a no-op for now.
        pub fn stop(&self) {
            // TODO: servo WebView does not expose a stop() method yet.
            log::warn!("stop() called but servo WebView::stop() is not available");
        }

        /// Returns the current load status of the active WebView.
        ///
        /// Reflects the most recent state synced in the last `spin()` call.
        pub fn load_status(&self) -> &LoadStatus {
            &self.last_load_status
        }

        /// Returns the current URL of the active WebView.
        ///
        /// Reflects the most recent URL synced when load completed in `spin()`.
        pub fn current_url(&self) -> &str {
            &self.current_url
        }

        /// Returns `true` if there is at least one page to go back to.
        pub fn can_go_back(&self) -> bool {
            *self.shared_nav_count.borrow() > 1
        }

        /// Returns `true` if there are forward pages in the navigation history.
        pub fn can_go_forward(&self) -> bool {
            // Forward history is not yet tracked.  Future: count go_back() calls.
            false
        }

        /// Pump the shared Servo engine for one turn.
        ///
        /// When multiple tabs are open this must be called **exactly once per
        /// tick** (on any one session) before calling `sync_and_read()` on
        /// every session.  Calling it more than once per tick risks double-
        /// processing paint messages and can cause a segfault inside Servo.
        pub fn pump_engine(&self) {
            self.servo.spin_event_loop();
        }

        /// Sync per-tab state from delegate callbacks and read back the latest
        /// composited frame into `last_frame`.
        ///
        /// Call this on **every** session after one `pump_engine()` call.
        pub fn sync_and_read(&mut self) {
            // Sync load status, URL, and page title from delegate callbacks.
            self.last_load_status = self.shared_load_status.borrow().clone();
            self.current_url = self.shared_url.borrow().clone();
            self.last_page_title = self.shared_page_title.borrow().clone();

            // Read back the current frame after paint.
            //
            // `pump_engine()` may have called `make_current()` on another
            // tab's rendering context (GL context is a per-thread global).
            // Re-establish this tab's context as current before the readback so
            // `glReadPixels` reads the correct surface.
            let _ = self.rendering_context.make_current();
            let rect = servo::DeviceIntRect::from_origin_and_size(
                servo::DeviceIntPoint::origin(),
                servo::DeviceIntSize::new(self.width as i32, self.height as i32),
            );
            if let Some(rgba) = self.rendering_context.read_to_image(rect) {
                self.last_frame = Some(rgba.into_raw());
            }
        }

        /// Convenience wrapper for the single-tab case: pump + sync + read in one call.
        pub fn spin(&mut self) {
            self.pump_engine();
            self.sync_and_read();
        }

        /// Returns `(width, height, rgba_bytes)` of the most recently rendered
        /// frame, or `None` if no frame has been produced yet.
        pub fn get_frame(&self) -> Option<(u32, u32, Vec<u8>)> {
            self.last_frame
                .as_ref()
                .map(|b| (self.width, self.height, b.clone()))
        }

        /// Returns the most recently received page title, or `None` if the page has
        /// not set a title yet.
        pub fn page_title(&self) -> Option<&str> {
            self.last_page_title.as_deref()
        }

        /// Execute `script` in the active WebView and return the result as a `String`.
        ///
        /// Uses `WebView::evaluate_javascript` (servo v0.0.5).  The result is
        /// serialised to JSON by Servo and returned as-is.  Returns `Err` if the
        /// WebView call itself fails (e.g. engine not initialised).
        pub fn execute_js(&mut self, script: &str) -> Result<String, String> {
            // Servo v0.0.5 exposes evaluate_javascript on WebView.
            // The callback receives Option<String> (Some = success, None = exception).
            let result_cell: Rc<std::cell::RefCell<Option<Result<String, String>>>> =
                Rc::new(std::cell::RefCell::new(None));
            let cell_clone = result_cell.clone();

            self.webview.evaluate_javascript(script, move |res| {
                *cell_clone.borrow_mut() = Some(match res {
                    Ok(v) => Ok(format!("{:?}", v)),
                    Err(e) => Err(format!("{:?}", e)),
                });
            });

            // Drive the event loop until the callback fires (max 2 s).
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            loop {
                self.pump_engine();
                if result_cell.borrow().is_some() {
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    return Err("JS evaluation timed out".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(16));
            }

            let result = result_cell
                .borrow_mut()
                .take()
                .unwrap_or_else(|| Err("JS evaluation result missing".to_string()));

            match &result {
                Ok(v) => println!(
                    "[ferrite-js] execute: {} chars, result: {}",
                    script.len(),
                    &v[..50.min(v.len())]
                ),
                Err(e) => println!(
                    "[ferrite-js] execute: {} chars, error: {}",
                    script.len(),
                    e
                ),
            }

            result
        }

        /// Drains and returns all JS console errors collected since the last call.
        pub fn take_console_errors(&mut self) -> Vec<String> {
            std::mem::take(&mut *self.shared_console_errors.borrow_mut())
        }

        /// Navigate to `url`, drive the event loop for up to `timeout_secs`, and
        /// return a [`JSCompatResult`] summarising what happened.
        ///
        /// JS execution is inferred from whether the page title changed from
        /// `None` — most non-trivial pages set their `<title>` via JS.
        pub fn test_js_compat(&mut self, url: &str) -> super::JSCompatResult {
            // Clear any state left over from a previous probe.
            *self.shared_page_title.borrow_mut() = None;
            self.last_page_title = None;
            let _ = self.take_console_errors();

            self.navigate(url);

            let timeout = std::time::Duration::from_secs(5);
            let started = std::time::Instant::now();

            loop {
                self.spin();

                if matches!(self.last_load_status, LoadStatus::Complete | LoadStatus::Failed(_)) {
                    break;
                }
                if started.elapsed() >= timeout {
                    break;
                }

                // Yield the thread briefly so we don't busy-spin at 100% CPU.
                std::thread::sleep(std::time::Duration::from_millis(16));
            }

            let title = self.last_page_title.clone();
            let js_executed = title.is_some();
            let console_errors = self.take_console_errors();

            super::JSCompatResult {
                url: url.to_string(),
                js_executed,
                console_errors,
                page_title: title,
            }
        }

        /// Resize the render surface and notify the WebView.
        pub fn resize(&mut self, width: u32, height: u32) {
            self.width = width;
            self.height = height;
            let size = PhysicalSize { width, height };
            self.rendering_context.resize(size);
            self.webview.resize(size);
        }

        /// Send a mouse-move event to the WebView at pixel coordinates `(x, y)`.
        pub fn send_mouse_move(&self, x: f32, y: f32) {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            self.webview
                .notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point)));
        }

        /// Send a mouse-button down+up (click) at pixel coordinates `(x, y)`.
        pub fn send_mouse_click(&self, x: f32, y: f32) {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            self.webview.notify_input_event(InputEvent::MouseButton(
                MouseButtonEvent::new(MouseButtonAction::Down, MouseButton::Left, point),
            ));
            self.webview.notify_input_event(InputEvent::MouseButton(
                MouseButtonEvent::new(MouseButtonAction::Up, MouseButton::Left, point),
            ));
        }

        /// Send a right mouse-button click at pixel coordinates `(x, y)`.
        pub fn send_right_click(&self, x: f32, y: f32) {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            self.webview.notify_input_event(InputEvent::MouseButton(
                MouseButtonEvent::new(MouseButtonAction::Down, MouseButton::Right, point),
            ));
            self.webview.notify_input_event(InputEvent::MouseButton(
                MouseButtonEvent::new(MouseButtonAction::Up, MouseButton::Right, point),
            ));
        }

        /// Send a scroll (wheel) event at pixel coordinates `(x, y)`.
        ///
        /// `delta_x` and `delta_y` are in CSS pixels; positive `delta_y` scrolls down.
        pub fn send_scroll(&self, x: f32, y: f32, delta_x: f64, delta_y: f64) {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            self.webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                WheelDelta {
                    x: delta_x,
                    y: delta_y,
                    z: 0.0,
                    mode: WheelMode::DeltaPixel,
                },
                point,
            )));
            // Also drive the scroll via the legacy Scroll API so Servo's
            // compositor can recomposite the page without waiting for a paint.
            let scroll_vec = DeviceVector2D::new(-delta_x as f32, -delta_y as f32);
            self.webview
                .notify_scroll_event(Scroll::Delta(scroll_vec.into()), point);
        }

        /// Send a mouse-down event (without the subsequent up) — for drag start.
        pub fn send_mouse_down(&self, x: f32, y: f32) {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            self.webview.notify_input_event(InputEvent::MouseButton(
                MouseButtonEvent::new(MouseButtonAction::Down, MouseButton::Left, point),
            ));
        }

        /// Send a mouse-up event — for drag end.
        pub fn send_mouse_up(&self, x: f32, y: f32) {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            self.webview.notify_input_event(InputEvent::MouseButton(
                MouseButtonEvent::new(MouseButtonAction::Up, MouseButton::Left, point),
            ));
        }
    }
}

// Stub for non-servo builds so the type name is always resolvable.
#[cfg(not(feature = "servo"))]
pub struct HeadlessServoSession;

#[cfg(not(feature = "servo"))]
impl HeadlessServoSession {
    pub fn new(_width: u32, _height: u32) -> Result<Self, String> {
        Err("ferrite-servo compiled without the `servo` feature".to_string())
    }

    pub fn navigate(&self, _url: &str) {}

    pub fn go_back(&self) {}

    pub fn go_forward(&self) {}

    pub fn reload(&self) {}

    pub fn stop(&self) {}

    pub fn pump_engine(&self) {}

    pub fn sync_and_read(&mut self) {}

    pub fn spin(&mut self) {}

    pub fn get_frame(&self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }

    pub fn resize(&mut self, _width: u32, _height: u32) {}

    pub fn load_status(&self) -> &LoadStatus {
        const STATUS: LoadStatus = LoadStatus::Loading;
        &STATUS
    }

    pub fn current_url(&self) -> &str {
        "about:blank"
    }

    pub fn can_go_back(&self) -> bool {
        false
    }

    pub fn can_go_forward(&self) -> bool {
        false
    }

    pub fn page_title(&self) -> Option<&str> {
        None
    }

    pub fn execute_js(&mut self, _script: &str) -> Result<String, String> {
        Err("ferrite-servo compiled without the `servo` feature".to_string())
    }

    pub fn take_console_errors(&mut self) -> Vec<String> {
        vec![]
    }

    pub fn test_js_compat(&mut self, url: &str) -> JSCompatResult {
        JSCompatResult {
            url: url.to_string(),
            js_executed: false,
            console_errors: vec![],
            page_title: None,
        }
    }

    pub fn send_mouse_move(&self, _x: f32, _y: f32) {}
    pub fn send_mouse_click(&self, _x: f32, _y: f32) {}
    pub fn send_right_click(&self, _x: f32, _y: f32) {}
    pub fn send_scroll(&self, _x: f32, _y: f32, _dx: f64, _dy: f64) {}
    pub fn send_mouse_down(&self, _x: f32, _y: f32) {}
    pub fn send_mouse_up(&self, _x: f32, _y: f32) {}
}
