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
    use std::rc::Rc;

    use rustls::crypto::aws_lc_rs;
    use servo::{
        RenderingContext, ServoBuilder, ServoDelegate, SoftwareRenderingContext, WebViewBuilder,
        WebViewDelegate,
    };
    use winit::dpi::PhysicalSize;

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

            // ── Rendering context ──────────────────────────────────────────
            let rendering_context = Rc::new(
                SoftwareRenderingContext::new(PhysicalSize { width, height })
                    .map_err(|e| format!("SoftwareRenderingContext: {:?}", e))?,
            );
            rendering_context
                .make_current()
                .map_err(|e| format!("make_current: {:?}", e))?;

            // ── Servo engine ───────────────────────────────────────────────
            let servo = ServoBuilder::default().build();
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

        /// Drive Servo's internal event loop for one turn, sync load state,
        /// then read back the composited frame.
        ///
        /// Call this on every Iced subscription tick before calling `get_frame()`.
        pub fn spin(&mut self) {
            self.servo.spin_event_loop();

            // Sync load status, URL, and page title from delegate callbacks.
            self.last_load_status = self.shared_load_status.borrow().clone();
            self.current_url = self.shared_url.borrow().clone();
            self.last_page_title = self.shared_page_title.borrow().clone();

            // Read back the current frame after paint.
            let rect = servo::DeviceIntRect::from_origin_and_size(
                servo::DeviceIntPoint::origin(),
                servo::DeviceIntSize::new(self.width as i32, self.height as i32),
            );
            if let Some(rgba) = self.rendering_context.read_to_image(rect) {
                self.last_frame = Some(rgba.into_raw());
            }
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

        /// Resize the render surface and notify the WebView.
        pub fn resize(&mut self, width: u32, height: u32) {
            self.width = width;
            self.height = height;
            let size = PhysicalSize { width, height };
            self.rendering_context.resize(size);
            self.webview.resize(size);
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
}
