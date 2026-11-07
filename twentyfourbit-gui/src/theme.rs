//! Visual identity.
//!
//! The target is the macOS native look rather than a themed toolkit: system
//! greys, one system accent, SF as the Latin face, and hierarchy carried by type
//! and spacing instead of borders and boxes.
//!
//! The rules encoded here:
//!
//! * **System greys, not invented blacks.** macOS elevates with lightness
//!   (#1C1C1E → #2C2C2E → #3A3A3C), which is why a native window has depth
//!   without shadows.
//! * **Colour comes from the artwork, not the chrome.** The interface is
//!   monochrome; the only saturated pixels are album covers. Any tint in the
//!   chrome competes with them and makes a music app look like a dashboard.
//! * **One accent, and it is white.** White is the strongest signal available
//!   on a dark surface, so "active" needs nothing else.
//! * **Hairlines.** Separators are 1px at ~8% white, never a visible border.
//! * **Type does the work.** Four sizes with clear jobs; colour is used only for
//!   emphasis that survives a squint test.

use egui::{Color32, CornerRadius, Stroke, Visuals};

// ---------------------------------------------------------------- surfaces

/// Window background ("system background" in dark mode).
pub const BG: Color32 = Color32::from_rgb(28, 28, 30);
/// Sidebar and transport — one step up from the content plane.
pub const PANEL: Color32 = Color32::from_rgb(36, 36, 38);
/// Cards and inputs ("secondary background").
pub const CARD: Color32 = Color32::from_rgb(44, 44, 46);
/// Rows and buttons under the pointer.
pub const CARD_HOVER: Color32 = Color32::from_rgb(58, 58, 60);
/// Hairline separators (~7% white).
pub const BORDER: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 255);
/// Hairline under the pointer.
pub const BORDER_STRONG: Color32 = Color32::from_rgb(78, 78, 82);

// -------------------------------------------------------------------- text

/// Primary label.
pub const TEXT: Color32 = Color32::from_rgb(255, 255, 255);
/// Secondary label.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(163, 163, 168);
/// Tertiary label.
pub const TEXT_FAINT: Color32 = Color32::from_rgb(138, 138, 143);

// ------------------------------------------------------------------ accent

/// The accent is plain white: on a dark surface it is the strongest signal
/// available, and it keeps the chrome free of a second hue.
pub const ACCENT: Color32 = Color32::from_rgb(245, 245, 247);
/// Glyph colour on an accent fill.
pub const ON_ACCENT: Color32 = Color32::from_rgb(18, 18, 20);
/// Accent under the pointer.
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(255, 255, 255);
/// Selection: a whisper of white rather than a colour.
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(48, 48, 51);
/// Destructive actions, used only on the control that destroys something.
pub const DANGER: Color32 = Color32::from_rgb(214, 118, 118);

// ------------------------------------------------------------------- scale

/// Page titles.
pub const T_TITLE: f32 = 22.0;
/// Section headers.
pub const T_SECTION: f32 = 13.0;
/// Item titles in lists.
pub const T_ITEM: f32 = 14.0;
/// Body text and buttons.
pub const T_BODY: f32 = 13.0;
/// Metadata under a title.
pub const T_META: f32 = 11.5;
/// Hints, counters, timestamps.
pub const T_FAINT: f32 = 10.5;

/// Outer page gutter.
pub const GUTTER: f32 = 28.0;
/// Gap between cards.
pub const GAP: f32 = 8.0;
/// Card corner radius (macOS "large" rounding).
pub const RADIUS: f32 = 10.0;
/// Small radius for covers, chips and rows.
pub const RADIUS_SM: f32 = 7.0;

/// Height of the custom window header (room for the traffic lights).
pub const HEADER_H: f32 = 38.0;
/// Sidebar width.
pub const SIDEBAR_W: f32 = 208.0;
/// Transport bar height.
pub const TRANSPORT_H: f32 = 96.0;
/// Status strip height.
pub const STATUS_H: f32 = 26.0;

/// Apply the palette and metrics to a context.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = Color32::from_rgb(20, 20, 22);
    visuals.faint_bg_color = CARD;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, TEXT_MUTED);
    visuals.hyperlink_color = TEXT;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(12);

    let r = CornerRadius::same(RADIUS_SM as u8);

    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.weak_bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.noninteractive.corner_radius = r;
    visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;

    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = r;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;

    visuals.widgets.hovered.bg_fill = CARD_HOVER;
    visuals.widgets.hovered.weak_bg_fill = CARD_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.corner_radius = r;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;

    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.active.corner_radius = r;
    visuals.widgets.active.bg_stroke = Stroke::NONE;

    visuals.widgets.open.bg_fill = CARD_HOVER;
    visuals.widgets.open.weak_bg_fill = CARD_HOVER;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.corner_radius = r;
    visuals.widgets.open.bg_stroke = Stroke::NONE;

    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(GAP, GAP);
        style.spacing.button_padding = egui::vec2(13.0, 6.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.scroll.bar_width = 7.0;
        style.animation_time = 0.12;

        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(T_TITLE, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(T_BODY, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(T_BODY, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(T_FAINT, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Paint the window backdrop.
///
/// Native windows are a flat fill; a gradient would read as decoration, so this
/// exists only to guarantee that no part of the window is left unpainted when a
/// panel does not cover it.
pub fn paint_backdrop(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 0.0, BG);
}

/// Install the fonts: SF (or the platform equivalent) for Latin, and a CJK face
/// behind it for Chinese.
///
/// Returns the CJK family name, or `None` when no CJK font was found (in which
/// case the caller warns rather than silently drawing tofu).
pub fn install_fonts(ctx: &egui::Context) -> Option<String> {
    let mut fonts = egui::FontDefinitions::default();

    // Latin first: this is what makes the chrome look native.
    if let Some(ui_font) = crate::fonts::find_ui_font() {
        if let Some(bytes) = crate::fonts::load_bytes(&ui_font) {
            let mut data = egui::FontData::from_owned((*bytes).clone());
            data.index = ui_font.index;
            fonts
                .font_data
                .insert("ui".to_owned(), std::sync::Arc::new(data));

            // Put the system face *ahead* of egui's bundled one so Latin text
            // renders in SF.
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .insert(0, "ui".to_owned());
            }
        }
    }

    let cjk = crate::fonts::find_cjk_font();
    if let Some(font) = &cjk {
        if let Some(bytes) = crate::fonts::load_bytes(font) {
            let mut data = egui::FontData::from_owned((*bytes).clone());
            data.index = font.index;
            fonts
                .font_data
                .insert("cjk".to_owned(), std::sync::Arc::new(data));

            // Appended, so Latin keeps the system face and Chinese resolves here.
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("cjk".to_owned());
            }
        }
    }

    ctx.set_fonts(fonts);
    cjk.map(|f| f.family)
}
