//! UI theme: rounded panels, gradient backgrounds, and gold-accented
//! buttons approximated with macroquad's immediate-mode drawing.

use macroquad::prelude::*;

/// 0xRRGGBB to an opaque [`Color`].
const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Public variant of the 0xRRGGBB conversion.
pub const fn rgb_pub(hex: u32) -> Color {
    rgb(hex)
}

/// 0xRRGGBB plus alpha to a translucent [`Color`].
pub const fn rgba(hex: u32, a: f32) -> Color {
    let c = rgb(hex);
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

// --- Palette mirroring the CSS variables ---
pub const FELT: Color = rgb(0x0c2218);
pub const FELT_EDGE: Color = rgb(0x060e09);
pub const BORDER: Color = rgb(0x1d4a2a);
pub const GOLD: Color = rgb(0xc9a227);
pub const GOLD_LT: Color = rgb(0xe8c84a);
pub const GOLD_DK: Color = rgb(0x9a7a1a);
pub const TEXT: Color = rgb(0xece4d2);
// The original #7a9880 is too pale on the dark board; brightened while
// keeping the mood.
pub const TEXT_DIM: Color = rgb(0xa3bcab);
pub const TEXT_BR: Color = rgb(0xf5f0e0);
pub const RED: Color = rgb(0xcc3333);
pub const RED_LT: Color = rgb(0xe84444);
pub const BLUE_LT: Color = rgb(0x70b7ff);

/// Background center color of the setup/end screens.
pub const SETUP_BG_INNER: Color = rgb(0x102a1e);

/// Panel background (the CSS `--panel`, made opaque).
pub const PANEL_BG: Color = rgb(0x050e08);
/// Panel border (the CSS `--pborder`).
pub const PANEL_BORDER: Color = rgba(0xc9a227, 0.28);

/// Linear interpolation between two colors by `t` (0..1).
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// Fills a quarter circle at (cx, cy) with radius r as a triangle fan,
/// spanning 90 degrees from a0.
fn fill_quarter_circle(cx: f32, cy: f32, r: f32, a0: f32, color: Color) {
    use std::f32::consts::PI;
    let a1 = a0 + PI / 2.0;
    let segs = 8;
    let center = vec2(cx, cy);
    for i in 0..segs {
        let t0 = a0 + (a1 - a0) * (i as f32 / segs as f32);
        let t1 = a0 + (a1 - a0) * ((i + 1) as f32 / segs as f32);
        draw_triangle(
            center,
            vec2(cx + r * t0.cos(), cy + r * t0.sin()),
            vec2(cx + r * t1.cos(), cy + r * t1.sin()),
            color,
        );
    }
}

/// Filled rounded rectangle.
///
/// The corners are quarter circles rather than full circles, so they
/// never overlap the body strips; translucent colors thus avoid doubled
/// alpha at the corners (which showed as faint circles).
pub fn draw_rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    use std::f32::consts::PI;
    let r = radius.min(w / 2.0).min(h / 2.0);
    // Body: a full-height center strip plus side strips that skip
    // the corners.
    draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    draw_rectangle(x, y + r, r, h - 2.0 * r, color);
    draw_rectangle(x + w - r, y + r, r, h - 2.0 * r, color);
    fill_quarter_circle(x + r, y + r, r, PI, color);
    fill_quarter_circle(x + w - r, y + r, r, PI * 1.5, color);
    fill_quarter_circle(x + w - r, y + h - r, r, 0.0, color);
    fill_quarter_circle(x + r, y + h - r, r, PI * 0.5, color);
}

/// Draws an arc at (cx, cy) with radius r from a0 to a1 radians using
/// line segments.
fn draw_arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, thickness: f32, color: Color) {
    let segs = 8;
    let mut prev = (cx + r * a0.cos(), cy + r * a0.sin());
    for i in 1..=segs {
        let t = a0 + (a1 - a0) * (i as f32 / segs as f32);
        let p = (cx + r * t.cos(), cy + r * t.sin());
        draw_line(prev.0, prev.1, p.0, p.1, thickness, color);
        prev = p;
    }
}

/// Rounded-rectangle outline: arcs at the corners, lines between.
pub fn draw_rounded_rect_lines(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    thickness: f32,
    color: Color,
) {
    use std::f32::consts::PI;
    let r = radius.min(w / 2.0).min(h / 2.0);
    let t = thickness;
    let ar = r - t / 2.0; // Inset so the stroke centers on the edge.
    draw_rectangle(x + r, y, w - 2.0 * r, t, color);
    draw_rectangle(x + r, y + h - t, w - 2.0 * r, t, color);
    draw_rectangle(x, y + r, t, h - 2.0 * r, color);
    draw_rectangle(x + w - t, y + r, t, h - 2.0 * r, color);
    draw_arc(x + r, y + r, ar, PI, PI * 1.5, t, color);
    draw_arc(x + w - r, y + r, ar, PI * 1.5, PI * 2.0, t, color);
    draw_arc(x + w - r, y + h - r, ar, 0.0, PI * 0.5, t, color);
    draw_arc(x + r, y + h - r, ar, PI * 0.5, PI, t, color);
}

/// Rounded panel: fill plus outline.
pub fn draw_panel(x: f32, y: f32, w: f32, h: f32, radius: f32, fill: Color, border: Color) {
    draw_rounded_rect(x, y, w, h, radius, fill);
    draw_rounded_rect_lines(x, y, w, h, radius, 1.5, border);
}

/// Fills a rounded shape with a vertical (top-to-bottom) gradient.
///
/// Each 1px horizontal strip gets its own interpolated color and its
/// corner inset, so the corners share the gradient and no odd-colored
/// fans remain.
pub fn draw_rounded_vgradient_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    top: Color,
    bottom: Color,
) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    let steps = h.ceil().max(1.0) as usize;
    let step_h = h / steps as f32;
    for i in 0..steps {
        let yy = y + i as f32 * step_h;
        let cy = yy + step_h / 2.0; // Strip center drives color and inset.
        let t = ((cy - y) / h).clamp(0.0, 1.0);
        let color = lerp_color(top, bottom, t);
        let dx = if cy < y + r {
            let d = (y + r) - cy;
            r - (r * r - d * d).max(0.0).sqrt()
        } else if cy > y + h - r {
            let d = cy - (y + h - r);
            r - (r * r - d * d).max(0.0).sqrt()
        } else {
            0.0
        };
        // Overlap strips slightly to hide seams.
        draw_rectangle(x + dx, yy, w - 2.0 * dx, step_h + 1.0, color);
    }
}

/// Rounded vertical-gradient button background plus outline.
#[allow(clippy::too_many_arguments)]
pub fn draw_gradient_button(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    top: Color,
    bottom: Color,
    border: Color,
    border_thickness: f32,
) {
    draw_rounded_vgradient_rect(x, y, w, h, radius, top, bottom);
    draw_rounded_rect_lines(x, y, w, h, radius, border_thickness, border);
}

/// Draws an elliptical radial-gradient background, darkening outwards.
///
/// `cx, cy` is the center, `rx, ry` the maximum radii, blending `inner`
/// to `outer`. The whole area is painted in the outer color first, then
/// progressively lighter ellipses are stacked inwards.
#[allow(clippy::too_many_arguments)]
pub fn draw_radial_bg(
    full_w: f32,
    full_h: f32,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    inner: Color,
    outer: Color,
) {
    draw_rectangle(0.0, 0.0, full_w, full_h, outer);
    let steps = 32;
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32; // 0 = outermost, 1 = innermost
        let scale = 1.0 - t;
        let c = lerp_color(outer, inner, t);
        draw_ellipse(cx, cy, rx * scale, ry * scale, 0.0, c);
    }
}

/// Uniform font scale for readability.
///
/// The original pixel sizes feel small on the dark board, so text draws
/// slightly larger; [`measure_scaled`] applies the same factor, keeping
/// layout consistent.
const FONT_SCALE: f32 = 1.2;

/// Base size to actual rendered size.
pub fn scaled_size(base: u16) -> u16 {
    (base as f32 * FONT_SCALE).round() as u16
}

/// Measures text at the scaled size, for manual layout.
pub fn measure_scaled(font: Option<&Font>, text: &str, base: u16) -> TextDimensions {
    measure_text(text, font, scaled_size(base), 1.0)
}

/// Draws text with a shadow and faux bold (no scaling; internal).
fn draw_text_raw(font: Option<&Font>, text: &str, x: f32, y: f32, fs: u16, color: Color) {
    let draw = |c: Color, dx: f32, dy: f32| {
        draw_text_ex(
            text,
            x + dx,
            y + dy,
            TextParams {
                font,
                font_size: fs,
                color: c,
                ..Default::default()
            },
        );
    };
    draw(Color::new(0.0, 0.0, 0.0, 0.55), 1.0, 1.0);
    // Faux bold: draw twice with a slight horizontal offset.
    draw(color, 0.0, 0.0);
    draw(color, 0.55, 0.0);
}

/// Draws readable text; `x` is the left edge, `y` the baseline.
///
/// A dark shadow boosts contrast and a slightly offset double draw fakes
/// bold, keeping thin faces legible; sizes scale by [`FONT_SCALE`].
pub fn draw_text(font: Option<&Font>, text: &str, x: f32, y: f32, base_size: u16, color: Color) {
    draw_text_raw(font, text, x, y, scaled_size(base_size), color);
}

/// Draws centered text; `y` is the baseline.
pub fn draw_text_centered(
    font: Option<&Font>,
    text: &str,
    center_x: f32,
    baseline_y: f32,
    base_size: u16,
    color: Color,
) {
    let fs = scaled_size(base_size);
    let dims = measure_text(text, font, fs, 1.0);
    draw_text_raw(
        font,
        text,
        center_x - dims.width / 2.0,
        baseline_y,
        fs,
        color,
    );
}
