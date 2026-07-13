//! Tile index labels for the English UI.
//!
//! Small colored digits/letters are baked into the tiles' corners so
//! players who cannot read kanji can identify them. Rather than
//! overlaying text every frame, the labels are composited into the
//! source [`Image`]s on the CPU at startup;
//! [`TileTextures`](super::TileTextures) keeps both the labeled and
//! plain sets and switches by language.
//!
//! With no GPU or font-atlas dependency, labels rotate with the tiles
//! and headless unit tests are possible.

use macroquad::prelude::{Color, Image, WHITE};
use mahjong_core::tile::{Tile, TileType};

use super::theme;

/// Characters digit color (red).
pub(super) const LABEL_RED: Color = theme::rgb_pub(0xc22a2a);
/// Circles digit color (blue).
pub(super) const LABEL_BLUE: Color = theme::rgb_pub(0x1d55b0);
/// Bamboos digit color (green).
pub(super) const LABEL_GREEN: Color = theme::rgb_pub(0x1e7d32);
/// Label color for winds and the White dragon (black).
pub(super) const LABEL_BLACK: Color = theme::rgb_pub(0x202020);

/// The label character and color for each tile kind.
///
/// Suit tiles use digits (characters red, circles blue, bamboos green);
/// winds use E/S/W/N in black; dragons use the traditional export-set
/// letters P (White), F (Green), C (Red), with F green and C red to
/// match the tiles.
pub(super) fn tile_index_label(tile_type: TileType) -> (&'static str, Color) {
    const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
    match tile_type {
        i @ Tile::M1..=Tile::M9 => (DIGITS[(i - Tile::M1) as usize], LABEL_RED),
        i @ Tile::P1..=Tile::P9 => (DIGITS[(i - Tile::P1) as usize], LABEL_BLUE),
        i @ Tile::S1..=Tile::S9 => (DIGITS[(i - Tile::S1) as usize], LABEL_GREEN),
        Tile::Z1 => ("E", LABEL_BLACK),
        Tile::Z2 => ("S", LABEL_BLACK),
        Tile::Z3 => ("W", LABEL_BLACK),
        Tile::Z4 => ("N", LABEL_BLACK),
        Tile::Z5 => ("P", LABEL_BLACK),
        Tile::Z6 => ("F", LABEL_GREEN),
        Tile::Z7 => ("C", LABEL_RED),
        _ => ("", LABEL_BLACK),
    }
}

/// Bakes an index label (white rounded badge plus colored character)
/// into the tile image's top-right corner.
///
/// Badge and glyph scale with the image, so the layout survives source
/// resolution changes.
pub(super) fn bake_label(img: &mut Image, label: &str, color: Color, font: &fontdue::Font) {
    let Some(ch) = label.chars().next() else {
        return;
    };

    let tile_w = img.width as f32;

    // Badge placement: ~47px badge with ~4px margin on a 111px tile.
    let badge = (tile_w * 0.42).round();
    let margin = (tile_w * 0.04).round();
    let bx = tile_w - margin - badge;
    let by = margin;
    let radius = badge * 0.24;

    // Paint the frame in the label color, then the inside in white,
    // leaving a colored ring.
    let border = 2.5;
    let ring = Color::new(color.r, color.g, color.b, 0.8);
    fill_rounded_rect(img, bx, by, badge, badge, radius, ring);
    fill_rounded_rect(
        img,
        bx + border,
        by + border,
        badge - 2.0 * border,
        badge - 2.0 * border,
        (radius - border).max(1.0),
        Color::new(WHITE.r, WHITE.g, WHITE.b, 0.92),
    );

    // Center the glyph on the badge, dilating its coverage for faux
    // bold: strokes thicken more evenly than a 1px double draw and
    // survive downscaling better.
    let px_size = badge * 0.92;
    let (metrics, bitmap) = font.rasterize(ch, px_size);
    let (bitmap, glyph_w, glyph_h) = dilate(&bitmap, metrics.width, metrics.height, 2, 2);
    let gx = (bx + (badge - glyph_w as f32) / 2.0).round() as i32;
    let gy = (by + (badge - glyph_h as f32) / 2.0).round() as i32;
    blit_glyph(img, gx, gy, glyph_w, glyph_h, &bitmap, color);
}

/// Dilates glyph coverage by `dx` px rightward and `dy` px downward,
/// returning the thickened bitmap and its dimensions.
fn dilate(bitmap: &[u8], w: usize, h: usize, dx: usize, dy: usize) -> (Vec<u8>, usize, usize) {
    let (nw, nh) = (w + dx, h + dy);
    let mut out = vec![0u8; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            let mut cov = 0u8;
            for oy in 0..=dy.min(y) {
                for ox in 0..=dx.min(x) {
                    let (sx, sy) = (x - ox, y - oy);
                    if sx < w && sy < h {
                        cov = cov.max(bitmap[sy * w + sx]);
                    }
                }
            }
            out[y * nw + x] = cov;
        }
    }
    (out, nw, nh)
}

/// Alpha-composites a rasterized glyph onto the image.
fn blit_glyph(img: &mut Image, gx: i32, gy: i32, w: usize, h: usize, bitmap: &[u8], color: Color) {
    for row in 0..h {
        for col in 0..w {
            let coverage = bitmap[row * w + col] as f32 / 255.0;
            blend_pixel(img, gx + col as i32, gy + row as i32, color, coverage);
        }
    }
}

/// Fills a rounded rectangle with SDF-based antialiased coverage.
fn fill_rounded_rect(img: &mut Image, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let (hx, hy) = (w / 2.0 - radius, h / 2.0 - radius);
    let x0 = x.floor().max(0.0) as i32;
    let y0 = y.floor().max(0.0) as i32;
    let x1 = (x + w).ceil() as i32;
    let y1 = (y + h).ceil() as i32;
    for py in y0..=y1 {
        for px in x0..=x1 {
            // Signed distance of the rounded rect at the pixel center.
            let qx = (px as f32 + 0.5 - cx).abs() - hx;
            let qy = (py as f32 + 0.5 - cy).abs() - hy;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let d = outside + qx.max(qy).min(0.0) - radius;
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            blend_pixel(img, px, py, color, coverage);
        }
    }
}

/// Alpha-composites one pixel.
///
/// The blend factor is multiplied by the destination alpha, so labels
/// never bleed outside the tile's rounded silhouette; the output alpha
/// keeps its original value.
fn blend_pixel(img: &mut Image, x: i32, y: i32, color: Color, coverage: f32) {
    if x < 0 || y < 0 || x >= img.width as i32 || y >= img.height as i32 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    let dst = img.get_pixel(x, y);
    let a = coverage * color.a * dst.a;
    if a <= 0.0 {
        return;
    }
    img.set_pixel(
        x,
        y,
        Color::new(
            dst.r + (color.r - dst.r) * a,
            dst.g + (color.g - dst.g) * a,
            dst.b + (color.b - dst.b) * a,
            dst.a,
        ),
    );
}
