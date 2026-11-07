//! Vector icons drawn with the painter.
//!
//! egui has no icon font, and pulling in one just for a play button would drag
//! a whole font file and a glyph-lookup layer behind it. These are drawn from
//! primitives instead: they stay crisp at any DPI, follow the theme colour, and
//! cost nothing at build time.

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

/// The icons this client needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Previous,
    Next,
    Volume,
    Search,
    Music,
    Library,
    Gear,
    Download,
    Close,
    /// Text lines, for the lyrics toggle.
    Lyrics,
}

/// Paint `icon` centred in `rect`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    // Everything is laid out on a notional 24x24 grid, then scaled.
    let s = rect.width().min(rect.height()) / 24.0;
    let p = |x: f32, y: f32| Pos2::new(c.x + (x - 12.0) * s, c.y + (y - 12.0) * s);
    let stroke = Stroke::new(2.0 * s, color);
    let solid = |pts: Vec<Pos2>| egui::Shape::convex_polygon(pts, color, Stroke::NONE);

    match icon {
        Icon::Play => {
            painter.add(solid(vec![p(9.0, 6.5), p(18.5, 12.0), p(9.0, 17.5)]));
        }
        Icon::Pause => {
            for x in [8.5, 14.0] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, 6.5), p(x + 3.0, 17.5)),
                    1.5 * s,
                    color,
                );
            }
        }
        Icon::Previous | Icon::Next => {
            let flip = icon == Icon::Previous;
            let (a, b, c2, bar) = if flip {
                (18.0, 12.5, 7.0, 18.5)
            } else {
                (6.0, 11.5, 17.0, 5.5)
            };
            painter.add(solid(vec![p(a, 6.5), p(b, 12.0), p(a, 17.5)]));
            painter.add(solid(vec![p(c2, 6.5), p(b, 12.0), p(c2, 17.5)]));
            painter.rect_filled(
                Rect::from_min_max(p(bar, 6.5), p(bar + 1.8, 17.5)),
                0.9 * s,
                color,
            );
        }
        Icon::Volume => {
            painter.add(solid(vec![p(4.5, 9.5), p(8.5, 9.5), p(13.0, 5.5), p(13.0, 18.5), p(8.5, 14.5), p(4.5, 14.5)]));
            painter.circle_stroke(p(13.0, 12.0), 4.0 * s, Stroke::new(1.8 * s, color));
            painter.circle_stroke(p(13.0, 12.0), 7.0 * s, Stroke::new(1.8 * s, color));
        }
        Icon::Search => {
            painter.circle_stroke(p(10.5, 10.5), 5.5 * s, stroke);
            painter.line_segment([p(15.0, 15.0), p(19.5, 19.5)], Stroke::new(2.2 * s, color));
        }
        Icon::Music => {
            // A quaver: stem, flag, note head.
            painter.rect_filled(Rect::from_min_max(p(14.0, 5.0), p(16.0, 15.0)), 1.0 * s, color);
            painter.add(solid(vec![p(16.0, 5.0), p(20.0, 6.5), p(20.0, 9.5), p(16.0, 8.0)]));
            painter.circle_filled(p(12.5, 16.0), 3.2 * s, color);
        }
        Icon::Library => {
            for (i, x) in [5.0f32, 9.5, 14.0].iter().enumerate() {
                let top = 5.5 + i as f32 * 0.6;
                painter.rect_filled(
                    Rect::from_min_max(p(*x, top), p(x + 2.6, 19.0)),
                    1.2 * s,
                    color,
                );
            }
            // One leaning record spine.
            painter.add(solid(vec![p(18.0, 6.5), p(20.6, 6.0), p(21.6, 18.6), p(19.0, 19.1)]));
        }
        Icon::Gear => {
            painter.circle_stroke(p(12.0, 12.0), 3.4 * s, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = a.sin_cos();
                painter.line_segment(
                    [p(12.0 + cos * 5.6, 12.0 + sin * 5.6), p(12.0 + cos * 7.6, 12.0 + sin * 7.6)],
                    Stroke::new(2.2 * s, color),
                );
            }
        }
        Icon::Download => {
            painter.line_segment([p(12.0, 5.0), p(12.0, 15.0)], stroke);
            painter.add(solid(vec![p(7.5, 11.0), p(12.0, 16.0), p(16.5, 11.0)]));
            painter.line_segment([p(5.5, 18.5), p(18.5, 18.5)], stroke);
        }
        Icon::Lyrics => {
            // Four lines of shrinking width, like a lyric sheet.
            for (i, w) in [14.0f32, 11.0, 13.0, 8.0].iter().enumerate() {
                let y = 6.5 + i as f32 * 3.6;
                painter.rect_filled(
                    Rect::from_min_max(p(5.0, y), p(5.0 + w, y + 1.8)),
                    0.9 * s,
                    color,
                );
            }
        }
        Icon::Close => {
            painter.line_segment([p(6.5, 6.5), p(17.5, 17.5)], stroke);
            painter.line_segment([p(17.5, 6.5), p(6.5, 17.5)], stroke);
        }
    }
}

/// How an icon button should be filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// No background until hovered. For toolbar actions.
    Ghost,
    /// Always filled with the accent colour. For the primary play button.
    Accent,
    /// Filled with a surface colour. For selected/standing controls.
    Filled,
}

/// An icon-only button of a given diameter.
pub fn button(ui: &mut Ui, icon: Icon, diameter: f32, style: Style) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter();

    let (bg, fg) = match style {
        Style::Accent => (
            Some(crate::theme::ACCENT),
            if hovered {
                crate::theme::ACCENT_HOVER
            } else {
                crate::theme::ON_ACCENT
            },
        ),
        Style::Filled => (
            Some(if hovered {
                crate::theme::CARD_HOVER
            } else {
                crate::theme::CARD
            }),
            crate::theme::TEXT,
        ),
        Style::Ghost => (
            hovered.then_some(crate::theme::CARD_HOVER),
            if hovered { crate::theme::TEXT } else { crate::theme::TEXT_MUTED },
        ),
    };

    if let Some(bg) = bg {
        painter.circle_filled(rect.center(), diameter / 2.0, bg);
    }

    // The glyph occupies the middle 55% of the hit area, which keeps small
    // icon buttons from looking cramped.
    paint(
        painter,
        Rect::from_center_size(rect.center(), Vec2::splat(diameter * 0.55)),
        icon,
        fg,
    );

    response
}

/// An icon button at an exact rectangle, for hand-painted rows.
pub fn button_at(ui: &mut Ui, rect: Rect, icon: Icon, style: Style) -> Response {
    let response = ui.interact(rect, ui.scope_id().with(("icon", icon as u8, rect.min.x as i32)), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter();

    let (bg, fg) = match style {
        Style::Accent => (
            Some(crate::theme::ACCENT),
            if hovered { crate::theme::ACCENT_HOVER } else { crate::theme::ON_ACCENT },
        ),
        Style::Filled => (
            Some(if hovered {
                crate::theme::ACCENT
            } else {
                crate::theme::CARD_HOVER
            }),
            if hovered {
                crate::theme::ON_ACCENT
            } else {
                crate::theme::TEXT
            },
        ),
        Style::Ghost => (
            hovered.then_some(crate::theme::CARD_HOVER),
            if hovered { crate::theme::TEXT } else { crate::theme::TEXT_MUTED },
        ),
    };

    if let Some(bg) = bg {
        painter.circle_filled(rect.center(), rect.width().min(rect.height()) / 2.0, bg);
    }
    paint(
        painter,
        Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.5)),
        icon,
        fg,
    );
    response
}

/// A thin rounded track with a filled portion and a draggable thumb.
///
/// Returns the response together with a requested position when the user
/// dragged or clicked, so the caller decides whether to seek.
pub fn seek_bar(ui: &mut Ui, value: f32, height: f32) -> (Response, Option<f32>) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let painter = ui.painter();

    let track = Rect::from_min_max(
        Pos2::new(rect.left(), rect.center().y - 2.0),
        Pos2::new(rect.right(), rect.center().y + 2.0),
    );
    let base = if response.hovered() {
        crate::theme::BORDER_STRONG
    } else {
        crate::theme::BORDER
    };
    painter.rect_filled(track, 2.0, base);

    let fraction = value.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let filled = Rect::from_min_max(
            track.min,
            Pos2::new(track.left() + track.width() * fraction, track.max.y),
        );
        painter.rect_filled(filled, 2.0, crate::theme::ACCENT);
    }

    // Thumb, so the bar reads as draggable rather than decorative.
    let thumb_x = track.left() + track.width() * fraction;
    if response.hovered() || response.dragged() {
        painter.circle_filled(
            Pos2::new(thumb_x, track.center().y),
            6.0,
            crate::theme::TEXT,
        );
    }

    let requested = if response.dragged() || response.clicked() {
        response.interact_pointer_pos().map(|pos| {
            ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0)
        })
    } else {
        None
    };

    (response, requested)
}

/// Draw a hairline separator in the current theme.
pub fn separator(ui: &mut Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, crate::theme::BORDER);
}

/// A rounded rectangle with an optional 1px border.
pub fn card(painter: &egui::Painter, rect: Rect, radius: f32, fill: Color32, border: Option<Color32>) {
    painter.rect_filled(rect, radius, fill);
    if let Some(border) = border {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
    }
}
