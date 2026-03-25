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
use ferrite_servo::session::HeadlessServoSession;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{time, Background, Color, Element, Length, Size, Subscription, Task, Theme};
use iced_widget::image::{Handle as ImageHandle, Image as ServoImage};

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
    /// Sessions are created on `AddTab` / startup and destroyed on `CloseTab`.
    pub servo_sessions: HashMap<usize, HeadlessServoSession>,
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
        }
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Messages the Ferrite Browser UI can receive.
#[derive(Debug, Clone)]
pub enum FerriteBrowserMessage {
    /// Open a new tab at the end of the tab strip.
    AddTab,
    /// Close the tab at index `i`.
    CloseTab(usize),
    /// Make tab `i` the active tab.
    SelectTab(usize),
    /// User typed in the address bar — `s` is the new full field value.
    AddressBarChanged(String),
    /// User pressed Enter in the address bar — commit the URL and navigate.
    NavigateRequested(String),
    /// Toggle the audit log panel open/closed.
    ToggleAuditPanel,
    /// Re-read the sandbox audit DB and refresh the displayed entries.
    RefreshAuditLog,
    /// Servo session initialised and ready.
    ServoReady,
    /// Subscription tick — spin Servo and grab the latest rendered frame.
    ServoFrame,
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

/// Processes a `FerriteBrowserMessage` and mutates `FerriteBrowser` state.
pub fn update(
    state: &mut FerriteBrowser,
    message: FerriteBrowserMessage,
) -> Task<FerriteBrowserMessage> {
    match message {
        FerriteBrowserMessage::AddTab => {
            state.tabs.push("New Tab".to_string());
            state.tab_urls.push("about:blank".to_string());
            let new_idx = state.tabs.len() - 1;
            state.active_tab = new_idx;
            state.address_bar_input = "about:blank".to_string();
            // Spawn a Servo session for the new tab.
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
                // Drop the session for the closed tab.
                state.servo_sessions.remove(&i);
                // Re-key all sessions with index > i down by one.
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
        }
        FerriteBrowserMessage::SelectTab(i) => {
            state.active_tab = i;
            state.address_bar_input = state.tab_urls[i].clone();
        }
        FerriteBrowserMessage::AddressBarChanged(s) => {
            state.address_bar_input = s;
        }
        FerriteBrowserMessage::NavigateRequested(url) => {
            state.tab_urls[state.active_tab] = url.clone();
            println!("[ferrite-ui] navigate requested: {}", url);
            if let Some(session) = state.servo_sessions.get(&state.active_tab) {
                session.navigate(&url);
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
        FerriteBrowserMessage::ServoReady => {
            // Initial session for tab 0 was created in run_with(); nothing
            // more to do here — the session is already in servo_sessions[0].
        }
        FerriteBrowserMessage::ServoFrame => {
            // Tick: spin all active sessions so background tabs stay alive,
            // but only the active tab's frame is displayed.
            for session in state.servo_sessions.values_mut() {
                session.spin();
            }
        }
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// Styling helpers
// ---------------------------------------------------------------------------

fn tab_bar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        ..container::Style::default()
    }
}

fn tab_inactive_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.background.strong.color,
            _ => palette.background.weak.color,
        })),
        text_color: palette.background.base.text,
        border: iced::Border {
            radius: iced::border::Radius::new(4.0),
            ..iced::Border::default()
        },
        ..button::Style::default()
    }
}

fn tab_active_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(palette.primary.strong.color)),
        text_color: palette.primary.strong.text,
        border: iced::Border {
            radius: iced::border::Radius::new(4.0),
            ..iced::Border::default()
        },
        ..button::Style::default()
    }
}

fn close_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => Color {
                a: 0.15,
                ..palette.danger.base.color
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: palette.background.base.text,
        border: iced::Border {
            radius: iced::border::Radius::new(3.0),
            ..iced::Border::default()
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
        border: iced::Border {
            radius: iced::border::Radius::new(4.0),
            ..iced::Border::default()
        },
        ..button::Style::default()
    }
}

/// Active state for the "Audit Log" toggle button.
fn audit_btn_active_style(theme: &Theme, _status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(palette.primary.strong.color)),
        text_color: palette.primary.strong.text,
        border: iced::Border {
            radius: iced::border::Radius::new(4.0),
            ..iced::Border::default()
        },
        ..button::Style::default()
    }
}

/// Inactive state for the "Audit Log" toggle button.
fn audit_btn_inactive_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => palette.background.strong.color,
            _ => palette.background.weak.color,
        })),
        text_color: palette.background.base.text,
        border: iced::Border {
            radius: iced::border::Radius::new(4.0),
            ..iced::Border::default()
        },
        ..button::Style::default()
    }
}

fn toolbar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        ..container::Style::default()
    }
}

fn audit_panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: iced::Border {
            color: palette.background.strong.color,
            width: 1.0,
            ..iced::Border::default()
        },
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// Audit panel helpers
// ---------------------------------------------------------------------------

/// Display label and colour for an `AuditEventKind`.
fn kind_label(kind: &AuditEventKind) -> (&'static str, Color) {
    match kind {
        AuditEventKind::CapabilityGranted => ("GRANTED", Color::from_rgb(0.2, 0.8, 0.4)),
        AuditEventKind::CapabilityDenied => ("DENIED", Color::from_rgb(0.9, 0.3, 0.3)),
        AuditEventKind::CapabilityExercised => ("EXERCISED", Color::from_rgb(0.4, 0.7, 1.0)),
        AuditEventKind::ContentBlocked => ("BLOCKED", Color::from_rgb(1.0, 0.6, 0.2)),
    }
}

/// Truncate a string to `max_chars`, appending "..." if truncated.
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
// View
// ---------------------------------------------------------------------------

/// Renders the current `FerriteBrowser` state.
///
/// Layout:
///   ┌─────────────────────────────────────────┐
///   │  [Tab 0] [×]  [Tab 1] [×]  …  [+]      │  ← tab bar
///   ├─────────────────────────────────────────┤
///   │  [address bar]                          │  ← address bar
///   ├─────────────────────────────────────────┤
///   │  [Audit Log]  [Refresh]                 │  ← toolbar
///   ├─────────────────────────────────────────┤
///   │  Seq | Timestamp | Kind | … (250px)     │  ← audit panel (when open)
///   ├─────────────────────────────────────────┤
///   │                                         │
///   │         Tab N content                   │  ← content area
///   │                                         │
///   └─────────────────────────────────────────┘
pub fn view(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    // ── Tab bar ────────────────────────────────────────────────────────────
    let mut tab_buttons: Vec<Element<FerriteBrowserMessage>> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == state.active_tab;
            let label_btn = button(text(label.as_str()))
                .padding([4, 10])
                .style(if is_active {
                    tab_active_style
                } else {
                    tab_inactive_style
                })
                .on_press(FerriteBrowserMessage::SelectTab(i));
            let close_btn = button(text("×"))
                .padding([4, 6])
                .style(close_btn_style)
                .on_press_maybe(
                    (state.tabs.len() > 1).then_some(FerriteBrowserMessage::CloseTab(i)),
                );
            row![label_btn, close_btn]
                .spacing(2)
                .align_y(iced::Alignment::Center)
                .into()
        })
        .collect();
    tab_buttons.push(
        button(text("+"))
            .padding([4, 10])
            .style(add_tab_style)
            .on_press(FerriteBrowserMessage::AddTab)
            .into(),
    );
    let tab_bar = container(
        row(tab_buttons)
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([4, 8]),
    )
    .width(Length::Fill)
    .style(tab_bar_style);

    // ── Address bar ────────────────────────────────────────────────────────
    let current_url = state
        .tab_urls
        .get(state.active_tab)
        .map(String::as_str)
        .unwrap_or("about:blank");
    let addr_bar = container(
        column![
            text_input("Enter URL...", &state.address_bar_input)
                .width(Length::Fill)
                .padding([6, 10])
                .on_input(FerriteBrowserMessage::AddressBarChanged)
                .on_submit(FerriteBrowserMessage::NavigateRequested(
                    state.address_bar_input.clone(),
                )),
            text(format!("Current URL: {}", current_url)).size(12),
        ]
        .spacing(2)
        .padding([4, 8]),
    )
    .width(Length::Fill);

    // ── Toolbar ────────────────────────────────────────────────────────────
    let audit_toggle_btn = button(text("Audit Log"))
        .padding([4, 10])
        .style(if state.show_audit_panel {
            audit_btn_active_style
        } else {
            audit_btn_inactive_style
        })
        .on_press(FerriteBrowserMessage::ToggleAuditPanel);

    let mut toolbar_items: Vec<Element<FerriteBrowserMessage>> = vec![audit_toggle_btn.into()];
    if state.show_audit_panel {
        toolbar_items.push(
            button(text("Refresh"))
                .padding([4, 10])
                .style(add_tab_style)
                .on_press(FerriteBrowserMessage::RefreshAuditLog)
                .into(),
        );
    }
    let toolbar = container(
        row(toolbar_items)
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .padding([4, 8]),
    )
    .width(Length::Fill)
    .style(toolbar_style);

    // ── Audit panel ────────────────────────────────────────────────────────
    let audit_panel: Option<Element<FerriteBrowserMessage>> = if state.show_audit_panel {
        // Header row
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
            .padding([2, 8]),
        )
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.strong.color)),
                ..container::Style::default()
            }
        });

        // Data rows
        let rows: Vec<Element<FerriteBrowserMessage>> = if state.audit_entries.is_empty() {
            vec![container(
                text("No audit entries — run the sandbox demo then click Refresh")
                    .size(12)
                    .center(),
            )
            .width(Length::Fill)
            .padding([8, 8])
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
                        .padding([2, 8]),
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
    // Render the active tab's latest Servo frame if one is available.
    // Falls back to placeholder text when the session has no frame yet or
    // when the `servo` feature is not enabled.
    let active_frame = state
        .servo_sessions
        .get(&state.active_tab)
        .and_then(|s| s.get_frame());

    let content: Element<FerriteBrowserMessage> = if let Some((w, h, bytes)) = active_frame {
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
        container(
            text(format!("Tab {} content", state.active_tab))
                .size(18)
                .center(),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
    };

    // ── Compose layout ─────────────────────────────────────────────────────
    let mut layout: Vec<Element<FerriteBrowserMessage>> =
        vec![tab_bar.into(), addr_bar.into(), toolbar.into()];
    if let Some(panel) = audit_panel {
        layout.push(panel);
    }
    layout.push(content);

    column(layout).into()
}

// ---------------------------------------------------------------------------
// Subscription — Servo tick
// ---------------------------------------------------------------------------

/// Drives Servo at ~60 fps when a session exists.
///
/// Uses `iced::time::every` (a `Subscription` that fires at a fixed interval)
/// to emit `ServoFrame` messages.  `update()` responds by calling
/// `session.spin()` and updating `state.servo_frame`.
pub fn subscription(state: &FerriteBrowser) -> Subscription<FerriteBrowserMessage> {
    if !state.servo_sessions.is_empty() {
        time::every(std::time::Duration::from_millis(16)).map(|_| FerriteBrowserMessage::ServoFrame)
    } else {
        Subscription::none()
    }
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

/// Launches the Ferrite Browser UI.
///
/// On startup a `HeadlessServoSession` (1280×600 surface) is initialised.
/// If the `servo` feature is not enabled on `ferrite-servo` the session stub
/// returns `Err` and `servo_shell` stays `None` — the UI still works, just
/// without a live browser viewport.
pub fn launch() -> iced::Result {
    iced::application("Ferrite Browser", update, view)
        .window_size(Size::new(1280.0, 800.0))
        .centered()
        .theme(|_state| Theme::Dark)
        .subscription(subscription)
        .run_with(|| {
            let mut state = FerriteBrowser::default();
            // Seed tab 0 with a Servo session on startup.
            // HeadlessServoSession::new() is a no-op stub when the `servo`
            // feature is off — it returns Err immediately and the UI runs
            // without a live viewport.
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
