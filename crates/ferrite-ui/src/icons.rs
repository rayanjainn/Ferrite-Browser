// ferrite-ui — hand-authored SVG icon set and the one rendering helper every
// view call site goes through (C1 design-system charter).
//
// Every icon is a standalone, hand-authored `.svg` file under
// `assets/icons/` (simple geometric shapes — outlined/stroke-based, a single
// consistent 24x24 viewBox and stroke-width across the set — never copied
// from a named commercial/open-source icon pack). Each is embedded at
// compile time via `include_bytes!`, so rendering never depends on the
// process's current working directory (unlike `svg::Handle::from_path`,
// which `cargo run`'s CWD could break depending on how the app is invoked).
//
// `icon()` is the single path from an [`Icon`] variant to a rendered, tinted
// `iced_widget::svg::Svg` element. Every place in `lib.rs`'s `view()`/
// `view_agent_sidebar()` that wants an icon goes through it, rather than
// constructing `Svg::new(...)` ad hoc per call site — this is what keeps
// sizing/color consistent across the whole chrome.
//
// Colour is applied by `iced_widget::svg::Style::color` (confirmed present
// on the pinned iced_widget 0.13.4 — "Useful for coloring a symbolic icon");
// this is why every source `.svg` can be authored in plain black (`#000000`)
// — the actual on-screen tint is supplied by the caller at render time, the
// same way this crate's existing buttons already recolor via
// `button::Style`/`text::color` rather than baking color into markup.

use iced::{Color, Element, Length, Theme};
use iced_widget::svg::{self, Svg};

/// Every icon this crate renders. Add a new `.svg` under `assets/icons/`
/// and a matching arm in [`icon_bytes`] to extend the set — `C2`'s agent
/// panel redesign should reuse this enum and `icon()` rather than starting
/// a second icon-rendering path (see `docs/handoffs/c01.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Back,
    Forward,
    Reload,
    /// Tab/dialog close (also used as "stop loading", matching the common
    /// browser convention of an X for interrupting an in-progress load).
    Close,
    Add,
    /// Agent-loop stop (a filled square, media-player convention) — kept
    /// distinct from `Close`, which is reserved for "close"/"stop loading".
    Stop,
    Play,
    Approve,
    Reject,
    Warning,
    Audit,
    Console,
    Agent,
    /// External-link glyph used by the consent panel's "Contacted <origin>"
    /// rows.
    Origin,
    /// C2: agent step-history glyphs, one per `AgentAction` group (see
    /// `lib.rs`'s `icon_for_action`) — grouped by the same action-class
    /// boundaries `ferrite_core::Primitive`'s own taxonomy uses
    /// (navigate/read/interact/write/execute/...) rather than one icon per
    /// raw primitive, so the set stays small and each glyph stays
    /// recognizable at the sidebar's small render size.
    Navigate,
    Read,
    Click,
    Write,
    /// Scroll/wait/screenshot — grouped under one "system action" glyph;
    /// none of the three is common or visually distinct enough on its own
    /// to justify a dedicated icon yet. Split out the moment one needs to
    /// read differently from the others.
    Activity,
    Download,
    Clipboard,
}

// A bookmark/star icon and a settings/gear icon were drawn for this charter
// (C3 will want both — a bookmark feature and a settings surface — and
// they're cheap to add) but deliberately left out of this enum: rustc's
// `dead_code` lint flags an enum variant as unused per the crate's actual
// *lib*-target compilation, which never includes `#[cfg(test)]` code no
// matter how thoroughly a test module constructs it — so an icon with no
// real (non-test) call site cannot be pre-added here without either
// violating `just check`'s `-D warnings` gate or reaching for
// `#[allow(dead_code)]`, which `CLAUDE.md`'s "no dead code" invariant rules
// out. Add the variant (and its `.svg`, and a real call site) together when
// C2/C3 actually needs one, rather than pre-declaring it here unused.

/// The embedded SVG bytes for `kind` — a pure mapping, kept separate from
/// [`icon`] so it's testable without spinning up Iced at all (mirrors this
/// crate's existing pure-function test style in `lib.rs`, e.g.
/// `consent_items`/`describe_scope`).
pub fn icon_bytes(kind: Icon) -> &'static [u8] {
    match kind {
        Icon::Back => include_bytes!("../assets/icons/back.svg"),
        Icon::Forward => include_bytes!("../assets/icons/forward.svg"),
        Icon::Reload => include_bytes!("../assets/icons/reload.svg"),
        Icon::Close => include_bytes!("../assets/icons/close.svg"),
        Icon::Add => include_bytes!("../assets/icons/add.svg"),
        Icon::Stop => include_bytes!("../assets/icons/stop.svg"),
        Icon::Play => include_bytes!("../assets/icons/play.svg"),
        Icon::Approve => include_bytes!("../assets/icons/approve.svg"),
        Icon::Reject => include_bytes!("../assets/icons/reject.svg"),
        Icon::Warning => include_bytes!("../assets/icons/warning.svg"),
        Icon::Audit => include_bytes!("../assets/icons/audit.svg"),
        Icon::Console => include_bytes!("../assets/icons/console.svg"),
        Icon::Agent => include_bytes!("../assets/icons/agent.svg"),
        Icon::Origin => include_bytes!("../assets/icons/origin.svg"),
        Icon::Navigate => include_bytes!("../assets/icons/navigate.svg"),
        Icon::Read => include_bytes!("../assets/icons/read.svg"),
        Icon::Click => include_bytes!("../assets/icons/click.svg"),
        Icon::Write => include_bytes!("../assets/icons/write.svg"),
        Icon::Activity => include_bytes!("../assets/icons/activity.svg"),
        Icon::Download => include_bytes!("../assets/icons/download.svg"),
        Icon::Clipboard => include_bytes!("../assets/icons/clipboard.svg"),
    }
}

/// Renders `kind` as a square `size`x`size` element tinted `color` — the one
/// icon-rendering path every view call site in this crate uses, so sizing
/// and color stay consistent instead of ad hoc `svg::Svg::new(...)` calls
/// scattered across the view code. `svg::Handle::from_memory` derives its
/// cache identity from a content hash (`iced_core::svg::Handle::from_data`),
/// so calling this repeatedly across frames with the same `kind` is cheap —
/// the renderer reuses the same rasterized/tinted image rather than
/// re-decoding the SVG every tick.
pub fn icon<'a, Message: 'a>(kind: Icon, size: f32, color: Color) -> Element<'a, Message> {
    let handle = svg::Handle::from_memory(icon_bytes(kind));
    Svg::new(handle)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme: &Theme, _status: svg::Status| svg::Style { color: Some(color) })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Icon; 21] = [
        Icon::Back,
        Icon::Forward,
        Icon::Reload,
        Icon::Close,
        Icon::Add,
        Icon::Stop,
        Icon::Play,
        Icon::Approve,
        Icon::Reject,
        Icon::Warning,
        Icon::Audit,
        Icon::Console,
        Icon::Agent,
        Icon::Origin,
        Icon::Navigate,
        Icon::Read,
        Icon::Click,
        Icon::Write,
        Icon::Activity,
        Icon::Download,
        Icon::Clipboard,
    ];

    /// Every icon variant embeds real, well-formed SVG data — catches a
    /// typo'd path/an empty file long before it would otherwise only show up
    /// as a blank glyph in a build this agent cannot visually inspect.
    #[test]
    fn every_icon_variant_embeds_non_empty_well_formed_svg() {
        for kind in ALL {
            let bytes = icon_bytes(kind);
            assert!(!bytes.is_empty(), "{kind:?} has empty svg bytes");
            let text = std::str::from_utf8(bytes)
                .unwrap_or_else(|e| panic!("{kind:?}'s svg is not valid utf8: {e}"));
            assert!(
                text.contains("<svg") && text.contains("viewBox"),
                "{kind:?}'s svg is missing an <svg>/viewBox root: {text}"
            );
        }
    }

    /// Every icon shares the same 24x24 viewBox — the consistency this
    /// module's own docs promise ("a single consistent viewBox ... across
    /// the set").
    #[test]
    fn every_icon_shares_the_same_viewbox() {
        for kind in ALL {
            let text = std::str::from_utf8(icon_bytes(kind)).unwrap();
            assert!(
                text.contains(r#"viewBox="0 0 24 24""#),
                "{kind:?} does not use the shared 0 0 24 24 viewBox: {text}"
            );
        }
    }
}
