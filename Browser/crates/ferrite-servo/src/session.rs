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
    }

    impl WebViewDelegate for HeadlessDelegate {
        fn notify_new_frame_ready(&self, webview: servo::WebView) {
            webview.paint();
        }

        fn notify_load_status_changed(&self, webview: servo::WebView, status: servo::LoadStatus) {
            if status == servo::LoadStatus::Complete {
                let url = webview
                    .url()
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                println!("[ferrite-session] page load complete: {}", url);
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
    pub struct HeadlessServoSession {
        servo: servo::Servo,
        webview: servo::WebView,
        rendering_context: Rc<SoftwareRenderingContext>,
        width: u32,
        height: u32,
        /// Cached last frame as raw RGBA bytes (width × height × 4).
        last_frame: Option<Vec<u8>>,
    }

    impl HeadlessServoSession {
        /// Create a new headless session with a `width × height` render surface.
        ///
        /// Opens `$TMPDIR/ferrite_servo_session.db` for the audit log and mints
        /// a wildcard `NetworkFetch` token (3600 s TTL).
        pub fn new(width: u32, height: u32) -> Result<Self, String> {
            // ── rustls crypto provider ─────────────────────────────────────
            // Servo's network stack uses rustls internally. rustls 0.23
            // requires an explicit CryptoProvider to be installed once per
            // process before any TLS handshake.  `install_default()` is a
            // no-op if another call already installed one.
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

        /// Drive Servo's internal event loop for one turn.  Call this on every
        /// Iced subscription tick before calling `get_frame()`.
        pub fn spin(&mut self) {
            self.servo.spin_event_loop();

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

    pub fn spin(&mut self) {}

    pub fn get_frame(&self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }

    pub fn resize(&mut self, _width: u32, _height: u32) {}
}
