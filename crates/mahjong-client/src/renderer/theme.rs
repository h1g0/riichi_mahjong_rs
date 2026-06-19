//! UI テーマ
//!
//! 角丸パネル・グラデーション背景・
//! ゴールド基調のボタンなどを macroquad の即時描画で近似する。

use macroquad::prelude::*;

/// 0xRRGGBB を不透明な [`Color`] に変換する。
const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// 0xRRGGBB を不透明な [`Color`] に変換する（公開版）。
pub const fn rgb_pub(hex: u32) -> Color {
    rgb(hex)
}

/// 0xRRGGBB ＋ アルファ で半透明の [`Color`] を作る。
pub const fn rgba(hex: u32, a: f32) -> Color {
    let c = rgb(hex);
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

// ── CSS 変数に対応する配色 ───────────────────────────────────────────────
pub const FELT: Color = rgb(0x0c2218);
pub const FELT_EDGE: Color = rgb(0x060e09);
pub const BORDER: Color = rgb(0x1d4a2a);
pub const GOLD: Color = rgb(0xc9a227);
pub const GOLD_LT: Color = rgb(0xe8c84a);
pub const GOLD_DK: Color = rgb(0x9a7a1a);
pub const TEXT: Color = rgb(0xece4d2);
// 元デザインの #7a9880 は暗背景で淡すぎるため、雰囲気を保ちつつ明度を上げる。
pub const TEXT_DIM: Color = rgb(0xa3bcab);
pub const TEXT_BR: Color = rgb(0xf5f0e0);
pub const RED: Color = rgb(0xcc3333);
pub const RED_LT: Color = rgb(0xe84444);

/// セットアップ／終了画面の背景中心色。
pub const SETUP_BG_INNER: Color = rgb(0x102a1e);

/// パネル背景（不透明化した CSS の `--panel`）。
pub const PANEL_BG: Color = rgb(0x050e08);
/// パネル枠（CSS の `--pborder` 相当）。
pub const PANEL_BORDER: Color = rgba(0xc9a227, 0.28);

/// 2 色を `t`(0..1) で線形補間する。
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// 角丸の塗りつぶし矩形。半透明色を渡すと角で僅かに重なるため、原則不透明色で使う。
pub fn draw_rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    // 中央の縦帯（全高）＋ 左右の帯（角を除く高さ）で本体を塗る。
    draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    draw_rectangle(x, y + r, r, h - 2.0 * r, color);
    draw_rectangle(x + w - r, y + r, r, h - 2.0 * r, color);
    // 四隅
    draw_circle(x + r, y + r, r, color);
    draw_circle(x + w - r, y + r, r, color);
    draw_circle(x + r, y + h - r, r, color);
    draw_circle(x + w - r, y + h - r, r, color);
}

/// 中心 (cx, cy)・半径 r の円弧（a0→a1 ラジアン）を線分で描く。
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

/// 角丸の枠線。角は四分円弧で描き、辺は直線で結ぶ。
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
    let ar = r - t / 2.0; // ストロークの中心に合わせて少し内側
    // 上下左右の辺
    draw_rectangle(x + r, y, w - 2.0 * r, t, color);
    draw_rectangle(x + r, y + h - t, w - 2.0 * r, t, color);
    draw_rectangle(x, y + r, t, h - 2.0 * r, color);
    draw_rectangle(x + w - t, y + r, t, h - 2.0 * r, color);
    // 四隅の四分円弧
    draw_arc(x + r, y + r, ar, PI, PI * 1.5, t, color); // 左上
    draw_arc(x + w - r, y + r, ar, PI * 1.5, PI * 2.0, t, color); // 右上
    draw_arc(x + w - r, y + h - r, ar, 0.0, PI * 0.5, t, color); // 右下
    draw_arc(x + r, y + h - r, ar, PI * 0.5, PI, t, color); // 左下
}

/// 角丸パネル（塗り＋枠）。
pub fn draw_panel(x: f32, y: f32, w: f32, h: f32, radius: f32, fill: Color, border: Color) {
    draw_rounded_rect(x, y, w, h, radius, fill);
    draw_rounded_rect_lines(x, y, w, h, radius, 1.5, border);
}

/// 縦方向のグラデーション矩形（上 → 下）。横帯を重ねて近似する。
pub fn draw_vgradient_rect(x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
    let steps = 24;
    let step_h = h / steps as f32;
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let c = lerp_color(top, bottom, t);
        // 帯同士の継ぎ目を消すため僅かに重ねる
        draw_rectangle(x, y + i as f32 * step_h, w, step_h + 1.0, c);
    }
}

/// 角丸の縦グラデーションボタン背景＋枠。
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
    // 角丸マスク代わりに、まず枠色で角丸ベースを描いてからグラデーションを内側に重ねる。
    draw_rounded_rect(x, y, w, h, radius, bottom);
    // 中央部分にグラデーション（角の丸みは bottom 色のベースで隠れる）
    let inset = radius.min(w / 2.0).min(h / 2.0);
    draw_vgradient_rect(x + inset, y, w - 2.0 * inset, h, top, bottom);
    draw_vgradient_rect(x, y + inset, w, h - 2.0 * inset, top, bottom);
    draw_rounded_rect_lines(x, y, w, h, radius, border_thickness, border);
}

/// 楕円の放射状グラデーション背景を描く（中心 → 周縁へ暗くなる）。
///
/// `cx, cy` は中心、`rx, ry` は最大半径。中心色 `inner` から周縁色 `outer` へ。
/// まず周縁色で全面を塗ってから、内側に向けて楕円を重ねて段階的に明るくする。
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
        // 外側(大きい楕円, outer 寄り)から内側(小さい楕円, inner 寄り)へ
        let t = i as f32 / (steps - 1) as f32; // 0=外, 1=内
        let scale = 1.0 - t;
        let c = lerp_color(outer, inner, t);
        draw_ellipse(cx, cy, rx * scale, ry * scale, 0.0, c);
    }
}

/// 視認性向上のためのフォント一律拡大率。
///
/// 元デザインの px 値は暗い盤面では小さく感じられるため少し大きく描く。
/// レイアウト計測（[`measure_scaled`]）も同じ係数を通すため、拡大してもずれない。
const FONT_SCALE: f32 = 1.2;

/// 基準サイズを実際の描画サイズへ変換する。
pub fn scaled_size(base: u16) -> u16 {
    (base as f32 * FONT_SCALE).round() as u16
}

/// 拡大後のサイズで文字寸法を測る（手動レイアウト用）。
pub fn measure_scaled(font: Option<&Font>, text: &str, base: u16) -> TextDimensions {
    measure_text(text, font, scaled_size(base), 1.0)
}

/// 影＋擬似ボールドでテキストを描く（拡大は行わない内部関数）。
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
    // 影（コントラスト補強）
    draw(Color::new(0.0, 0.0, 0.0, 0.55), 1.0, 1.0);
    // 本体（横方向に僅かにずらした二重描画で太く見せる）
    draw(color, 0.0, 0.0);
    draw(color, 0.55, 0.0);
}

/// 視認性を高めてテキストを描画する。`x` は左端、`y` はベースライン。
///
/// 細い書体でも読めるよう、暗い影でコントラストを補強し、横に僅かずらした
/// 二重描画で擬似的に太字化する。サイズは [`FONT_SCALE`] で拡大される。
pub fn draw_text(font: Option<&Font>, text: &str, x: f32, y: f32, base_size: u16, color: Color) {
    draw_text_raw(font, text, x, y, scaled_size(base_size), color);
}

/// 中央寄せでテキストを描く。`y` はベースライン。
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
