//! The website's screenshots (`site/static/img/*-{dark,light}.png`), drawn by
//! the app's own views from sample state.
//!
//! Ignored: it writes files into the repository, so it runs only when asked:
//!
//! ```text
//! cargo test -p ferrite-ui --lib site_shots -- --ignored
//! ```
//!
//! Each picture is a `FerriteBrowser` set up the way the shot needs, laid out
//! by `iced_runtime::UserInterface` exactly as the window would lay it out,
//! and drawn by iced's software renderer (the same one the app falls back to
//! without a GPU). Nothing is painted by hand. Where a shot shows a page, the
//! page is a picture the engine drew (`FERRITE_SHOT_PAGES`, below), placed in
//! the tab the way a live frame is.

use super::*;

use iced::{mouse, Pixels, Rectangle};
use iced_runtime::user_interface::{Cache, UserInterface};
use iced_widget::graphics::Viewport;

/// Where the pictures go: the website's image folder.
fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../site/static/img")
}

fn load_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let mut fonts = iced_widget::graphics::text::font_system()
            .write()
            .expect("font system");
        for bytes in [
            FONT_INTER_REGULAR,
            FONT_INTER_MEDIUM,
            FONT_INTER_SEMIBOLD,
            FONT_INTER_BOLD,
        ] {
            fonts.load_font(std::borrow::Cow::Borrowed(bytes));
        }
    });
}

/// Lays out and draws `element` at `width` x `height` points, the way the
/// window would, and returns top-row-first RGBA.
fn draw<'a>(
    state: &FerriteBrowser,
    element: Element<'a, FerriteBrowserMessage>,
    width: f32,
    height: f32,
) -> (u32, u32, Vec<u8>) {
    load_fonts();
    let theme = state.theme_mode.to_iced_theme();
    let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
        Font::with_name(FONT_FAMILY),
        Pixels(16.0),
    ));
    let size = Size::new(width, height);
    let mut ui = UserInterface::build(element, size, Cache::default(), &mut renderer);
    // The console keeps its newest message in view, as the app does by
    // snapping it on every new message (a no-op where there is no console).
    ui.operate(
        &renderer,
        &mut iced_widget::core::widget::operation::scrollable::snap_to(
            devtools::console_scroll_id().into(),
            iced::widget::scrollable::RelativeOffset::END,
        ),
    );
    // One pass of events settles widgets that measure themselves on the first
    // draw (`responsive`, scrollables) before the picture is taken.
    let mut messages = Vec::new();
    let _ = ui.update(
        &[],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut iced_widget::core::clipboard::Null,
        &mut messages,
    );
    ui.draw(
        &mut renderer,
        &theme,
        &iced_widget::core::renderer::Style {
            text_color: state.palette().text,
        },
        mouse::Cursor::Unavailable,
    );
    // Drawn while `ui` is alive: the text layers hold weak references to
    // paragraphs its widget tree owns, and draw nothing once those are gone.
    // At one pixel per point, because iced_tiny_skia 0.13 scales an SVG's
    // position and size twice at any other scale (`engine.rs`, `Image::Vector`).
    let iced::Renderer::Secondary(soft) = &mut renderer else {
        unreachable!("built as the software renderer above")
    };
    let (pw, ph) = (width.round() as u32, height.round() as u32);
    let mut pixmap = tiny_skia::Pixmap::new(pw, ph).expect("pixmap");
    let mut mask = tiny_skia::Mask::new(pw, ph).expect("mask");
    soft.draw::<&str>(
        &mut pixmap.as_mut(),
        &mut mask,
        &Viewport::with_physical_size(Size::new(pw, ph), 1.0),
        &[Rectangle::with_size(size)],
        state.palette().base,
        &[],
    );
    drop(ui);
    // The software renderer stores B, G, R, A (it writes for a window
    // surface); everything here is opaque, so no un-premultiplying.
    let mut rgba = pixmap.take();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        px[3] = 255;
    }
    (pw, ph, rgba)
}

fn save(name: &str, theme: AppTheme, (w, h, rgba): (u32, u32, Vec<u8>)) {
    let suffix = match theme {
        AppTheme::Light => "light",
        AppTheme::Dark => "dark",
    };
    let path = out_dir().join(format!("{name}-{suffix}.png"));
    image::save_buffer(&path, &rgba, w, h, image::ExtendedColorType::Rgba8)
        .unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
    eprintln!("wrote {}", path.display());
}

/// A page the engine drew, for a shot that shows one: `<dir>/<name>.png`,
/// rendered by `ferrite-servo`'s `page_shot` example at the content area's
/// size (see `site/shots/README.md`). Without `FERRITE_SHOT_PAGES` a
/// page shot is skipped rather than drawn with a stand-in.
fn page_picture(name: &str) -> Option<(page_view::PageFrame, ImageHandle)> {
    let dir = std::env::var_os("FERRITE_SHOT_PAGES")?;
    let path = PathBuf::from(dir).join(format!("{name}.png"));
    let picture = image::open(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .to_rgba8();
    let (width, height) = picture.dimensions();
    let pixels = picture.into_raw();
    let frame = page_view::PageFrame {
        seq: 1,
        width,
        height,
        pixels: Arc::new(pixels.clone()),
        bottom_up: false,
        bgra: false,
    };
    Some((frame, ImageHandle::from_rgba(width, height, pixels)))
}

/// The four tabs every window shot shows; `active` is the one on screen, at
/// `url` with `title`, and its page is `page` when one was drawn.
fn window(theme: AppTheme, url: &str, title: &str, page: Option<&str>) -> Option<FerriteBrowser> {
    let mut state = themed(theme);
    let tabs = [
        (url, title),
        ("http://localhost:8080/form.html", "Shipping details"),
        ("http://localhost:8080/shop.html", "Checkout demo"),
        ("about:blank", "New Tab"),
    ];
    for (i, (tab_url, tab_title)) in tabs.into_iter().enumerate() {
        if i > 0 {
            push_tab_state(&mut state);
        }
        state.tabs[i] = tab_title.to_string();
        state.tab_titles[i] = tab_title.to_string();
        state.tab_urls[i] = tab_url.to_string();
    }
    state.tab_open_anim = None;
    state.active_tab = 0;
    state.address_bar_input = url.to_string();
    state.can_go_back = true;
    if let Some(name) = page {
        let picture = page_picture(name)?;
        state.frame_cache.insert(0, (picture.0, Some(picture.1)));
    }
    Some(state)
}

fn crop(
    (w, h, rgba): (u32, u32, Vec<u8>),
    x: f32,
    y: f32,
    cw: f32,
    ch: f32,
) -> (u32, u32, Vec<u8>) {
    let (x, y, cw, ch) = (x as u32, y as u32, cw as u32, ch as u32);
    assert!(x + cw <= w && y + ch <= h, "crop inside the picture");
    let mut out = Vec::with_capacity((cw * ch * 4) as usize);
    for row in y..y + ch {
        let at = ((row * w + x) * 4) as usize;
        out.extend_from_slice(&rgba[at..at + (cw * 4) as usize]);
    }
    (cw, ch, out)
}

fn themed(theme: AppTheme) -> FerriteBrowser {
    let mut state = FerriteBrowser {
        theme_mode: theme,
        window_size: Size::new(1280.0, 800.0),
        menu_anim: 1.0,
        drawer_anim: 1.0,
        bottom_anim: 1.0,
        consent_panel_anim: 1.0,
        ..FerriteBrowser::default()
    };
    // A model is connected (the "connect a model" banner is its own story).
    state.settings.active = Some(ferrite_model::settings::ProviderChoice::OllamaCloud);
    state
}

fn news_note() -> ferrite_agent::chat::PageContextNote {
    ferrite_agent::chat::PageContextNote {
        url: "http://localhost:8080/news.html".into(),
        title: "The Daily Digest".into(),
        used_full_page: true,
        reason: "the request is about this page".into(),
    }
}

/// The agent panel after a finished task.
fn agent(theme: AppTheme) -> FerriteBrowser {
    let mut state = window(
        theme,
        "http://localhost:8080/news.html",
        "The Daily Digest",
        None,
    )
    .expect("no page needed");
    state.show_agent_sidebar = true;
    state
        .chat
        .begin_turn("Summarize today's top stories", Some(news_note()));
    state.chat.record_step(StepRecord {
        label: "Read page".into(),
        detail: String::new(),
        result: "The Daily Digest: Top stories. 5 links, 1 navigation bar.".into(),
        blocked: false,
    });
    state.chat.record_step(StepRecord {
        label: "Read text".into(),
        detail: "body".into(),
        result: "1. A small browser engine renders the modern web, slowly but correctly \
                 (312 points) 2. Why predicting what a task needs beats guessing what an \
                 attacker wrote (287 points) ..."
            .into(),
        blocked: false,
    });
    state.chat.finish_turn(Outcome::Answered(
        "Here are today's top stories:\n\n\
         1. **A small browser engine renders the modern web** (312 points)\n\
         2. **Why predicting what a task needs beats guessing what an attacker wrote** (287)\n\
         3. **A hash-chained log you can verify after the fact** (201)\n\n\
         The rest are about Rust workspaces and consent prompts."
            .into(),
    ));
    let turn = state.chat.turns[0].id.to_string();
    state.expanded.insert(agent_panel::expand_key(
        &turn,
        agent_panel::ExpandPart::Steps,
    ));
    state
}

/// The review panel: two actions outside the request, one already rejected.
fn approval(theme: AppTheme) -> FerriteBrowser {
    use ferrite_core::scope::OriginScope;
    use ferrite_core::{Capability, ExpectedCapability, ExpectedCapabilitySet, Origin};
    let mut state = agent(theme);
    state.chat = ferrite_agent::chat::Chat::new();
    state
        .chat
        .begin_turn("Summarize today's top stories", Some(news_note()));
    let mut diff = FingerprintDiff::default();
    diff.extra_primitives.insert(ToolId::new("js.execute"));
    diff.out_of_scope_origins
        .insert("https://attacker.example".into());
    let local = || {
        OriginScope::exact([Origin::parse("http://localhost:8080").expect("origin")])
            .expect("non-empty")
    };
    state.pending_expected = Some(ExpectedFingerprint::from_capabilities(
        ExpectedCapabilitySet::new([
            ExpectedCapability::new(Capability::WebNavigate, local()),
            ExpectedCapability::new(Capability::WebRead, local()),
        ])
        .expect("distinct"),
    ));
    state.pending_diff = Some(diff);
    state.pending_task = Some("Summarize today's top stories".into());
    state
        .pending_decision
        .rejected
        .insert(ToolId::new("js.execute"));
    state
}

/// The live check stopping a navigation the request never implied.
fn runtime(theme: AppTheme) -> FerriteBrowser {
    let mut state = agent(theme);
    state.chat = ferrite_agent::chat::Chat::new();
    state
        .chat
        .begin_turn("Summarize today's top stories", Some(news_note()));
    state.agent_is_running = true;
    state.run_id = 1;
    let verdict = ferrite_ipi::comparator::GuardVerdict::Block(
        ferrite_ipi::comparator::EventVerdict::OutOfScopeOrigin("https://attacker.example".into()),
    );
    let ask = runtime_guard::ask_for(&verdict, "Navigate").expect("a block asks");
    state.pending_runtime = Some(PendingRuntimeConsent {
        run_id: 1,
        action: AgentAction::Navigate {
            url: "https://attacker.example/collect".into(),
        },
        fast: None,
        ask,
    });
    state
}

/// The Settings drawer, connected to a hosted provider.
fn settings(theme: AppTheme) -> FerriteBrowser {
    use ferrite_model::settings::{KeySource, ProviderChoice};
    let mut state = window(theme, "about:blank", "New Tab", None).expect("no page needed");
    state.show_settings_panel = true;
    let s = &mut state.settings;
    let choice = ProviderChoice::OllamaCloud;
    s.saved.provider = Some(choice);
    s.saved.pair_mut(choice).small = "qwen3:8b".into();
    s.saved.pair_mut(choice).main = "deepseek-v3.1:671b".into();
    s.draft = s.saved.clone();
    state.model_tag_small = "qwen3:8b".into();
    state.model_tag_main = "deepseek-v3.1:671b".into();
    let s = &mut state.settings;
    s.same_model = false;
    s.key_source = KeySource::Keyring;
    s.active = Some(choice);
    s.models = [
        "deepseek-v3.1:671b",
        "gpt-oss:120b",
        "qwen3:8b",
        "qwen3-coder:480b",
    ]
    .map(String::from)
    .to_vec();
    s.notice = Some(settings_panel::Notice {
        kind: settings_panel::NoticeKind::Success,
        text: "Saved. The agent will use this model from its next task.".into(),
    });
    state
}

/// When the shop page's messages "arrived": a few seconds before the typed
/// expression below, which the log stamps with the real time.
fn log_ms() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).expect("after 1970") - 5_000
}

/// The shop page with the developer tools open on `tab`.
fn devtools(theme: AppTheme, tab: devtools::DevTab) -> Option<FerriteBrowser> {
    use ferrite_servo::diag::{ConsoleEntry, ConsoleLevel, NetEvent};
    let url = "http://localhost:8080/shop.html";
    let mut state = window(theme, url, "Checkout demo", Some("shop"))?;
    state.show_js_console = true;
    state.devtools.tab = tab;
    let message = |at: u64, level, text: &str, source: Option<&str>| ConsoleEntry {
        at_ms: log_ms() + at,
        level,
        message: text.into(),
        source: source.map(String::from),
    };
    let request = |at: u64, path: &str, kind: &str| NetEvent {
        at_ms: log_ms() - 300 + at,
        method: "GET".into(),
        url: format!("http://localhost:8080/{path}"),
        kind: kind.into(),
        is_main_frame: kind == "Document",
    };
    let log = &mut state.tab_diag[0].log;
    log.ingest(
        vec![
            // What the engine reported for site/shots/pages/shop.html, in order
            // (`page_shot` prints a page's console).
            message(0, ConsoleLevel::Log, r#"app booted {"version":"1.4.2","env":"dev"}"#, None),
            message(10, ConsoleLevel::Info, "cart: 2 items restored from the last visit", None),
            message(20, ConsoleLevel::Warn, "Deprecated API: document.write() called after load", None),
            message(30, ConsoleLevel::Debug, "layout pass 3", None),
            message(
                40,
                ConsoleLevel::Error,
                r#"Error at http://localhost:8080/shop.html:27:10 can't access property "addEventListener", document.getElementById(...) is null"#,
                Some("http://localhost:8080/shop.html:27:10"),
            ),
            message(100, ConsoleLevel::Error, "Failed to load recommendations: HTTP 404", None),
        ],
        vec![
            request(0, "shop.html", "Document"),
            request(20, "demo.css", "Style"),
            request(30, "app.js", "Script"),
            request(40, "logo.png", "Image"),
            request(290, "api/recommendations.json", "Other"),
        ],
        false,
    );
    log.push_input("document.title");
    log.push_result("\"Checkout demo\"");
    Some(state)
}

/// A page control over the form page, entered fully.
fn control(theme: AppTheme, control: ferrite_servo::diag::PageControl) -> Option<FerriteBrowser> {
    let url = "http://localhost:8080/form.html";
    let mut state = window(theme, url, "Shipping details", Some("form"))?;
    let mut pending = controls::PendingControl::new(control, Size::new(1280.0, 725.0));
    pending.shown = 1.0;
    pending.page_url = url.into();
    if let controls::ControlUi::File(file) = &mut pending.ui {
        file.single = "/Users/me/Pictures/photo.png".into();
    }
    state.tab_diag[0].control = Some(pending);
    Some(state)
}

#[test]
#[ignore = "writes the website's screenshots; run on purpose"]
fn site_shots() {
    use ferrite_servo::diag::{DeviceRect, DialogKind, PageControl, SelectOptionView};
    for theme in [AppTheme::Dark, AppTheme::Light] {
        let side = |state: &FerriteBrowser, h: f32| {
            let w = state.panels.side_width(state.window_size);
            draw(state, agent_panel::view_agent_sidebar(state), w, h)
        };
        let full = |state: &FerriteBrowser| {
            let picture = draw(state, view(state), 1280.0, 800.0);
            // The engine's picture must be exactly the area it is shown in,
            // or the shot would show it scaled.
            if let Some((frame, _)) = state.frame_cache.get(&state.active_tab) {
                let area = state.content_area_size.get();
                assert_eq!(
                    (frame.width, frame.height),
                    (area.width as u32, area.height as u32),
                    "page picture vs content area"
                );
            }
            picture
        };

        let s = agent(theme);
        save("agent", theme, side(&s, 725.0));
        let s = approval(theme);
        save("approval", theme, side(&s, 725.0));
        let s = runtime(theme);
        let w = s.panels.side_width(s.window_size);
        save(
            "runtime",
            theme,
            crop(side(&s, 725.0), 0.0, 460.0, w, 265.0),
        );
        let s = settings(theme);
        let w = s.panels.side_width(s.window_size);
        save(
            "settings",
            theme,
            draw(&s, settings_panel::view(&s), w, 1060.0),
        );

        // Shots with a page under the interface: only with the engine's pictures.
        if let Some(s) = window(theme, "http://localhost:8080/", "Ferrite", Some("site")) {
            save("browser", theme, full(&s));
        }
        if let Some(mut s) = window(
            theme,
            "http://localhost:8080/news.html",
            "The Daily Digest",
            Some("news"),
        ) {
            s.show_menu = true;
            save("menu", theme, crop(full(&s), 880.0, 0.0, 400.0, 430.0));
            s.show_menu = false;
            s.tab_diag[0].crash = Some(crash::CrashState::new(ferrite_servo::diag::CrashNote {
                at_ms: log_ms(),
                reason: "assertion failed in layout".into(),
                backtrace: None,
            }));
            save("crash", theme, crop(full(&s), 0.0, 0.0, 1280.0, 230.0));
        }
        if let Some(s) = devtools(theme, devtools::DevTab::Console) {
            save("devtools", theme, full(&s));
        }
        if let Some(s) = devtools(theme, devtools::DevTab::Network) {
            let h = state_bottom(&s);
            save(
                "devtools-network",
                theme,
                crop(full(&s), 0.0, 800.0 - h, 1280.0, h),
            );
        }
        let options = [
            "Canada",
            "France",
            "Germany",
            "India",
            "Japan",
            "Netherlands",
            "United Kingdom",
            "United States",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, label)| SelectOptionView {
            index,
            label: label.into(),
            disabled: false,
            selected: label == "Germany",
            group: None,
        })
        .collect();
        // The select box's place on the page, in device pixels (form.html's
        // `#c`, as the engine laid it out).
        let anchor = DeviceRect {
            x: 19.0,
            y: 137.0,
            width: 340.0,
            height: 36.0,
        };
        if let Some(s) = control(
            theme,
            PageControl::Select {
                options,
                multiple: false,
                anchor,
            },
        ) {
            save(
                "controls-select",
                theme,
                crop(full(&s), 0.0, 75.0, 490.0, 410.0),
            );
        }
        let dialog = PageControl::Dialog {
            kind: DialogKind::Confirm,
            message: "Remove 2 items from your cart?".into(),
            default: String::new(),
        };
        if let Some(s) = control(theme, dialog) {
            save(
                "controls-dialog",
                theme,
                crop(full(&s), 400.0, 300.0, 480.0, 270.0),
            );
        }
        let file = PageControl::File {
            multiple: false,
            accept: vec![".pdf".into(), ".png".into()],
        };
        if let Some(s) = control(theme, file) {
            save(
                "controls-file",
                theme,
                crop(full(&s), 400.0, 300.0, 480.0, 270.0),
            );
        }
    }
}

/// The bottom panel's height in this window.
fn state_bottom(state: &FerriteBrowser) -> f32 {
    state.panels.bottom_height(state.window_size)
}
