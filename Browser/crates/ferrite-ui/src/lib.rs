// ferrite-ui — Iced UI shell for Ferrite Browser.
//
// ## Iced 0.13 API note
//
// Iced 0.13 replaced the `Application` trait with a functional builder API.
// `iced::application(title, update, view)` returns an `Application` builder
// struct; methods like `.theme()`, `.window_size()`, `.centered()`, and
// `.run()` are chained on it.  There is no trait to implement — instead, the
// state, message, update function, and view function are all free items, and
// the builder wires them together.
//
// `FerriteBrowser` is the application state struct.
// `FerriteBrowserMessage` is the message enum.
// `launch()` constructs and runs the application via the builder.
//
// ## Servo integration
//
// `HeadlessServoSession` (from ferrite-servo) is only active when the
// `servo` Cargo feature is enabled on that crate.  Without the feature the
// stub type compiles but `new()` returns `Err`, so `servo_shell` stays `None`
// and the content area shows the placeholder text as before.
//
// The subscription fires 60 times per second.  Each tick it calls `spin()` on
// every active session (drives Servo's event loop + reads back its frame) then
// emits `ServoFrame` so `update()` can display the active tab's latest pixels.

use std::collections::HashMap;

use ferrite_audit_log::{AuditEntry, AuditEventKind, PersistentAuditLog};
use ferrite_servo::session::{HeadlessServoSession, LoadStatus};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{
    keyboard, time, Background, Border, Color, Element, Length, Size, Subscription, Task, Theme,
};
use iced_widget::image::{Handle as ImageHandle, Image as ServoImage};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const ADDRESS_BAR_ID: &str = "ferrite_address_bar";

const TOOLBAR_HEIGHT: f32 = 44.0;
const TAB_BAR_HEIGHT: f32 = 36.0;
#[allow(dead_code)]
const TAB_MIN_WIDTH: f32 = 120.0;
#[allow(dead_code)]
const TAB_MAX_WIDTH: f32 = 240.0;
const BORDER_RADIUS: f32 = 6.0;
const PANEL_PADDING: u16 = 8;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Application state for the Ferrite Browser UI shell.
pub struct FerriteBrowser {
    /// Labels for each open tab.
    pub tabs: Vec<String>,
    /// Index of the currently selected tab.
    pub active_tab: usize,
    /// Live content of the address bar text field.
    pub address_bar_input: String,
    /// The committed (navigated-to) URL for each tab.  Parallel to `tabs`.
    pub tab_urls: Vec<String>,
    /// Whether the audit log panel is open.
    pub show_audit_panel: bool,
    /// Audit entries loaded from the sandbox SQLite database.
    pub audit_entries: Vec<AuditEntry>,
    /// One `HeadlessServoSession` per tab, keyed by tab index.
    pub servo_sessions: HashMap<usize, HeadlessServoSession>,
    /// Whether the active tab is currently loading a page.
    pub is_loading: bool,
    /// Whether the active tab has a previous page to go back to.
    pub can_go_back: bool,
    /// Whether the active tab has a forward page to navigate to.
    pub can_go_forward: bool,
    /// Phase offset (0..1) for the indeterminate progress bar animation.
    pub progress_offset: f32,
    /// Whether the address bar text field is currently focused.
    pub address_bar_focused: bool,
    /// Error message for each tab (None = no error).  Parallel to `tabs`.
    pub tab_error: Vec<Option<String>>,
    /// Page title for each tab (from Servo's title callback).  Parallel to `tabs`.
    pub tab_titles: Vec<String>,
    /// Live content of the search field on the new-tab page.
    pub new_tab_search_input: String,
}

impl Default for FerriteBrowser {
    fn default() -> Self {
        Self {
            tabs: vec!["New Tab".to_string()],
            active_tab: 0,
            address_bar_input: String::new(),
            tab_urls: vec!["about:blank".to_string()],
            show_audit_panel: false,
            audit_entries: vec![],
            servo_sessions: HashMap::new(),
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            progress_offset: 0.0,
            address_bar_focused: false,
            tab_error: vec![None],
            tab_titles: vec!["New Tab".to_string()],
            new_tab_search_input: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Messages the Ferrite Browser UI can receive.
#[derive(Debug, Clone)]
pub enum FerriteBrowserMessage {
    AddTab,
    CloseTab(usize),
    SelectTab(usize),
    AddressBarChanged(String),
    NavigateRequested(String),
    ToggleAuditPanel,
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
    /// Keyboard shortcut Ctrl+W — closes whichever tab is active at dispatch time.
    CloseActiveTab,
    /// Keyboard Escape — stop loading if active, otherwise clear address bar focus.
    EscapePressed,
    /// Live content of the new-tab page search field changed.
    NewTabSearchChanged(String),
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
            let new_idx = state.tabs.len() - 1;
            state.active_tab = new_idx;
            state.address_bar_input = String::new();
            state.is_loading = false;
            state.can_go_back = false;
            state.can_go_forward = false;
            match HeadlessServoSession::new(1280, 600) {
                Ok(session) => {
                    state.servo_sessions.insert(new_idx, session);
                }
                Err(e) => eprintln!("[ferrite-ui] Servo session for tab {}: {}", new_idx, e),
            }
        }
        FerriteBrowserMessage::CloseTab(i) => {
            if state.tabs.len() > 1 {
                state.tabs.remove(i);
                state.tab_urls.remove(i);
                if i < state.tab_error.len() {
                    state.tab_error.remove(i);
                }
                if i < state.tab_titles.len() {
                    state.tab_titles.remove(i);
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
        FerriteBrowserMessage::SelectTab(i) => {
            state.active_tab = i;
            state.address_bar_input = state.tab_urls[i].clone();
            if let Some(session) = state.servo_sessions.get(&i) {
                state.is_loading = matches!(session.load_status(), LoadStatus::Loading);
                state.can_go_back = session.can_go_back();
                state.can_go_forward = session.can_go_forward();
            } else {
                state.is_loading = false;
                state.can_go_back = false;
                state.can_go_forward = false;
            }
        }
        FerriteBrowserMessage::AddressBarChanged(s) => {
            state.address_bar_input = s;
        }
        FerriteBrowserMessage::NavigateRequested(raw) => {
            let url = resolve_url(&raw);
            state.address_bar_input = url.clone();
            state.tab_urls[state.active_tab] = url.clone();
            // Clear any previous error for this tab.
            if state.active_tab < state.tab_error.len() {
                state.tab_error[state.active_tab] = None;
            }
            state.new_tab_search_input = String::new();
            state.is_loading = true;
            state.address_bar_focused = false;
            println!("[ferrite-ui] navigate requested: {}", url);
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
        }
        FerriteBrowserMessage::RefreshAuditLog => {
            let db_path = std::env::temp_dir()
                .join("ferrite_sandbox.db")
                .to_string_lossy()
                .into_owned();
            state.audit_entries = match PersistentAuditLog::load(&db_path) {
                Ok(log) => log.log.entries,
                Err(e) => {
                    eprintln!("[ferrite-ui] audit log load failed: {}", e);
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
        FerriteBrowserMessage::ServoReady => {}
        FerriteBrowserMessage::ServoFrame => {
            state.progress_offset = (state.progress_offset + 0.02) % 1.0;
            for session in state.servo_sessions.values_mut() {
                session.spin();
            }
            let active = state.active_tab;
            if let Some(session) = state.servo_sessions.get(&active) {
                state.can_go_back = session.can_go_back();
                state.can_go_forward = session.can_go_forward();
                // Sync page title for the active tab.
                if let Some(title) = session.page_title() {
                    if active < state.tab_titles.len() && !title.is_empty() {
                        state.tab_titles[active] = title.to_string();
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
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// Styling helpers
// ---------------------------------------------------------------------------

/// 1 px horizontal separator using `background.strong` colour.
fn separator_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.strong.color)),
        ..container::Style::default()
    }
}

fn tab_bar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        ..container::Style::default()
    }
}

/// Active tab: primary.base with a subtle tint, BORDER_RADIUS corners.
fn tab_active_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    // Blend primary base at 15 % alpha over the tab bar background.
    let bg = Color {
        a: 0.15,
        ..palette.primary.base.color
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: palette.background.base.text,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

fn tab_inactive_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.background.strong.color,
            _ => Color::TRANSPARENT,
        })),
        text_color: Color {
            a: match status {
                button::Status::Hovered | button::Status::Pressed => 1.0,
                _ => 0.7,
            },
            ..palette.background.base.text
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

/// Close (×) button — text is invisible until the button itself is hovered.
fn close_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => Color {
                a: 0.2,
                ..palette.danger.base.color
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: Color {
            a: match status {
                button::Status::Hovered | button::Status::Pressed => 0.85,
                _ => 0.0,
            },
            ..palette.background.base.text
        },
        border: Border {
            radius: iced::border::Radius::new(4.0),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

fn add_tab_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.background.strong.color,
            _ => Color::TRANSPARENT,
        })),
        text_color: palette.background.base.text,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

/// Navigation buttons (back, forward, reload/stop) — 32 × 32, rounded square.
fn nav_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => palette.background.strong.color,
            button::Status::Pressed => Color {
                a: 0.7,
                ..palette.background.strong.color
            },
            button::Status::Disabled => Color::TRANSPARENT,
            _ => Color::TRANSPARENT,
        })),
        text_color: Color {
            a: match status {
                button::Status::Disabled => 0.25,
                _ => 1.0,
            },
            ..palette.background.base.text
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

/// Toolbar background = window background (darkest layer).
fn toolbar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        ..container::Style::default()
    }
}

/// Audit Log toggle — active state.
fn audit_btn_active_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(palette.primary.strong.color)),
        text_color: palette.primary.strong.text,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

/// Audit Log toggle — inactive state.
fn audit_btn_inactive_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.background.strong.color,
            _ => palette.background.weak.color,
        })),
        text_color: palette.background.base.text,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

/// Audit panel — rounded top corners, subtle shadow via border.
fn audit_panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: iced::border::Radius {
                top_left: BORDER_RADIUS,
                top_right: BORDER_RADIUS,
                bottom_left: 0.0,
                bottom_right: 0.0,
            },
        },
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// Audit panel helpers
// ---------------------------------------------------------------------------

fn kind_label(kind: &AuditEventKind) -> (&'static str, Color) {
    match kind {
        AuditEventKind::CapabilityGranted => ("GRANTED", Color::from_rgb(0.2, 0.8, 0.4)),
        AuditEventKind::CapabilityDenied => ("DENIED", Color::from_rgb(0.9, 0.3, 0.3)),
        AuditEventKind::CapabilityExercised => ("EXERCISED", Color::from_rgb(0.4, 0.7, 1.0)),
        AuditEventKind::ContentBlocked => ("BLOCKED", Color::from_rgb(1.0, 0.6, 0.2)),
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!(
            "{}...",
            &s[..s
                .char_indices()
                .nth(max_chars)
                .map(|(i, _)| i)
                .unwrap_or(s.len())]
        )
    }
}

// ---------------------------------------------------------------------------
// URL resolution
// ---------------------------------------------------------------------------

/// Resolve a raw address bar input string to a navigable URL.
///
/// Rules (in order):
/// 1. Already has an `http://` or `https://` scheme → use as-is.
/// 2. No spaces and contains a dot → prepend `https://`.
/// 3. Everything else → DuckDuckGo Lite search.
fn resolve_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    if !trimmed.contains(' ') && trimmed.contains('.') {
        return format!("https://{}", trimmed);
    }
    let encoded = urlencoding::encode(trimmed);
    format!("https://lite.duckduckgo.com/lite/?q={}", encoded)
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    // ── Tab bar ────────────────────────────────────────────────────────────
    // Each tab is a column[ tab_row , underline_strip ] so the active tab
    // shows a 2 px accent-colour bottom border without touching button::Style.
    let is_loading_active = state.is_loading;
    let active_tab_idx = state.active_tab;

    let mut tab_elements: Vec<Element<FerriteBrowserMessage>> = state
        .tab_titles
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active_tab_idx;
            let is_this_loading = is_loading_active && is_active;

            // Favicon: spinner when this tab is loading, globe otherwise.
            let favicon = text(if is_this_loading { "⟳" } else { "🌐" }).size(12);

            let label_btn = button(
                row![favicon, text(truncate(label, 22)).size(13)]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
            )
            .padding([0, 12])
            .style(if is_active {
                tab_active_style
            } else {
                tab_inactive_style
            })
            .on_press(FerriteBrowserMessage::SelectTab(i));

            // Close button — text alpha = 0 by default, visible on hover.
            let close_btn = button(text("×").size(14))
                .padding([1, 4])
                .style(close_btn_style)
                .on_press_maybe(
                    (state.tabs.len() > 1).then_some(FerriteBrowserMessage::CloseTab(i)),
                );

            let tab_row = row![label_btn, close_btn]
                .spacing(2)
                .align_y(iced::Alignment::Center);

            // 2 px accent underline for the active tab; transparent otherwise.
            let accent_color = {
                // Captured by move into the style closure below.
                is_active
            };
            let underline = container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .style(move |theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(Background::Color(if accent_color {
                            palette.primary.strong.color
                        } else {
                            Color::TRANSPARENT
                        })),
                        ..container::Style::default()
                    }
                });

            column![tab_row, underline]
                .spacing(0)
                .width(Length::Shrink)
                .into()
        })
        .collect();

    // "+" new-tab button — square 36 × 36 approximated via padding.
    tab_elements.push(
        button(text("+").size(14))
            .padding([10, 10])
            .style(add_tab_style)
            .on_press(FerriteBrowserMessage::AddTab)
            .into(),
    );

    // Horizontal-scrolling tab strip.
    let tab_strip = scrollable(
        row(tab_elements)
            .spacing(4)
            .align_y(iced::Alignment::End)
            .padding([4, 8]),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new().margin(0).scroller_width(3),
    ));

    let tab_bar = container(tab_strip)
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT))
        .style(tab_bar_style);

    // ── Separator: tab bar → nav toolbar ──────────────────────────────────
    let sep_top = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Navigation toolbar ─────────────────────────────────────────────────
    // [←] [→] [⟳/✕]   8px gap   [🔒/🔓  address bar fills width]
    // Buttons are ~32 × 32 via padding [8, 8] + text size 16.

    let back_btn = button(text("←").size(16))
        .padding([8, 8])
        .style(nav_btn_style)
        .on_press_maybe(state.can_go_back.then_some(FerriteBrowserMessage::GoBack));

    let forward_btn = button(text("→").size(16))
        .padding([8, 8])
        .style(nav_btn_style)
        .on_press_maybe(
            state
                .can_go_forward
                .then_some(FerriteBrowserMessage::GoForward),
        );

    let reload_stop_btn: Element<FerriteBrowserMessage> = if state.is_loading {
        button(text("✕").size(16))
            .padding([8, 8])
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::StopLoading)
            .into()
    } else {
        button(text("⟳").size(16))
            .padding([8, 8])
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::Reload)
            .into()
    };

    // Lock icon: green for HTTPS, dimmed for everything else.
    let current_url = state
        .tab_urls
        .get(state.active_tab)
        .map(String::as_str)
        .unwrap_or("about:blank");

    let is_https = current_url.starts_with("https://");
    let lock_icon: Element<FerriteBrowserMessage> = text(if is_https { "🔒" } else { "🔓" })
        .size(13)
        .color(if is_https {
            Color::from_rgb(0.2, 0.8, 0.4) // green
        } else {
            Color {
                a: 0.4,
                r: 0.7,
                g: 0.7,
                b: 0.7,
            } // muted grey
        })
        .into();

    // Pill-shaped address bar: height 32, border-radius 16 (fully rounded ends).
    let addr_input = text_input("Enter URL...", &state.address_bar_input)
        .id(text_input::Id::new(ADDRESS_BAR_ID))
        .width(Length::Fill)
        .padding([6, 10])
        .size(13)
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let focused = matches!(status, text_input::Status::Focused);
            text_input::Style {
                background: Background::Color(palette.background.strong.color),
                border: Border {
                    radius: iced::border::Radius::new(16.0),
                    width: if focused { 1.5 } else { 0.0 },
                    color: if focused {
                        palette.primary.strong.color
                    } else {
                        Color::TRANSPARENT
                    },
                },
                icon: palette.background.base.text,
                placeholder: Color {
                    a: 0.4,
                    ..palette.background.base.text
                },
                value: palette.background.base.text,
                selection: Color {
                    a: 0.35,
                    ..palette.primary.base.color
                },
            }
        })
        .on_input(FerriteBrowserMessage::AddressBarChanged)
        .on_submit(FerriteBrowserMessage::NavigateRequested(
            state.address_bar_input.clone(),
        ));

    // Combine lock icon + address input.
    let addr_area = row![lock_icon, addr_input]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

    let nav_toolbar = container(
        row![back_btn, forward_btn, reload_stop_btn, addr_area]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([0, PANEL_PADDING as u16]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .style(toolbar_style);

    // ── Indeterminate progress bar ─────────────────────────────────────────
    let progress_offset = state.progress_offset;
    let progress_bar: Option<Element<FerriteBrowserMessage>> = if state.is_loading {
        let alpha = 0.5 + 0.5 * (progress_offset * std::f32::consts::TAU).sin().abs();
        Some(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(3.0))
                .style(move |theme: &Theme| {
                    let palette = theme.extended_palette();
                    let mut color = palette.primary.strong.color;
                    color.a = alpha;
                    container::Style {
                        background: Some(Background::Color(color)),
                        ..container::Style::default()
                    }
                })
                .into(),
        )
    } else {
        None
    };

    // ── Separator: nav toolbar → viewport ─────────────────────────────────
    let sep_bottom = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Audit controls toolbar ─────────────────────────────────────────────
    let audit_toggle_btn = button(text("Audit Log").size(14))
        .padding([4, 10])
        .style(if state.show_audit_panel {
            audit_btn_active_style
        } else {
            audit_btn_inactive_style
        })
        .on_press(FerriteBrowserMessage::ToggleAuditPanel);

    let mut audit_items: Vec<Element<FerriteBrowserMessage>> = vec![audit_toggle_btn.into()];
    if state.show_audit_panel {
        audit_items.push(
            button(text("Refresh").size(14))
                .padding([4, 10])
                .style(audit_btn_inactive_style)
                .on_press(FerriteBrowserMessage::RefreshAuditLog)
                .into(),
        );
    }
    let audit_toolbar = container(
        row(audit_items)
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([4, PANEL_PADDING as u16]),
    )
    .width(Length::Fill)
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            ..container::Style::default()
        }
    });

    // ── Audit panel ────────────────────────────────────────────────────────
    let audit_panel: Option<Element<FerriteBrowserMessage>> = if state.show_audit_panel {
        let header = container(
            row![
                text("Seq").size(11).width(40),
                text("Timestamp").size(11).width(160),
                text("Kind").size(11).width(80),
                text("Principal").size(11).width(120),
                text("Capability").size(11).width(100),
                text("URL").size(11).width(Length::Fill),
            ]
            .spacing(8)
            .padding([2, PANEL_PADDING as u16]),
        )
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.strong.color)),
                ..container::Style::default()
            }
        });

        let rows: Vec<Element<FerriteBrowserMessage>> = if state.audit_entries.is_empty() {
            vec![container(
                text("No audit entries — run the sandbox demo then click Refresh")
                    .size(12)
                    .center(),
            )
            .width(Length::Fill)
            .padding([8, PANEL_PADDING as u16])
            .into()]
        } else {
            state
                .audit_entries
                .iter()
                .map(|entry| {
                    let (kind_str, kind_color) = kind_label(&entry.kind);
                    let timestamp = entry.timestamp.format("%H:%M:%S%.3f").to_string();
                    let principal = truncate(&entry.principal_id.to_string(), 8);
                    let capability = entry.capability.as_deref().unwrap_or("—").to_string();
                    let url = truncate(entry.url.as_deref().unwrap_or("—"), 40);

                    container(
                        row![
                            text(entry.sequence.to_string()).size(12).width(40),
                            text(timestamp).size(12).width(160),
                            text(kind_str).size(12).color(kind_color).width(80),
                            text(principal).size(12).width(120),
                            text(capability).size(12).width(100),
                            text(url).size(12).width(Length::Fill),
                        ]
                        .spacing(8)
                        .padding([2, PANEL_PADDING as u16]),
                    )
                    .width(Length::Fill)
                    .into()
                })
                .collect()
        };

        let table = scrollable(column(rows).spacing(1)).height(Length::Fill);

        Some(
            container(column![header, table])
                .width(Length::Fill)
                .height(250)
                .style(audit_panel_style)
                .into(),
        )
    } else {
        None
    };

    // ── Content area ───────────────────────────────────────────────────────
    let active = state.active_tab;

    // Priority: error page > new-tab page > Servo frame > fallback placeholder.
    let content: Element<FerriteBrowserMessage> =
        if let Some(err_msg) = state.tab_error.get(active).and_then(|e| e.as_ref()) {
            // ── Error page ─────────────────────────────────────────────────
            let failed_url = state.tab_urls.get(active).map(String::as_str).unwrap_or("");
            container(
                column![
                    text("⚠").size(48).color(Color::from_rgb(0.9, 0.3, 0.3)),
                    text("Could not load page").size(20),
                    text(failed_url).size(13).color(Color {
                        a: 0.6,
                        r: 0.8,
                        g: 0.8,
                        b: 0.8,
                    }),
                    text(err_msg.as_str()).size(12).color(Color {
                        a: 0.5,
                        r: 0.8,
                        g: 0.8,
                        b: 0.8,
                    }),
                    button(text("Try Again").size(14))
                        .padding([6, 16])
                        .style(nav_btn_style)
                        .on_press(FerriteBrowserMessage::Reload),
                    button(text("Go Home").size(14))
                        .padding([6, 16])
                        .style(nav_btn_style)
                        .on_press(FerriteBrowserMessage::NavigateRequested(
                            "https://lite.duckduckgo.com".to_string(),
                        )),
                ]
                .spacing(12)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into()
        } else if state
            .tab_urls
            .get(active)
            .map(|u| u == "about:blank")
            .unwrap_or(true)
        {
            // ── New-tab page ───────────────────────────────────────────────
            let search_input = text_input("Search or enter address", &state.new_tab_search_input)
                .width(480)
                .padding([10, 16])
                .size(14)
                .style(|theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let focused = matches!(status, text_input::Status::Focused);
                    text_input::Style {
                        background: Background::Color(palette.background.strong.color),
                        border: Border {
                            radius: iced::border::Radius::new(22.0),
                            width: if focused { 1.5 } else { 0.0 },
                            color: if focused {
                                palette.primary.strong.color
                            } else {
                                Color::TRANSPARENT
                            },
                        },
                        icon: palette.background.base.text,
                        placeholder: Color {
                            a: 0.4,
                            ..palette.background.base.text
                        },
                        value: palette.background.base.text,
                        selection: Color {
                            a: 0.35,
                            ..palette.primary.base.color
                        },
                    }
                })
                .on_input(FerriteBrowserMessage::NewTabSearchChanged)
                .on_submit(FerriteBrowserMessage::NavigateRequested(resolve_url(
                    &state.new_tab_search_input,
                )));

            container(
                column![
                    text("ferrite").size(48),
                    text("capability-governed browser").size(14).color(Color {
                        a: 0.6,
                        r: 0.8,
                        g: 0.8,
                        b: 0.8,
                    }),
                    container(text("")).height(32),
                    search_input,
                    container(text("")).height(16),
                    row![
                        button(text("DuckDuckGo").size(13))
                            .padding([6, 12])
                            .style(nav_btn_style)
                            .on_press(FerriteBrowserMessage::NavigateRequested(
                                "https://lite.duckduckgo.com".to_string()
                            )),
                        button(text("Rust Docs").size(13))
                            .padding([6, 12])
                            .style(nav_btn_style)
                            .on_press(FerriteBrowserMessage::NavigateRequested(
                                "https://doc.rust-lang.org".to_string()
                            )),
                        button(text("Servo").size(13))
                            .padding([6, 12])
                            .style(nav_btn_style)
                            .on_press(FerriteBrowserMessage::NavigateRequested(
                                "https://servo.org".to_string()
                            )),
                    ]
                    .spacing(8),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into()
        } else if let Some((w, h, bytes)) = state
            .servo_sessions
            .get(&active)
            .and_then(|s| s.get_frame())
        {
            // ── Servo frame ────────────────────────────────────────────────
            let handle = ImageHandle::from_rgba(w, h, bytes);
            container(
                ServoImage::new(handle)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            // ── Placeholder (session not ready yet) ────────────────────────
            container(text("Loading…").size(18).center())
                .width(Length::Fill)
                .height(Length::Fill)
                .center(Length::Fill)
                .into()
        };

    // ── Compose layout ─────────────────────────────────────────────────────
    let mut layout: Vec<Element<FerriteBrowserMessage>> =
        vec![tab_bar.into(), sep_top.into(), nav_toolbar.into()];
    if let Some(bar) = progress_bar {
        layout.push(bar);
    }
    layout.push(sep_bottom.into());
    layout.push(audit_toolbar.into());
    if let Some(panel) = audit_panel {
        layout.push(panel);
    }
    layout.push(content);

    column(layout).into()
}

// ---------------------------------------------------------------------------
// Subscription — Servo tick
// ---------------------------------------------------------------------------

fn handle_key_press(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<FerriteBrowserMessage> {
    use keyboard::key::Named;
    match key {
        keyboard::Key::Character(c) if modifiers.control() => match c.as_str() {
            "t" => Some(FerriteBrowserMessage::AddTab),
            "w" => Some(FerriteBrowserMessage::CloseActiveTab),
            "r" => Some(FerriteBrowserMessage::Reload),
            "l" => Some(FerriteBrowserMessage::FocusAddressBar),
            _ => None,
        },
        keyboard::Key::Named(Named::F5) => Some(FerriteBrowserMessage::Reload),
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

    Subscription::batch([keyboard_sub, servo_tick])
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

pub fn launch() -> iced::Result {
    iced::application("Ferrite Browser", update, view)
        .window_size(Size::new(1280.0, 800.0))
        .centered()
        .theme(|_state| Theme::Dark)
        .subscription(subscription)
        .run_with(|| {
            let mut state = FerriteBrowser::default();
            match HeadlessServoSession::new(1280, 600) {
                Ok(session) => {
                    state.servo_sessions.insert(0, session);
                    (state, Task::done(FerriteBrowserMessage::ServoReady))
                }
                Err(e) => {
                    eprintln!("[ferrite-ui] Servo session unavailable: {}", e);
                    (state, Task::none())
                }
            }
        })
}
