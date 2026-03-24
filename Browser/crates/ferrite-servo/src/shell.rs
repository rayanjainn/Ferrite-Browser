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
// All Servo-specific code is gated behind the `servo` Cargo feature:
//   cargo build -p ferrite-servo --features servo
//
// Dependency fix: the package is `libservo` (not `servo`) because the servo
// repo root is a workspace-only manifest.  The lib name inside is `servo`, so
// Rust imports still use `use servo::...`.
//
// Without the feature the winit shell compiles and opens a bare window.

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

// ---------------------------------------------------------------------------
// Servo delegate stubs (compiled only with the `servo` feature)
// ---------------------------------------------------------------------------

/// Implements `servo::WebViewDelegate` — receives per-tab callbacks from Servo.
///
/// When Servo finishes rendering a frame it calls `notify_new_frame_ready`;
/// we respond by calling `webview.paint()` to push the rendered pixels to the
/// window surface.
#[cfg(feature = "servo")]
struct FerriteWebViewDelegate;

#[cfg(feature = "servo")]
impl servo::WebViewDelegate for FerriteWebViewDelegate {
    /// Called whenever Servo has a new composited frame ready to display.
    fn notify_new_frame_ready(&self, webview: servo::WebView) {
        webview.paint();
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

    /// Servo browser engine instance.  `None` until `resumed()` fires.
    #[cfg(feature = "servo")]
    servo: Option<servo::Servo>,

    /// Handle to the single top-level WebView (about:blank on startup).
    #[cfg(feature = "servo")]
    webview: Option<servo::WebView>,
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
            use std::rc::Rc;
            use servo::{ServoBuilder, WebViewBuilder};

            // Build the Servo engine.
            //
            // `event_loop_waker` should be a platform waker that calls
            // `event_loop.wake()` when Servo needs attention.  Winit's
            // `EventLoopProxy::send_event` can serve this role once wired up.
            let servo = ServoBuilder::default()
                // .event_loop_waker(waker)   // TODO: wire winit EventLoopProxy
                // .opts(opts)                // TODO: forward CLI opts
                // .preferences(prefs)       // TODO: load user prefs
                .build();

            // Register global delegate (logging, devtools, etc.)
            servo.set_delegate(Rc::new(FerriteServoDelegate));

            // Create the initial WebView and load about:blank.
            //
            // WebViewBuilder::new() requires a RenderingContext (GL surface).
            // This is deferred to Block 4 when surfman/GL setup is added.
            // The commented block below shows the correct call sequence:
            //
            // let rendering_context = ...; // surfman SoftwareRenderingContext or GL
            // let webview = WebViewBuilder::new(&servo, rendering_context)
            //     .delegate(Rc::new(FerriteWebViewDelegate))
            //     .url(url::Url::parse("about:blank").unwrap())
            //     .build();
            // webview.resize(winit::dpi::PhysicalSize::new(1280, 800));
            //
            // For now store the engine and leave webview as None until
            // the rendering context is wired in.

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
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                // ── Drive Servo and composite ───────────────────────────────
                #[cfg(feature = "servo")]
                if let (Some(servo), Some(webview)) =
                    (&mut self.servo, &mut self.webview)
                {
                    // Process pending Servo tasks (layout, JS, network, ...).
                    servo.spin_event_loop();
                    // Push the composited frame to the window surface.
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
}

// ---------------------------------------------------------------------------
// ServoShell public API
// ---------------------------------------------------------------------------

/// Owns the winit `EventLoop` and drives the browser window.
pub struct ServoShell {
    event_loop: EventLoop<()>,
}

impl ServoShell {
    /// Creates the winit `EventLoop`.  The window and (optionally) Servo are
    /// initialised lazily inside `AppHandler::resumed()`.
    pub fn new() -> Self {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        ServoShell { event_loop }
    }

    /// Starts the event loop.  Blocks until the window is closed.
    pub fn run(self) {
        let mut app = AppHandler {
            window: None,
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
