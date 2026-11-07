//! Generate the application icon.
//!
//! Drawn rather than hand-authored so it is reproducible: a rounded dark tile
//! with an equaliser motif in the app's accent colour.

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/icon.png".to_owned());
    let size = 1024u32;
    let radius = 220.0f32;
    let accent = [96u8, 200, 160];

    let mut img = image::RgbaImage::new(size, size);

    for y in 0..size {
        for x in 0..size {
            if !inside_rounded(x as f32, y as f32, size as f32, radius) {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                continue;
            }
            let t = y as f32 / size as f32;
            let r = (18.0 + 6.0 * (1.0 - t)) as u8;
            let g = (20.0 + 8.0 * (1.0 - t)) as u8;
            let b = (26.0 + 10.0 * (1.0 - t)) as u8;
            img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }

    let bars = 5u32;
    let bar_w = 74u32;
    let gap = 46u32;
    let total_w = bars * bar_w + (bars - 1) * gap;
    let start_x = (size - total_w) / 2;
    let center_y = size / 2;
    let heights = [260u32, 420, 560, 360, 220];

    for (i, h) in heights.iter().enumerate() {
        let bx = start_x + i as u32 * (bar_w + gap);
        let top = center_y - h / 2;
        let bottom = center_y + h / 2;
        let half = bar_w as f32 / 2.0;
        let cap_y_top = top as f32 + half;
        let cap_y_bottom = bottom as f32 - half;
        let cx = bx as f32 + half;

        for y in top..bottom {
            for x in bx..bx + bar_w {
                if (y as f32) < cap_y_top {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cap_y_top;
                    if (dx * dx + dy * dy).sqrt() > half {
                        continue;
                    }
                } else if (y as f32) >= cap_y_bottom {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cap_y_bottom;
                    if (dx * dx + dy * dy).sqrt() > half {
                        continue;
                    }
                }
                img.put_pixel(x, y, image::Rgba([accent[0], accent[1], accent[2], 255]));
            }
        }
    }

    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    img.save(&path).unwrap();
    println!("wrote {path} ({size}x{size})");
}

fn inside_rounded(x: f32, y: f32, size: f32, radius: f32) -> bool {
    let (cx, cy) = (radius, radius);
    let (dx, dy) = (size - radius, size - radius);

    let ex = if x < cx { cx } else if x > dx { dx } else { x };
    let ey = if y < cy { cy } else if y > dy { dy } else { y };
    ((x - ex).powi(2) + (y - ey).powi(2)).sqrt() <= radius
}
