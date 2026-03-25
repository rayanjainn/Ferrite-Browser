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

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Background, Color, Element, Length, Size, Task, Theme};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Application state for the Ferrite Browser UI shell.
#[derive(Debug)]
pub struct FerriteBrowser {
    /// Labels for each open tab.
    pub tabs: Vec<String>,
    /// Index of the currently selected tab.
    pub active_tab: usize,
    /// Live content of the address bar text field.
    pub address_bar_input: String,
    /// The committed (navigated-to) URL for each tab.  Parallel to `tabs`.
    pub tab_urls: Vec<String>,
}

impl Default for FerriteBrowser {
    fn default() -> Self {
        Self {
            tabs: vec!["New Tab".to_string()],
            active_tab: 0,
            address_bar_input: String::new(),
            tab_urls: vec!["about:blank".to_string()],
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
            state.active_tab = state.tabs.len() - 1;
            // Populate the address bar with the new tab's URL.
            state.address_bar_input = state.tab_urls[state.active_tab].clone();
        }
        FerriteBrowserMessage::CloseTab(i) => {
            if state.tabs.len() > 1 {
                state.tabs.remove(i);
                state.tab_urls.remove(i);
            }
            state.active_tab = state.active_tab.min(state.tabs.len().saturating_sub(1));
            state.address_bar_input = state.tab_urls[state.active_tab].clone();
        }
        FerriteBrowserMessage::SelectTab(i) => {
            state.active_tab = i;
            // Reflect the selected tab's committed URL in the address bar.
            state.address_bar_input = state.tab_urls[i].clone();
        }
        FerriteBrowserMessage::AddressBarChanged(s) => {
            state.address_bar_input = s;
        }
        FerriteBrowserMessage::NavigateRequested(url) => {
            state.tab_urls[state.active_tab] = url.clone();
            println!("[ferrite-ui] navigate requested: {}", url);
        }
    }
    Task::none()
}

// ---------------------------------------------------------------------------
// Styling helpers
// ---------------------------------------------------------------------------

/// Background colour for the tab strip — one step lighter than the window
/// background using the theme's `background.weak` palette slot.
fn tab_bar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        ..container::Style::default()
    }
}

/// Style for an inactive tab button.
fn tab_inactive_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let base_bg = palette.background.weak.color;
    let hover_bg = palette.background.strong.color;
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => hover_bg,
            _ => base_bg,
        })),
        text_color: palette.background.base.text,
        border: iced::Border {
            radius: iced::border::Radius::new(4.0),
            ..iced::Border::default()
        },
        ..button::Style::default()
    }
}

/// Style for the active (selected) tab button — uses the primary accent colour
/// so it stands out clearly from inactive tabs.
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

/// Style for the small "×" close button — transparent background, subtle text.
fn close_btn_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => {
                // Slightly visible tint on hover.
                Color {
                    a: 0.15,
                    ..palette.danger.base.color
                }
            }
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

/// Style for the "+" add-tab button.
fn add_tab_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => {
                palette.background.strong.color
            }
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

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Renders the current `FerriteBrowser` state.
///
/// Layout:
///   ┌─────────────────────────────────────────┐
///   │  [Tab 0] [×]  [Tab 1] [×]  …  [+]      │  ← tab bar (background.weak)
///   ├─────────────────────────────────────────┤
///   │                                         │
///   │         Tab N content                   │  ← content area
///   │                                         │
///   └─────────────────────────────────────────┘
pub fn view(state: &FerriteBrowser) -> Element<'_, FerriteBrowserMessage> {
    // Build the tab strip: one (label + ×) group per tab, then "+".
    let mut tab_buttons: Vec<Element<FerriteBrowserMessage>> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == state.active_tab;

            // Label button — selects the tab on press.
            let label_btn = button(text(label.as_str()))
                .padding([4, 10])
                .style(if is_active {
                    tab_active_style
                } else {
                    tab_inactive_style
                })
                .on_press(FerriteBrowserMessage::SelectTab(i));

            // Close button — only enabled when there is more than one tab.
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

    // "+" button at the right end of the tab strip.
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

    // Address bar row.
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

    // Content area — placeholder until Servo WebView is wired in.
    let content = container(
        text(format!("Tab {} content", state.active_tab))
            .size(18)
            .center(),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center(Length::Fill);

    column![tab_bar, addr_bar, content].into()
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

/// Launches the Ferrite Browser UI.
///
/// Blocks until the window is closed.  Returns `iced::Result` so the caller
/// can propagate any startup error.
pub fn launch() -> iced::Result {
    iced::application("Ferrite Browser", update, view)
        .window_size(Size::new(1280.0, 800.0))
        .centered()
        .theme(|_state| Theme::Dark)
        .run()
}
