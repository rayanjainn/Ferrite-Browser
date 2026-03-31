// ferrite-ui — Iced UI shell for Ferrite Browser.
//
// ## Architecture
//
// Iced 0.13 functional builder API — no trait to implement. State is
// `FerriteBrowser`, messages are `FerriteBrowserMessage`, and the three
// free functions `update`, `view`, `subscription` are passed to the builder.
//
// ## Interaction model
//
// The Servo frame is rendered as an `iced_widget::image` inside a
// `mouse_area` that captures mouse move, mouse press, mouse release, and
// wheel events and forwards them to `HeadlessServoSession` as native Servo
// input events (`InputEvent::MouseMove`, `MouseButton`, `Wheel`).
//
// ## Keyboard shortcuts (platform-aware)
//   macOS : Cmd+T/W/R/L/J, F5, F12, Alt+←/→, Esc
//   other : Ctrl+T/W/R/L/J, F5, F12, Alt+←/→, Esc

use std::collections::HashMap;

use ferrite_audit_log::{AuditEntry, AuditEventKind, PersistentAuditLog};
use ferrite_capability_broker::{
    BrokerDecision, CapabilityBroker, CapabilityType, Principal, PrincipalKind,
};
use ferrite_servo::session::{HeadlessServoSession, LoadStatus};
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{
    keyboard, time, Background, Border, Color, Element, Length, Size, Subscription, Task, Theme,
};
use iced_widget::image::{Handle as ImageHandle, Image as ServoImage};

// ---------------------------------------------------------------------------
// Platform detection — used for keyboard shortcut labels
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
const MOD_LABEL: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
const MOD_LABEL: &str = "Ctrl";

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const ADDRESS_BAR_ID: &str = "ferrite_address_bar";
const JS_INPUT_ID: &str = "ferrite_js_input";

const TOOLBAR_HEIGHT: f32 = 46.0;
const TAB_BAR_HEIGHT: f32 = 36.0;
const BORDER_RADIUS: f32 = 8.0;
const PANEL_PADDING: u16 = 12;

// ---------------------------------------------------------------------------
// Colour palette
// ---------------------------------------------------------------------------

const C_BASE: Color = Color { r: 0.08, g: 0.08, b: 0.10, a: 1.0 };
const C_SURFACE: Color = Color { r: 0.11, g: 0.11, b: 0.14, a: 1.0 };
const C_RAISED: Color = Color { r: 0.17, g: 0.17, b: 0.21, a: 1.0 };
const C_DIVIDER: Color = Color { r: 0.20, g: 0.20, b: 0.25, a: 1.0 };
const C_TEXT: Color = Color { r: 0.93, g: 0.93, b: 0.96, a: 1.0 };
const C_TEXT_DIM: Color = Color { r: 0.50, g: 0.50, b: 0.58, a: 1.0 };
const C_ACCENT: Color = Color { r: 0.44, g: 0.38, b: 1.0, a: 1.0 };
const C_ACCENT_BRIGHT: Color = Color { r: 0.56, g: 0.50, b: 1.0, a: 1.0 };
const C_INPUT: Color = Color { r: 0.14, g: 0.14, b: 0.18, a: 1.0 };
const C_SAFE: Color = Color { r: 0.20, g: 0.84, b: 0.54, a: 1.0 };
const C_WARN: Color = Color { r: 0.95, g: 0.65, b: 0.20, a: 1.0 };
const C_DANGER: Color = Color { r: 1.0, g: 0.35, b: 0.35, a: 1.0 };

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct FerriteBrowser {
    pub tabs: Vec<String>,
    pub active_tab: usize,
    pub address_bar_input: String,
    /// Committed (navigated-to) URL per tab.
    pub tab_urls: Vec<String>,
    pub show_audit_panel: bool,
    pub show_js_console: bool,
    pub audit_entries: Vec<AuditEntry>,
    pub servo_sessions: HashMap<usize, HeadlessServoSession>,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// Phase for the animated loading bar (0..1).
    pub progress_offset: f32,
    pub address_bar_focused: bool,
    pub tab_error: Vec<Option<String>>,
    pub tab_titles: Vec<String>,
    pub new_tab_search_input: String,
    pub js_input: String,
    pub js_output: Vec<(String, String)>,
    pub js_broker: Option<(CapabilityBroker, uuid::Uuid)>,
    /// Most recent cursor position over the Servo content area (in content pixels).
    pub cursor_pos: (f32, f32),
    /// Y offset of the Servo content area inside the window (toolbar + tab bar heights).
    pub content_y_offset: f32,
}

impl Default for FerriteBrowser {
    fn default() -> Self {
        let mut broker = CapabilityBroker::new();
        let token_id = broker.mint_token(
            Principal {
                id: uuid::Uuid::new_v4(),
                kind: PrincipalKind::Agent,
                label: "ferrite-ui-js-console".to_string(),
            },
            CapabilityType::JsExecute,
            "*".to_string(),
            vec![],
            None,
            3600,
        );
        Self {
            tabs: vec!["New Tab".to_string()],
            active_tab: 0,
            address_bar_input: String::new(),
            tab_urls: vec!["about:blank".to_string()],
            show_audit_panel: false,
            show_js_console: false,
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
            js_input: String::new(),
            js_output: Vec::new(),
            js_broker: Some((broker, token_id)),
            cursor_pos: (0.0, 0.0),
            content_y_offset: TAB_BAR_HEIGHT + TOOLBAR_HEIGHT + 4.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FerriteBrowserMessage {
    AddTab,
    CloseTab(usize),
    SelectTab(usize),
    AddressBarChanged(String),
    NavigateRequested(String),
    ToggleAuditPanel,
    ToggleJsConsole,
    RefreshAuditLog,
    ServoReady,
    ServoFrame,
    GoBack,
    GoForward,
    Reload,
    StopLoading,
    LoadStatusChanged { tab: usize, status: String, url: String },
    FocusAddressBar,
    ClearAddressBarFocus,
    CloseActiveTab,
    EscapePressed,
    NewTabSearchChanged(String),
    JsInputChanged(String),
    JsExecuteRequested,
    JsConsoleClear,
    // Mouse/scroll events forwarded to Servo
    /// Mouse moved over the content area — position is relative to content area origin.
    ServoMouseMove { x: f32, y: f32 },
    /// Mouse button pressed (position taken from last ServoMouseMove).
    ServoMousePress,
    /// Mouse button released (position taken from last ServoMouseMove).
    ServoMouseRelease,
    /// Scroll wheel event.
    ServoScroll { delta_x: f32, delta_y: f32 },
    ContentAreaResized { height: f32 },
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
            match HeadlessServoSession::new(1280, 700) {
                Ok(session) => {
                    state.servo_sessions.insert(new_idx, session);
                }
                Err(e) => eprintln!("[ferrite-ui] Servo session tab {}: {}", new_idx, e),
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
                let keys_to_shift: Vec<usize> =
                    state.servo_sessions.keys().copied().filter(|&k| k > i).collect();
                for k in keys_to_shift {
                    if let Some(session) = state.servo_sessions.remove(&k) {
                        state.servo_sessions.insert(k - 1, session);
                    }
                }
            }
            state.active_tab = state.active_tab.min(state.tabs.len().saturating_sub(1));
            state.address_bar_input = state.tab_urls[state.active_tab].clone();
            sync_nav_state(state);
        }
        FerriteBrowserMessage::SelectTab(i) => {
            state.active_tab = i;
            state.address_bar_input = state.tab_urls[i].clone();
            sync_nav_state(state);
        }
        FerriteBrowserMessage::AddressBarChanged(s) => {
            state.address_bar_input = s;
        }
        FerriteBrowserMessage::NavigateRequested(raw) => {
            let url = resolve_url(&raw);
            state.address_bar_input = url.clone();
            state.tab_urls[state.active_tab] = url.clone();
            if state.active_tab < state.tab_error.len() {
                state.tab_error[state.active_tab] = None;
            }
            state.new_tab_search_input = String::new();
            state.is_loading = true;
            state.address_bar_focused = false;
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
            if state.show_audit_panel {
                state.show_js_console = false;
            }
        }
        FerriteBrowserMessage::ToggleJsConsole => {
            state.show_js_console = !state.show_js_console;
            if state.show_js_console {
                state.show_audit_panel = false;
            }
        }
        FerriteBrowserMessage::RefreshAuditLog => {
            let db_path = std::env::temp_dir()
                .join("ferrite_sandbox.db")
                .to_string_lossy()
                .into_owned();
            state.audit_entries = match PersistentAuditLog::load(&db_path) {
                Ok(log) => log.log.entries,
                Err(e) => {
                    eprintln!("[ferrite-ui] audit log load: {}", e);
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
        FerriteBrowserMessage::JsInputChanged(s) => {
            state.js_input = s;
        }
        FerriteBrowserMessage::JsConsoleClear => {
            state.js_output.clear();
        }
        FerriteBrowserMessage::JsExecuteRequested => {
            let script = state.js_input.trim().to_string();
            if script.is_empty() {
                return Task::none();
            }
            state.js_input.clear();

            let result = if let Some((broker, token_id)) = &state.js_broker {
                match broker.check(*token_id, "*") {
                    BrokerDecision::Granted { .. } => {
                        if let Some(session) =
                            state.servo_sessions.get_mut(&state.active_tab)
                        {
                            match session.execute_js(&script) {
                                Ok(v) => v,
                                Err(e) => format!("Error: {}", e),
                            }
                        } else {
                            "Error: no active Servo session".to_string()
                        }
                    }
                    BrokerDecision::Denied { .. } => {
                        "BLOCKED: step-up consent required".to_string()
                    }
                }
            } else {
                "Error: JS broker not initialised".to_string()
            };

            let snippet = if script.len() > 60 {
                format!("{}...", &script[..59])
            } else {
                script
            };
            state.js_output.push((snippet, result));
        }
        // ── Servo mouse/scroll events ──────────────────────────────────────
        FerriteBrowserMessage::ServoMouseMove { x, y } => {
            state.cursor_pos = (x, y);
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.send_mouse_move(x, y);
            }
        }
        FerriteBrowserMessage::ServoMousePress => {
            let (x, y) = state.cursor_pos;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.send_mouse_down(x, y);
            }
        }
        FerriteBrowserMessage::ServoMouseRelease => {
            let (x, y) = state.cursor_pos;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                // Up event first, then a synthesised click for hit-testing.
                session.send_mouse_up(x, y);
                session.send_mouse_click(x, y);
            }
        }
        FerriteBrowserMessage::ServoScroll { delta_x, delta_y } => {
            let (x, y) = state.cursor_pos;
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.send_scroll(x, y, delta_x as f64, delta_y as f64);
            }
        }
        FerriteBrowserMessage::ContentAreaResized { height } => {
            state.content_y_offset = height;
        }
        FerriteBrowserMessage::ServoReady => {}
        FerriteBrowserMessage::ServoFrame => {
            state.progress_offset = (state.progress_offset + 0.02) % 1.0;
            // Pump engine once, then sync every tab's state and read pixels.
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
                    let status =
                        if is_now_loading { "loading" } else { "complete" }.to_string();
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

fn sync_nav_state(state: &mut FerriteBrowser) {
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
                Color { r: 1.0, g: 0.35, b: 0.35, a: 0.15 }
            }
            _ => Color::TRANSPARENT,
        })),
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => C_DANGER,
            _ => C_TEXT_DIM,
        },
        border: Border {
            radius: iced::border::Radius::new(4.0),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn nav_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => C_RAISED,
            button::Status::Pressed => Color {
                r: C_RAISED.r * 0.80,
                g: C_RAISED.g * 0.80,
                b: C_RAISED.b * 0.80,
                a: 1.0,
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: match status {
            button::Status::Disabled => Color { a: 0.20, ..C_TEXT_DIM },
            _ => C_TEXT,
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            ..Border::default()
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

fn panel_btn_active(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(C_ACCENT)),
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn panel_btn_inactive(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => C_RAISED,
            _ => C_SURFACE,
        })),
        text_color: match status {
            button::Status::Hovered => C_TEXT,
            _ => C_TEXT_DIM,
        },
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            width: 1.0,
            color: C_DIVIDER,
        },
        ..button::Style::default()
    }
}

fn accent_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => C_ACCENT_BRIGHT,
            _ => C_ACCENT,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::Radius::new(BORDER_RADIUS),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn bottom_panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(C_SURFACE)),
        border: Border {
            color: C_DIVIDER,
            width: 1.0,
            radius: iced::border::Radius {
                top_left: BORDER_RADIUS,
                top_right: BORDER_RADIUS,
                bottom_left: 0.0,
                bottom_right: 0.0,
            },
        },
        shadow: iced::Shadow {
            color: Color { a: 0.3, r: 0.0, g: 0.0, b: 0.0 },
            offset: iced::Vector::new(0.0, -4.0),
            blur_radius: 14.0,
        },
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// Audit helpers
// ---------------------------------------------------------------------------

fn kind_label(kind: &AuditEventKind) -> (&'static str, Color) {
    match kind {
        AuditEventKind::CapabilityGranted => ("GRANTED", C_SAFE),
        AuditEventKind::CapabilityDenied => ("DENIED", C_DANGER),
        AuditEventKind::CapabilityExercised => ("USED", Color::from_rgb(0.4, 0.7, 1.0)),
        AuditEventKind::ContentBlocked => ("BLOCKED", C_WARN),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            &s[..s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len())]
        )
    }
}

// ---------------------------------------------------------------------------
// URL resolution
// ---------------------------------------------------------------------------

/// Smart URL resolver — only adds a scheme when one is absent, and picks
/// https vs search based on whether the input looks like a hostname.
fn resolve_url(input: &str) -> String {
    let trimmed = input.trim();

    // Already has a scheme → pass through unchanged.
    if trimmed.contains("://") {
        return trimmed.to_string();
    }

    // Special pages.
    if trimmed == "about:blank" || trimmed.starts_with("about:") {
        return trimmed.to_string();
    }

    // Looks like a hostname (no spaces, has a dot, no special chars that
    // would be illegal in a hostname).  Prepend https://.
    let no_spaces = !trimmed.contains(' ');
    let has_dot = trimmed.contains('.');
    let path_like = trimmed.starts_with('/');
    if no_spaces && (has_dot || path_like) {
        return format!("https://{}", trimmed);
    }

    // Everything else → DuckDuckGo Lite search.
    let encoded = urlencoding::encode(trimmed);
    format!("https://lite.duckduckgo.com/lite/?q={}", encoded)
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    let active_tab_idx = state.active_tab;
    let is_loading_active = state.is_loading;

    // ── Tab bar ────────────────────────────────────────────────────────────
    const UNDERLINE_H: f32 = 2.5;
    const TAB_INNER_H: f32 = TAB_BAR_HEIGHT - UNDERLINE_H;
    let can_close = state.tabs.len() > 1;

    let mut tab_elements: Vec<Element<FerriteBrowserMessage>> = state
        .tab_titles
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active_tab_idx;
            let spinning = is_loading_active && i == active_tab_idx;

            let favicon = text(if spinning { "..." } else if is_active { ">" } else { "-" })
                .size(11)
                .color(if is_active { C_ACCENT } else { C_TEXT_DIM });

            let label_elem = text(truncate(label, 22))
                .size(13)
                .color(if is_active { C_TEXT } else { C_TEXT_DIM });

            let close_btn = button(
                text("x")
                    .size(10)
                    .color(if can_close { C_TEXT_DIM } else { Color::TRANSPARENT }),
            )
            .padding([2, 4])
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0))
            .style(close_btn_style)
            .on_press_maybe(can_close.then_some(FerriteBrowserMessage::CloseTab(i)));

            let content_row = container(
                row![favicon, label_elem, close_btn]
                    .spacing(5)
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
            });

            let underline = container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(UNDERLINE_H))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(if is_active {
                        C_ACCENT
                    } else {
                        Color::TRANSPARENT
                    })),
                    ..container::Style::default()
                });

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

    // "+" new-tab button
    tab_elements.push(
        mouse_area(
            container(text("+").size(17).color(C_TEXT_DIM).center())
                .width(Length::Fixed(TAB_BAR_HEIGHT))
                .height(Length::Fixed(TAB_BAR_HEIGHT))
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .style(|_: &Theme| container::Style::default()),
        )
        .on_press(FerriteBrowserMessage::AddTab)
        .into(),
    );

    let tab_bar = container(
        scrollable(
            row(tab_elements)
                .spacing(1)
                .align_y(iced::Alignment::Center)
                .padding([0, 8]),
        )
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().margin(0).scroller_width(2),
        )),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TAB_BAR_HEIGHT))
    .align_y(iced::Alignment::Center)
    .style(tab_bar_style);

    // ── Separator ──────────────────────────────────────────────────────────
    let sep_top = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(separator_style);

    // ── Toolbar ────────────────────────────────────────────────────────────
    // [←] [→] [↺/✕]  [🔒 address bar ...]  [Audit] [JS]

    let back_btn = button(text("Back").size(12))
        .padding([5, 11])
        .style(nav_btn_style)
        .on_press_maybe(state.can_go_back.then_some(FerriteBrowserMessage::GoBack));

    let fwd_btn = button(text("Fwd").size(12))
        .padding([5, 11])
        .style(nav_btn_style)
        .on_press_maybe(
            state.can_go_forward.then_some(FerriteBrowserMessage::GoForward),
        );

    let reload_btn: Element<FerriteBrowserMessage> = if state.is_loading {
        button(text("Stop").size(12)).padding([6, 11]).style(nav_btn_style)
            .on_press(FerriteBrowserMessage::StopLoading)
            .into()
    } else {
        button(text("Reload").size(12)).padding([6, 11]).style(nav_btn_style)
            .on_press(FerriteBrowserMessage::Reload)
            .into()
    };

    let current_url = state
        .tab_urls
        .get(state.active_tab)
        .map(String::as_str)
        .unwrap_or("about:blank");
    let is_https = current_url.starts_with("https://");
    let is_http_insecure =
        current_url.starts_with("http://") && !current_url.starts_with("https://");
    let is_about = current_url == "about:blank";

    let security_icon: Element<FerriteBrowserMessage> = if is_about {
        text("").size(13).into()
    } else if is_https {
        text("HTTPS").size(10).color(C_SAFE).into()
    } else if is_http_insecure {
        text("HTTP").size(10).color(C_WARN).into()
    } else {
        text("").size(13).into()
    };

    let addr_input = text_input(
        if is_about { "Search or type an address" } else { "" },
        &state.address_bar_input,
    )
    .id(text_input::Id::new(ADDRESS_BAR_ID))
    .width(Length::Fill)
    .padding([7, 10])
    .size(13)
    .style(|_: &Theme, status| {
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

    let addr_row = container(
        row![security_icon, addr_input]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([0, 4]);

    // DevTools toggles with platform shortcut hints in tooltips
    let audit_btn = button(
        row![
            text(if state.show_audit_panel { "v" } else { "+" }).size(10),
            text(" Audit").size(12),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center),
    )
    .padding([5, 10])
    .style(if state.show_audit_panel { panel_btn_active } else { panel_btn_inactive })
    .on_press(FerriteBrowserMessage::ToggleAuditPanel);

    let js_btn = button(
        row![
            text(if state.show_js_console { "v" } else { "+" }).size(10),
            text(" JS").size(12),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center),
    )
    .padding([5, 10])
    .style(if state.show_js_console { panel_btn_active } else { panel_btn_inactive })
    .on_press(FerriteBrowserMessage::ToggleJsConsole);

    let toolbar = container(
        row![back_btn, fwd_btn, reload_btn, addr_row, audit_btn, js_btn]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([0, PANEL_PADDING]),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .style(toolbar_style);

    // ── Progress bar ───────────────────────────────────────────────────────
    let progress_offset = state.progress_offset;
    let maybe_progress: Option<Element<FerriteBrowserMessage>> = if state.is_loading {
        let pulse =
            0.55 + 0.45 * (progress_offset * std::f32::consts::TAU * 1.5).sin().abs();
        Some(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: pulse,
                        ..C_ACCENT
                    })),
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

    // ── Audit panel ────────────────────────────────────────────────────────
    let audit_panel: Option<Element<FerriteBrowserMessage>> = if state.show_audit_panel {
        let hdr = container(
            row![
                text("  Audit Log").size(12).color(C_TEXT).width(Length::Fill),
                button(text("Refresh").size(11))
                    .padding([2, 8])
                    .style(panel_btn_inactive)
                    .on_press(FerriteBrowserMessage::RefreshAuditLog),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([5, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(C_RAISED)),
            ..container::Style::default()
        });

        let col_hdr = container(
            row![
                text("SEQ").size(11).color(C_TEXT_DIM).width(36),
                text("TIME").size(11).color(C_TEXT_DIM).width(76),
                text("KIND").size(11).color(C_TEXT_DIM).width(72),
                text("PRINCIPAL").size(11).color(C_TEXT_DIM).width(95),
                text("CAPABILITY").size(11).color(C_TEXT_DIM).width(95),
                text("URL").size(11).color(C_TEXT_DIM).width(Length::Fill),
            ]
            .spacing(8)
            .padding([3, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.5, ..C_RAISED })),
            ..container::Style::default()
        });

        let rows: Vec<Element<FerriteBrowserMessage>> = if state.audit_entries.is_empty() {
            vec![container(
                text("No audit entries yet - run the sandbox demo and click Refresh")
                    .size(12)
                    .color(C_TEXT_DIM),
            )
            .width(Length::Fill)
            .padding([12, PANEL_PADDING])
            .into()]
        } else {
            state
                .audit_entries
                .iter()
                .map(|e| {
                    let (ks, kc) = kind_label(&e.kind);
                    let ts = e.timestamp.format("%H:%M:%S%.3f").to_string();
                    container(
                        row![
                            text(e.sequence.to_string()).size(12).color(C_TEXT_DIM).width(36),
                            text(ts).size(12).color(C_TEXT_DIM).width(76),
                            text(ks).size(12).color(kc).width(72),
                            text(truncate(&e.principal_id.to_string(), 8))
                                .size(12).color(C_TEXT).width(95),
                            text(e.capability.as_deref().unwrap_or("-"))
                                .size(12).color(C_TEXT).width(95),
                            text(truncate(e.url.as_deref().unwrap_or("-"), 60))
                                .size(12).color(C_TEXT_DIM).width(Length::Fill),
                        ]
                        .spacing(8)
                        .padding([3, PANEL_PADDING]),
                    )
                    .width(Length::Fill)
                    .into()
                })
                .collect()
        };

        Some(
            container(column![hdr, col_hdr, scrollable(column(rows)).height(Length::Fill)])
                .width(Length::Fill)
                .height(220)
                .style(bottom_panel_style)
                .into(),
        )
    } else {
        None
    };

    // ── JS console panel ───────────────────────────────────────────────────
    let js_panel: Option<Element<FerriteBrowserMessage>> = if state.show_js_console {
        let hdr = container(
            row![
                text("  JS Console").size(12).color(C_TEXT).width(Length::Fill),
                text(format!("({} shortcut)", MOD_LABEL)).size(11).color(C_TEXT_DIM),
                button(text("Clear").size(11))
                    .padding([2, 8])
                    .style(panel_btn_inactive)
                    .on_press(FerriteBrowserMessage::JsConsoleClear),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([5, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(C_RAISED)),
            ..container::Style::default()
        });

        let out_rows: Vec<Element<FerriteBrowserMessage>> = if state.js_output.is_empty() {
            vec![container(
                text("Type an expression and press Enter")
                    .size(12)
                    .color(C_TEXT_DIM),
            )
            .padding([10, PANEL_PADDING])
            .into()]
        } else {
            state
                .js_output
                .iter()
                .flat_map(|(snip, res)| {
                    let err = res.starts_with("Error")
                        || res.starts_with("BLOCKED")
                        || res.starts_with("ERROR");
                    [
                        container(text(format!("> {}", snip)).size(12).color(C_ACCENT))
                            .padding([2, PANEL_PADDING])
                            .width(Length::Fill)
                            .into(),
                        container(
                            text(format!("  {}", res))
                                .size(12)
                                .color(if err { C_DANGER } else { C_SAFE }),
                        )
                        .padding([1, PANEL_PADDING])
                        .width(Length::Fill)
                        .into(),
                    ]
                })
                .collect()
        };

        let run_btn = button(text("Run").size(12))
            .padding([6, 12])
            .style(accent_btn_style)
            .on_press(FerriteBrowserMessage::JsExecuteRequested);

        let js_field = text_input("JavaScript expression...", &state.js_input)
            .id(text_input::Id::new(JS_INPUT_ID))
            .width(Length::Fill)
            .padding([6, 8])
            .size(13)
            .style(|_: &Theme, status| {
                let focused = matches!(status, text_input::Status::Focused);
                text_input::Style {
                    background: Background::Color(C_INPUT),
                    border: Border {
                        radius: iced::border::Radius::new(6.0),
                        width: if focused { 1.5 } else { 1.0 },
                        color: if focused { C_ACCENT } else { C_DIVIDER },
                    },
                    icon: C_TEXT_DIM,
                    placeholder: C_TEXT_DIM,
                    value: C_TEXT,
                    selection: Color { a: 0.30, ..C_ACCENT },
                }
            })
            .on_input(FerriteBrowserMessage::JsInputChanged)
            .on_submit(FerriteBrowserMessage::JsExecuteRequested);

        let input_row = container(
            row![text(">").size(13).color(C_ACCENT), js_field, run_btn]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .padding([5, PANEL_PADDING]),
        )
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(C_BASE)),
            ..container::Style::default()
        });

        Some(
            container(column![
                hdr,
                scrollable(column(out_rows).spacing(0).width(Length::Fill)).height(Length::Fill),
                input_row,
            ])
            .width(Length::Fill)
            .height(260)
            .style(bottom_panel_style)
            .into(),
        )
    } else {
        None
    };

    // ── Content area ───────────────────────────────────────────────────────
    let active = state.active_tab;

    let content: Element<FerriteBrowserMessage> =
        if let Some(err_msg) = state.tab_error.get(active).and_then(|e| e.as_ref()) {
            // Error page
            let failed_url = state.tab_urls.get(active).map(String::as_str).unwrap_or("");
            container(
                column![
                    text("ERR").size(42).color(C_DANGER),
                    container(text("")).height(10),
                    text("Page could not be loaded").size(22).color(C_TEXT),
                    container(text("")).height(6),
                    text(failed_url).size(13).color(C_TEXT_DIM),
                    container(text("")).height(4),
                    text(err_msg.as_str()).size(12).color(C_TEXT_DIM),
                    container(text("")).height(28),
                    row![
                        button(text("Try Again").size(13))
                            .padding([9, 24])
                            .style(accent_btn_style)
                            .on_press(FerriteBrowserMessage::Reload),
                        button(text("New Tab").size(13))
                            .padding([9, 24])
                            .style(nav_btn_style)
                            .on_press(FerriteBrowserMessage::NavigateRequested(
                                "about:blank".to_string(),
                            )),
                    ]
                    .spacing(12),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_: &Theme| container::Style {
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
            // Home / new-tab page — always shown for about:blank, even if Servo
            // has produced a blank white frame for that URL.
            new_tab_page(state)
        } else if let Some((w, h, bytes)) =
            state.servo_sessions.get(&active).and_then(|s| s.get_frame())
        {
            // Live Servo frame — interactive via mouse_area
            let handle = ImageHandle::from_rgba(w, h, bytes);
            let img = ServoImage::new(handle).width(Length::Fill).height(Length::Fill);

            mouse_area(container(img).width(Length::Fill).height(Length::Fill))
                .on_move(|pos| FerriteBrowserMessage::ServoMouseMove {
                    x: pos.x,
                    y: pos.y,
                })
                .on_press(FerriteBrowserMessage::ServoMousePress)
                .on_release(FerriteBrowserMessage::ServoMouseRelease)
                .on_scroll(|delta| {
                    use iced::mouse::ScrollDelta;
                    let (dx, dy) = match delta {
                        ScrollDelta::Lines { x, y } => (x * 60.0, y * 60.0),
                        ScrollDelta::Pixels { x, y } => (x, y),
                    };
                    FerriteBrowserMessage::ServoScroll {
                        delta_x: dx,
                        delta_y: dy,
                    }
                })
                .into()
        } else {
            // Loading placeholder (no frame yet for a non-blank URL)
            let pulse = 0.25
                + 0.20
                    * (state.progress_offset * std::f32::consts::TAU).sin().abs();
            container(
                column![
                    text("Fe").size(40).color(Color { a: pulse, ..C_ACCENT }),
                    container(text("")).height(10),
                    text("Loading...").size(14).color(C_TEXT_DIM),
                ]
                .spacing(4)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(C_BASE)),
                ..container::Style::default()
            })
            .into()
        };

    // ── Compose layout ──────────────────────────────────────────────────────
    let mut layout: Vec<Element<FerriteBrowserMessage>> =
        vec![tab_bar.into(), sep_top.into(), toolbar.into()];

    if let Some(bar) = maybe_progress {
        layout.push(bar);
    } else {
        layout.push(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .into(),
        );
    }
    layout.push(sep_bottom.into());

    if let Some(p) = audit_panel {
        layout.push(p);
    } else if let Some(p) = js_panel {
        layout.push(p);
    }

    layout.push(content);

    container(column(layout))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(C_BASE)),
            ..container::Style::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// New-tab / home page
// ---------------------------------------------------------------------------

fn new_tab_page(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    let search_bar = text_input("Search or type an address", &state.new_tab_search_input)
        .width(560)
        .padding([14, 22])
        .size(15)
        .style(|_: &Theme, status| {
            let focused = matches!(status, text_input::Status::Focused);
            text_input::Style {
                background: Background::Color(C_INPUT),
                border: Border {
                    radius: iced::border::Radius::new(30.0),
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

    // Quick-access tiles
    let tiles: Vec<(&str, &str, &str)> = vec![
        ("[D]", "DuckDuckGo", "https://lite.duckduckgo.com"),
        ("[R]", "Rust Docs", "https://doc.rust-lang.org"),
        ("[G]", "GitHub", "https://github.com"),
        ("[S]", "Servo", "https://servo.org"),
        ("[N]", "Hacker News", "https://news.ycombinator.com"),
        ("[W]", "Wikipedia", "https://en.m.wikipedia.org"),
    ];

    let tile_row: Vec<Element<FerriteBrowserMessage>> = tiles
        .iter()
        .map(|(icon, label, url)| {
            let url = url.to_string();
            button(
                column![
                    text(*icon).size(26).color(C_ACCENT),
                    text(*label).size(12).color(C_TEXT_DIM),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .padding([16, 18])
            .style(|_: &Theme, s| {
                let hov = matches!(s, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(if hov { C_RAISED } else { C_SURFACE })),
                    text_color: C_TEXT,
                    border: Border {
                        radius: iced::border::Radius::new(12.0),
                        width: 1.0,
                        color: if hov { C_DIVIDER } else { Color { a: 0.35, ..C_DIVIDER } },
                    },
                    shadow: if hov {
                        iced::Shadow {
                            color: Color { a: 0.15, r: 0.44, g: 0.38, b: 1.0 },
                            offset: iced::Vector::new(0.0, 2.0),
                            blur_radius: 8.0,
                        }
                    } else {
                        iced::Shadow::default()
                    },
                    ..button::Style::default()
                }
            })
            .on_press(FerriteBrowserMessage::NavigateRequested(url))
            .into()
        })
        .collect();

    // Keyboard shortcut reference (platform-aware)
    let shortcuts_text = format!(
        "{M}+T  New tab   {M}+W  Close   {M}+L  Address   {M}+R  Reload   {M}+J  JS Console   F12  Audit",
        M = MOD_LABEL,
    );

    container(
        column![
            // Logo
            column![
                text("Fe").size(64).color(C_ACCENT),
                text("ferrite").size(40).color(C_TEXT),
                text("capability-governed browser").size(13).color(C_TEXT_DIM),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center),
            container(text("")).height(44),
            // Search bar
            search_bar,
            container(text("")).height(36),
            // Quick-access tiles
            row(tile_row).spacing(12).wrap(),
            container(text("")).height(40),
            // Keyboard shortcut hints
            container(
                text(shortcuts_text).size(11).color(C_TEXT_DIM),
            )
            .padding([8, 16])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(C_SURFACE)),
                border: Border {
                    radius: iced::border::Radius::new(8.0),
                    width: 1.0,
                    color: C_DIVIDER,
                },
                ..container::Style::default()
            }),
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(C_BASE)),
        ..container::Style::default()
    })
    .into()
}

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

fn handle_key_press(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<FerriteBrowserMessage> {
    use keyboard::key::Named;

    // On macOS the modifier is the Super (Cmd) key; everywhere else it's Ctrl.
    #[cfg(target_os = "macos")]
    let mod_active = modifiers.command();
    #[cfg(not(target_os = "macos"))]
    let mod_active = modifiers.control();

    match key {
        keyboard::Key::Character(c) if mod_active => match c.as_str() {
            "t" => Some(FerriteBrowserMessage::AddTab),
            "w" => Some(FerriteBrowserMessage::CloseActiveTab),
            "r" => Some(FerriteBrowserMessage::Reload),
            "l" => Some(FerriteBrowserMessage::FocusAddressBar),
            "j" => Some(FerriteBrowserMessage::ToggleJsConsole),
            _ => None,
        },
        keyboard::Key::Named(Named::F5) => Some(FerriteBrowserMessage::Reload),
        keyboard::Key::Named(Named::F12) => Some(FerriteBrowserMessage::ToggleAuditPanel),
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
        time::every(std::time::Duration::from_millis(16))
            .map(|_| FerriteBrowserMessage::ServoFrame)
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
            match HeadlessServoSession::new(1280, 700) {
                Ok(session) => {
                    state.servo_sessions.insert(0, session);
                    (state, Task::done(FerriteBrowserMessage::ServoReady))
                }
                Err(e) => {
                    eprintln!("[ferrite-ui] Servo unavailable: {}", e);
                    (state, Task::none())
                }
            }
        })
}
