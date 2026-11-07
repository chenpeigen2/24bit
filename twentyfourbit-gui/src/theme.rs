//! Visual theme.
//!
//! egui's default look is utilitarian grey. This is a dark, warm-neutral
//! palette chosen to sit behind album art without competing with it.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Background of the window.
pub const BG: Color32 = Color32::from_rgb(16, 16, 18);
/// Slightly raised surfaces (panels, cards).
pub const SURFACE: Color32 = Color32::from_rgb(24, 24, 28);
/// Hovered / selected surfaces.
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(34, 34, 40);
/// Primary text.
pub const TEXT: Color32 = Color32::from_rgb(236, 236, 240);
/// Secondary text (artist, album, metadata).
pub const TEXT_DIM: Color32 = Color32::from_rgb(150, 150, 160);
/// Accent used for progress, selection and primary actions.
pub const ACCENT: Color32 = Color32::from_rgb(96, 200, 160);
/// Accent, dimmed for backgrounds.
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(30, 62, 52);
/// Errors and destructive actions.
pub const DANGER: Color32 = Color32::from_rgb(224, 108, 108);

/// Apply the palette to a context.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(10, 10, 12);
    visuals.faint_bg_color = SURFACE;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;

    let rounding = CornerRadius::same(8);

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.corner_radius = rounding;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.weak_bg_fill = SURFACE;
    visuals.widgets.inactive.corner_radius = rounding;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.weak_bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.corner_radius = rounding;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);

    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.active.corner_radius = rounding;
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, ACCENT);

    visuals.window_corner_radius = CornerRadius::same(12);

    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 28.0;
    });
}

/// Register the CJK font so Chinese text actually renders.
///
/// Returns the family name used, or `None` when no CJK font was found (in which
/// case the caller shows a warning instead of silently rendering tofu).
pub fn install_fonts(ctx: &egui::Context) -> Option<String> {
    let font = crate::fonts::find_cjk_font()?;
    let bytes = crate::fonts::load_bytes(&font)?;

    let mut fonts = egui::FontDefinitions::default();

    let mut data = egui::FontData::from_owned((*bytes).clone());
    data.index = font.index;

    fonts
        .font_data
        .insert("cjk".to_owned(), std::sync::Arc::new(data));

    // Append as a *fallback* so Latin text keeps the crisper default face and
    // Chinese resolves through the CJK font.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("cjk".to_owned());
    }

    ctx.set_fonts(fonts);
    Some(font.family)
}
