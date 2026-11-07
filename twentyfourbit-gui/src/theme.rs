//! Visual identity.
//!
//! A light, paper-like surface: white where content lives, a slightly cooler
//! grey for the chrome that frames it, and near-black type. The interface has
//! no colour of its own — every saturated pixel on screen comes from album art.
//!
//! The rules encoded here:
//!
//! * **Light is a hierarchy, not a flat white.** Window and panels sit 3–5%
//!   apart in luminance; that difference is what makes a layout read as layers
//!   instead of one sheet.
//! * **One accent, and it is ink.** Near-black is the strongest signal available
//!   on paper, so "active" needs no hue at all.
//! * **Hairlines.** Separators are 1px at ~10% black, never a drawn border.
//! * **Type does the work.** Four sizes with clear jobs; weight and spacing
//!   carry hierarchy before colour does.

use egui::{Color32, CornerRadius, Stroke, Visuals};

// ---------------------------------------------------------------- surfaces

/// Content plane — paper.
pub const BG: Color32 = Color32::from_rgb(255, 255, 255);
/// Chrome: sidebar, header, transport. A hair cooler than the content plane.
pub const PANEL: Color32 = Color32::from_rgb(240, 240, 244);
/// Inputs, rows and grouped containers sitting on the content plane.
pub const CARD: Color32 = Color32::from_rgb(240, 240, 244);
/// The same, under the pointer.
pub const CARD_HOVER: Color32 = Color32::from_rgb(230, 230, 235);
/// Hairline separators (~10% black).
pub const BORDER: Color32 = Color32::from_rgb(219, 219, 224);
/// Hairline under the pointer.
pub const BORDER_STRONG: Color32 = Color32::from_rgb(198, 198, 204);

// -------------------------------------------------------------------- text

/// Primary label — near-black rather than pure black, which is easier on paper.
pub const TEXT: Color32 = Color32::from_rgb(29, 29, 31);
/// Secondary label.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(110, 110, 115);
/// Tertiary label.
pub const TEXT_FAINT: Color32 = Color32::from_rgb(134, 134, 139);

// ------------------------------------------------------------------ accent

/// Ink: the only accent.
pub const ACCENT: Color32 = Color32::from_rgb(29, 29, 31);
/// Glyph colour on an ink fill.
pub const ON_ACCENT: Color32 = Color32::from_rgb(255, 255, 255);
/// Ink under the pointer.
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0, 0, 0);
/// Selection: a tint of grey, never a hue.
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(222, 222, 228);
/// Destructive actions.
pub const DANGER: Color32 = Color32::from_rgb(197, 26, 34);

// ------------------------------------------------------------------- scale

/// Page titles.
pub const T_TITLE: f32 = 24.0;
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
/// Card corner radius.
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

/// A soft shadow, for the few surfaces that float above the page.
pub fn shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 1],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(18),
    }
}

/// Apply the palette and metrics to a context.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::light();

    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = Color32::WHITE;
    visuals.extreme_bg_color = Color32::from_rgb(250, 250, 252);
    visuals.faint_bg_color = CARD;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.hyperlink_color = TEXT;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_shadow = shadow();

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
/// exists only to guarantee no part of the window is left unpainted when a panel
/// does not cover it.
pub fn paint_backdrop(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 0.0, BG);
}

/// Install the fonts: SF (or the platform equivalent) for Latin, and a CJK face
/// behind it for Chinese.
pub fn install_fonts(ctx: &egui::Context) -> Option<String> {
    let mut fonts = egui::FontDefinitions::default();

    if let Some(ui_font) = crate::fonts::find_ui_font() {
        if let Some(bytes) = crate::fonts::load_bytes(&ui_font) {
            let mut data = egui::FontData::from_owned((*bytes).clone());
            data.index = ui_font.index;
            fonts
                .font_data
                .insert("ui".to_owned(), std::sync::Arc::new(data));

            // Ahead of egui's bundled face so Latin renders in SF.
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
