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
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{
    keyboard, time, Background, Border, Color, Element, Length, Size, Subscription, Task, Theme,
};
use iced_widget::image::{Handle as ImageHandle, Image as ServoImage};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const ADDRESS_BAR_ID: &str = "ferrite_address_bar";

const TOOLBAR_HEIGHT: f32 = 48.0;
const TAB_BAR_HEIGHT: f32 = 34.0;
#[allow(dead_code)]
const TAB_MIN_WIDTH: f32 = 120.0;
#[allow(dead_code)]
const TAB_MAX_WIDTH: f32 = 240.0;
const BORDER_RADIUS: f32 = 8.0;
const PANEL_PADDING: u16 = 10;

// ---------------------------------------------------------------------------
// Colour palette  (hand-crafted dark theme — no theme tokens needed)
// ---------------------------------------------------------------------------

/// Window/app background — deepest layer.
const C_BASE: Color = Color { r: 0.09, g: 0.09, b: 0.11, a: 1.0 };
/// Surface layer — tab bar, toolbars.
const C_SURFACE: Color = Color { r: 0.12, g: 0.12, b: 0.15, a: 1.0 };
/// Raised surface — inactive tab hover, button hover.
const C_RAISED: Color = Color { r: 0.18, g: 0.18, b: 0.22, a: 1.0 };
/// Divider / separator lines.
const C_DIVIDER: Color = Color { r: 0.22, g: 0.22, b: 0.27, a: 1.0 };
/// Primary text.
const C_TEXT: Color = Color { r: 0.92, g: 0.92, b: 0.95, a: 1.0 };
/// Secondary / muted text.
const C_TEXT_DIM: Color = Color { r: 0.55, g: 0.55, b: 0.62, a: 1.0 };
/// Accent — electric indigo.
const C_ACCENT: Color = Color { r: 0.44, g: 0.38, b: 1.0, a: 1.0 };
/// Accent hover / strong.
#[allow(dead_code)]
const C_ACCENT_BRIGHT: Color = Color { r: 0.58, g: 0.52, b: 1.0, a: 1.0 };
/// Address bar input field.
const C_INPUT: Color = Color { r: 0.15, g: 0.15, b: 0.19, a: 1.0 };
/// Safe / HTTPS green.
const C_SAFE: Color = Color { r: 0.22, g: 0.85, b: 0.55, a: 1.0 };
/// Warning / insecure amber.
const C_WARN: Color = Color { r: 0.95, g: 0.65, b: 0.20, a: 1.0 };

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
            // Pump the shared Servo engine exactly once per tick, then sync
            // each tab's state and read back its frame pixels separately.
            // Pumping more than once causes double-processing of paint messages
            // and can segfault inside Servo's compositor.
            if let Some(first) = state.servo_sessions.values().next() {
                first.pump_engine();
            }
            for session in state.servo_sessions.values_mut() {
                session.sync_and_read();
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

fn separator_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(C_DIVIDER)),
        ..container::Style::default()
    }
}

fn tab_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(C_SURFACE)),
        ..container::Style::default()
    }
}

fn close_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => {
                Color { r: 1.0, g: 0.35, b: 0.35, a: 0.18 }
            }
            _ => Color::TRANSPARENT,
        })),
        // Text colour is set inline on the child text widget; pass through here.
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => {
                Color { r: 1.0, g: 0.50, b: 0.50, a: 1.0 }
            }
            _ => C_TEXT_DIM,
        },
        border: Border {
            radius: iced::border::Radius::new(4.0),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

fn nav_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => C_RAISED,
            button::Status::Pressed => Color { r: C_RAISED.r * 0.85, g: C_RAISED.g * 0.85, b: C_RAISED.b * 0.85, a: 1.0 },
            _ => Color::TRANSPARENT,
        })),
        text_color: match status {
            button::Status::Disabled => Color { a: 0.22, ..C_TEXT_DIM },
            _ => C_TEXT,
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

fn toolbar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(C_BASE)),
        ..container::Style::default()
    }
}

fn audit_btn_active_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(C_ACCENT)),
        text_color: Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..button::Style::default()
    }
}

fn audit_btn_inactive_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => C_RAISED,
            _ => C_SURFACE,
        })),
        text_color: C_TEXT_DIM,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 1.0,
            color: C_DIVIDER,
        },
        ..button::Style::default()
    }
}

fn audit_panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(C_SURFACE)),
        border: Border {
            color: C_DIVIDER,
            width: 1.0,
            radius: iced::border::Radius::new(0.0),
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
    let is_loading_active = state.is_loading;
    let active_tab_idx = state.active_tab;

    // ── Tab bar ────────────────────────────────────────────────────────────
    // Use mouse_area + container instead of button so we have exact control
    // over every pixel — no implicit button padding fighting our layout.
    // Structure per tab:
    //   column [
    //     container(row[favicon, label, ×])   ← TAB_BAR_HEIGHT - 2 px tall
    //     container("")                        ← 2 px accent underline
    //   ]
    const UNDERLINE_H: f32 = 2.0;
    const TAB_INNER_H: f32 = TAB_BAR_HEIGHT - UNDERLINE_H;

    let can_close = state.tabs.len() > 1;

    let mut tab_elements: Vec<Element<FerriteBrowserMessage>> = state
        .tab_titles
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active_tab_idx;
            let is_this_loading = is_loading_active && i == active_tab_idx;

            let favicon_elem: Element<FerriteBrowserMessage> = text(
                if is_this_loading { "⟳" } else { "·" },
            )
            .size(14)
            .color(if is_active { C_ACCENT } else { C_TEXT_DIM })
            .into();

            let label_elem: Element<FerriteBrowserMessage> = text(truncate(label, 20))
                .size(13)
                .color(if is_active { C_TEXT } else { C_TEXT_DIM })
                .into();

            // Close × — a small button; invisible text when only 1 tab open.
            let close_btn_elem: Element<FerriteBrowserMessage> = button(
                text("×")
                    .size(13)
                    .color(if can_close { C_TEXT_DIM } else { Color::TRANSPARENT }),
            )
            .padding([0, 4])
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(close_btn_style)
            .on_press_maybe(can_close.then_some(FerriteBrowserMessage::CloseTab(i)))
            .into();

            // Content row — exact height, vertically centered.
            let content_row: Element<FerriteBrowserMessage> =
                container(
                    row![favicon_elem, label_elem, close_btn_elem]
                        .spacing(6)
                        .align_y(iced::Alignment::Center),
                )
                .width(Length::Shrink)
                .height(Length::Fixed(TAB_INNER_H))
                .padding([0, 10])
                .align_y(iced::Alignment::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(if is_active {
                        C_BASE
                    } else {
                        Color::TRANSPARENT
                    })),
                    border: Border {
                        radius: iced::border::Radius {
                            top_left: BORDER_RADIUS,
                            top_right: BORDER_RADIUS,
                            bottom_left: 0.0,
                            bottom_right: 0.0,
                        },
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into();

            // 2 px underline strip.
            let underline: Element<FerriteBrowserMessage> =
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fixed(UNDERLINE_H))
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(
                            if is_active { C_ACCENT } else { Color::TRANSPARENT },
                        )),
                        ..container::Style::default()
                    })
                    .into();

            // Wrap in mouse_area for click-to-select.
            mouse_area(
                column![content_row, underline]
                    .spacing(0)
                    .width(Length::Shrink)
                    .height(Length::Fixed(TAB_BAR_HEIGHT)),
            )
            .on_press(FerriteBrowserMessage::SelectTab(i))
            .into()
        })
        .collect();

    // "+" new-tab button, same total height.
    tab_elements.push(
        mouse_area(
            container(
                text("+").size(15).color(C_TEXT_DIM).center(),
            )
            .width(Length::Fixed(TAB_BAR_HEIGHT))
            .height(Length::Fixed(TAB_BAR_HEIGHT))
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                ..container::Style::default()
            }),
        )
        .on_press(FerriteBrowserMessage::AddTab)
        .into(),
    );

    let tab_strip = scrollable(
        row(tab_elements)
            .spacing(1)
            .align_y(iced::Alignment::Center)
            .padding([0, 8]),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new().margin(0).scroller_width(2),
    ));

    let tab_bar = container(tab_strip)
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT))
        .align_y(iced::Alignment::Center)
        .style(tab_bar_style);

    // ── Separator ──────────────────────────────────────────────────────────
    let sep_top = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Navigation toolbar ─────────────────────────────────────────────────
    let back_btn = button(text("‹").size(20))
        .padding([6, 10])
        .style(nav_btn_style)
        .on_press_maybe(state.can_go_back.then_some(FerriteBrowserMessage::GoBack));

    let forward_btn = button(text("›").size(20))
        .padding([6, 10])
        .style(nav_btn_style)
        .on_press_maybe(state.can_go_forward.then_some(FerriteBrowserMessage::GoForward));

    let reload_stop_btn: Element<FerriteBrowserMessage> = if state.is_loading {
        button(text("✕").size(14))
            .padding([8, 10])
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::StopLoading)
            .into()
    } else {
        button(text("↺").size(17))
            .padding([6, 10])
            .style(nav_btn_style)
            .on_press(FerriteBrowserMessage::Reload)
            .into()
    };

    let current_url = state
        .tab_urls
        .get(state.active_tab)
        .map(String::as_str)
        .unwrap_or("about:blank");

    let is_https = current_url.starts_with("https://");
    let is_about = current_url == "about:blank";

    // Security indicator: text badge instead of emoji for crisper look
    let security_badge: Element<FerriteBrowserMessage> = if is_about {
        text("").size(12).into()
    } else if is_https {
        text("⚿")
            .size(13)
            .color(C_SAFE)
            .into()
    } else {
        text("!")
            .size(12)
            .color(C_WARN)
            .into()
    };

    let addr_input = text_input(
        if is_about { "Search or enter address…" } else { "" },
        &state.address_bar_input,
    )
    .id(text_input::Id::new(ADDRESS_BAR_ID))
    .width(Length::Fill)
    .padding([7, 10])
    .size(13)
    .style(|_theme: &Theme, status| {
        let focused = matches!(status, text_input::Status::Focused);
        text_input::Style {
            background: Background::Color(C_INPUT),
            border: Border {
                radius: iced::border::Radius::new(20.0),
                width: if focused { 1.5 } else { 1.0 },
                color: if focused { C_ACCENT } else { C_DIVIDER },
            },
            icon: C_TEXT_DIM,
            placeholder: C_TEXT_DIM,
            value: C_TEXT,
            selection: Color { a: 0.30, ..C_ACCENT },
        }
    })
    .on_input(FerriteBrowserMessage::AddressBarChanged)
    .on_submit(FerriteBrowserMessage::NavigateRequested(
        state.address_bar_input.clone(),
    ));

    let addr_area = container(
        row![security_badge, addr_input]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([0, 4]);

    let nav_toolbar = container(
        row![back_btn, forward_btn, reload_stop_btn, addr_area]
            .spacing(2)
            .align_y(iced::Alignment::Center)
            .padding([0, PANEL_PADDING as u16]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .style(toolbar_style);

    // ── Loading bar ────────────────────────────────────────────────────────
    // A 2 px accent stripe that animates across the top of the content area.
    let progress_offset = state.progress_offset;
    let progress_bar: Option<Element<FerriteBrowserMessage>> = if state.is_loading {
        let pulse = 0.6 + 0.4 * (progress_offset * std::f32::consts::TAU * 1.5).sin().abs();
        Some(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(Color { a: pulse, ..C_ACCENT })),
                    ..container::Style::default()
                })
                .into(),
        )
    } else {
        None
    };

    let sep_bottom = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Audit controls ─────────────────────────────────────────────────────
    let audit_toggle_btn = button(
        row![
            text(if state.show_audit_panel { "▾" } else { "▸" }).size(10).color(if state.show_audit_panel { C_ACCENT } else { C_TEXT_DIM }),
            text("Audit Log").size(12),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
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
            button(text("Refresh").size(12))
                .padding([4, 10])
                .style(audit_btn_inactive_style)
                .on_press(FerriteBrowserMessage::RefreshAuditLog)
                .into(),
        );
    }
    let audit_toolbar = container(
        row(audit_items)
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .padding([5, PANEL_PADDING as u16]),
    )
    .width(Length::Fill)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(C_SURFACE)),
        ..container::Style::default()
    });

    // ── Audit panel ────────────────────────────────────────────────────────
    let audit_panel: Option<Element<FerriteBrowserMessage>> = if state.show_audit_panel {
        let header = container(
            row![
                text("SEQ").size(11).color(C_TEXT_DIM).width(40),
                text("TIME").size(11).color(C_TEXT_DIM).width(90),
                text("KIND").size(11).color(C_TEXT_DIM).width(80),
                text("PRINCIPAL").size(11).color(C_TEXT_DIM).width(110),
                text("CAPABILITY").size(11).color(C_TEXT_DIM).width(100),
                text("URL").size(11).color(C_TEXT_DIM).width(Length::Fill),
            ]
            .spacing(8)
            .padding([4, PANEL_PADDING as u16]),
        )
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(C_RAISED)),
            ..container::Style::default()
        });

        let rows: Vec<Element<FerriteBrowserMessage>> = if state.audit_entries.is_empty() {
            vec![container(
                text("No audit entries — run the sandbox demo then click Refresh")
                    .size(12)
                    .color(C_TEXT_DIM)
                    .center(),
            )
            .width(Length::Fill)
            .padding([12, PANEL_PADDING as u16])
            .into()]
        } else {
            state
                .audit_entries
                .iter()
                .map(|entry| {
                    let (kind_str, kind_color) = kind_label(&entry.kind);
                    let timestamp = entry.timestamp.format("%H:%M:%S").to_string();
                    let principal = truncate(&entry.principal_id.to_string(), 8);
                    let capability = entry.capability.as_deref().unwrap_or("—").to_string();
                    let url = truncate(entry.url.as_deref().unwrap_or("—"), 50);

                    container(
                        row![
                            text(entry.sequence.to_string()).size(12).color(C_TEXT_DIM).width(40),
                            text(timestamp).size(12).color(C_TEXT_DIM).width(90),
                            text(kind_str).size(12).color(kind_color).width(80),
                            text(principal).size(12).color(C_TEXT).width(110),
                            text(capability).size(12).color(C_TEXT).width(100),
                            text(url).size(12).color(C_TEXT_DIM).width(Length::Fill),
                        ]
                        .spacing(8)
                        .padding([3, PANEL_PADDING as u16]),
                    )
                    .width(Length::Fill)
                    .into()
                })
                .collect()
        };

        let table = scrollable(column(rows).spacing(0)).height(Length::Fill);

        Some(
            container(column![header, table])
                .width(Length::Fill)
                .height(220)
                .style(audit_panel_style)
                .into(),
        )
    } else {
        None
    };

    // ── Content area ───────────────────────────────────────────────────────
    let active = state.active_tab;

    let content: Element<FerriteBrowserMessage> =
        if let Some(err_msg) = state.tab_error.get(active).and_then(|e| e.as_ref()) {
            // ── Error page ─────────────────────────────────────────────────
            let failed_url = state.tab_urls.get(active).map(String::as_str).unwrap_or("");
            container(
                column![
                    text("⚠").size(42).color(Color { r: 1.0, g: 0.38, b: 0.38, a: 1.0 }),
                    container(text("")).height(8),
                    text("Page could not be loaded").size(22).color(C_TEXT),
                    container(text("")).height(4),
                    text(failed_url).size(13).color(C_TEXT_DIM),
                    container(text("")).height(4),
                    text(err_msg.as_str()).size(12).color(C_TEXT_DIM),
                    container(text("")).height(20),
                    row![
                        button(text("Try Again").size(13))
                            .padding([8, 20])
                            .style(|_t: &Theme, _s| button::Style {
                                background: Some(Background::Color(C_ACCENT)),
                                text_color: Color::WHITE,
                                border: Border {
                                    radius: iced::border::Radius::new(8.0),
                                    ..Border::default()
                                },
                                shadow: iced::Shadow {
                                    color: Color { a: 0.25, ..C_ACCENT },
                                    offset: iced::Vector::new(0.0, 2.0),
                                    blur_radius: 8.0,
                                },
                            })
                            .on_press(FerriteBrowserMessage::Reload),
                        button(text("New Tab").size(13))
                            .padding([8, 20])
                            .style(nav_btn_style)
                            .on_press(FerriteBrowserMessage::NavigateRequested(
                                "about:blank".to_string(),
                            )),
                    ]
                    .spacing(10),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_t: &Theme| container::Style {
                background: Some(Background::Color(C_BASE)),
                ..container::Style::default()
            })
            .into()
        } else if state
            .tab_urls
            .get(active)
            .map(|u| u == "about:blank")
            .unwrap_or(true)
        {
            // ── New-tab page ───────────────────────────────────────────────
            let search_input = text_input("Search or enter address…", &state.new_tab_search_input)
                .width(520)
                .padding([12, 20])
                .size(15)
                .style(|_theme: &Theme, status| {
                    let focused = matches!(status, text_input::Status::Focused);
                    text_input::Style {
                        background: Background::Color(C_INPUT),
                        border: Border {
                            radius: iced::border::Radius::new(26.0),
                            width: if focused { 1.5 } else { 1.0 },
                            color: if focused { C_ACCENT } else { C_DIVIDER },
                        },
                        icon: C_TEXT_DIM,
                        placeholder: C_TEXT_DIM,
                        value: C_TEXT,
                        selection: Color { a: 0.30, ..C_ACCENT },
                    }
                })
                .on_input(FerriteBrowserMessage::NewTabSearchChanged)
                .on_submit(FerriteBrowserMessage::NavigateRequested(resolve_url(
                    &state.new_tab_search_input,
                )));

            let make_shortcut = |label: String, url: String| -> Element<'_, FerriteBrowserMessage> {
                button(
                    column![
                        text("⬡").size(22).color(C_ACCENT),
                        text(label).size(12).color(C_TEXT),
                    ]
                    .spacing(6)
                    .align_x(iced::Alignment::Center),
                )
                .padding([14, 20])
                .style(|_t: &Theme, s| {
                    let hovered = matches!(s, button::Status::Hovered | button::Status::Pressed);
                    button::Style {
                        background: Some(Background::Color(if hovered { C_RAISED } else { C_SURFACE })),
                        text_color: C_TEXT,
                        border: Border {
                            radius: iced::border::Radius::new(12.0),
                            width: 1.0,
                            color: if hovered { C_DIVIDER } else { Color { a: 0.5, ..C_DIVIDER } },
                        },
                        ..button::Style::default()
                    }
                })
                .on_press(FerriteBrowserMessage::NavigateRequested(url))
                .into()
            };

            container(
                column![
                    column![
                        text("⬡").size(52).color(C_ACCENT),
                        text("ferrite").size(36).color(C_TEXT),
                        text("capability-governed browser")
                            .size(13)
                            .color(C_TEXT_DIM),
                    ]
                    .spacing(4)
                    .align_x(iced::Alignment::Center),
                    container(text("")).height(36),
                    search_input,
                    container(text("")).height(28),
                    row![
                        make_shortcut("DuckDuckGo".into(), "https://lite.duckduckgo.com".into()),
                        make_shortcut("Rust Docs".into(), "https://doc.rust-lang.org".into()),
                        make_shortcut("Servo".into(), "https://servo.org".into()),
                        make_shortcut("Ferrite".into(), "https://github.com".into()),
                    ]
                    .spacing(12),
                ]
                .spacing(0)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_t: &Theme| container::Style {
                background: Some(Background::Color(C_BASE)),
                ..container::Style::default()
            })
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
            // ── Loading placeholder ────────────────────────────────────────
            container(
                column![
                    text("⬡").size(32).color(Color { a: 0.3, ..C_ACCENT }),
                    text("Loading…").size(14).color(C_TEXT_DIM),
                ]
                .spacing(10)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_t: &Theme| container::Style {
                background: Some(Background::Color(C_BASE)),
                ..container::Style::default()
            })
            .into()
        };

    // ── Compose layout ─────────────────────────────────────────────────────
    let mut layout: Vec<Element<FerriteBrowserMessage>> =
        vec![tab_bar.into(), sep_top.into(), nav_toolbar.into()];
    if let Some(bar) = progress_bar {
        layout.push(bar);
    } else {
        // Reserve the 2 px slot so content doesn't jump while loading.
        layout.push(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .into(),
        );
    }
    layout.push(sep_bottom.into());
    layout.push(audit_toolbar.into());
    if let Some(panel) = audit_panel {
        layout.push(panel);
    }
    layout.push(content);

    column(layout)
        .into()
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
    iced::application("Ferrite", update, view)
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
