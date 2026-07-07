//! 英語UI向けの牌インデックスラベル
//!
//! 漢字が読めないプレイヤーでも牌を識別できるよう、牌画像の右上に
//! 小さな色付きの数字・文字を焼き込む。描画時に毎フレーム文字を
//! 重ねるのではなく、起動時に CPU 側で牌テクスチャの元画像
//! （[`Image`]）へ合成しておく（[`TileTextures`](super::TileTextures) が
//! ラベル付き・なしの両テクスチャセットを保持し、言語設定で切り替える）。
//!
//! GPU やフォントアトラスに依存しないため、回転描画でも牌と一緒に
//! ラベルが回り、ヘッドレスの単体テストも可能。

use macroquad::prelude::{Color, Image, WHITE};
use mahjong_core::tile::{Tile, TileType};

use super::theme;

/// 萬子の数字色（赤）。
pub(super) const LABEL_RED: Color = theme::rgb_pub(0xc22a2a);
/// 筒子の数字色（青）。
pub(super) const LABEL_BLUE: Color = theme::rgb_pub(0x1d55b0);
/// 索子の数字色（緑）。
pub(super) const LABEL_GREEN: Color = theme::rgb_pub(0x1e7d32);
/// 風牌・白のラベル色（黒）。
pub(super) const LABEL_BLACK: Color = theme::rgb_pub(0x202020);

/// 牌種別ごとのラベル文字と色を返す。
///
/// 数牌は数字（萬子=赤・筒子=青・索子=緑）、風牌は E/S/W/N（黒）、
/// 三元牌は輸出用麻雀牌の伝統的表記 P（白）/ F（發）/ C（中）を使う。
/// F は緑・C は赤と、牌自体の色に合わせる。
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

/// 牌画像の右上へインデックスラベル（白い角丸バッジ＋色付き文字）を焼き込む。
///
/// バッジ・文字とも画像サイズに比例した大きさで配置するため、
/// 元画像の解像度が変わってもレイアウトは保たれる。
pub(super) fn bake_label(img: &mut Image, label: &str, color: Color, font: &fontdue::Font) {
    let Some(ch) = label.chars().next() else {
        return;
    };

    let tile_w = img.width as f32;

    // バッジの配置（111px 幅の牌でバッジ約47px・余白約4px）
    let badge = (tile_w * 0.42).round();
    let margin = (tile_w * 0.04).round();
    let bx = tile_w - margin - badge;
    let by = margin;
    let radius = badge * 0.24;

    // 枠（ラベル色）→ 内側（白）の順に塗って色付きリングを作る
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

    // 文字をバッジ中央へ。カバレッジを縦横に膨張させて擬似太字にする
    // （1pxずらしの二重描画より線が均一に太り、縮小描画でも潰れにくい）。
    let px_size = badge * 0.92;
    let (metrics, bitmap) = font.rasterize(ch, px_size);
    let (bitmap, glyph_w, glyph_h) = dilate(&bitmap, metrics.width, metrics.height, 2, 2);
    let gx = (bx + (badge - glyph_w as f32) / 2.0).round() as i32;
    let gy = (by + (badge - glyph_h as f32) / 2.0).round() as i32;
    blit_glyph(img, gx, gy, glyph_w, glyph_h, &bitmap, color);
}

/// グリフのカバレッジを右方向 `dx`・下方向 `dy` ピクセル分膨張させ、
/// ストロークを太らせた新しいビットマップと寸法を返す。
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

/// ラスタライズ済みグリフを画像へアルファ合成する。
fn blit_glyph(img: &mut Image, gx: i32, gy: i32, w: usize, h: usize, bitmap: &[u8], color: Color) {
    for row in 0..h {
        for col in 0..w {
            let coverage = bitmap[row * w + col] as f32 / 255.0;
            blend_pixel(img, gx + col as i32, gy + row as i32, color, coverage);
        }
    }
}

/// 角丸矩形を SDF ベースのカバレッジでアンチエイリアス付きに塗る。
fn fill_rounded_rect(img: &mut Image, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let (hx, hy) = (w / 2.0 - radius, h / 2.0 - radius);
    let x0 = x.floor().max(0.0) as i32;
    let y0 = y.floor().max(0.0) as i32;
    let x1 = (x + w).ceil() as i32;
    let y1 = (y + h).ceil() as i32;
    for py in y0..=y1 {
        for px in x0..=x1 {
            // ピクセル中心での角丸矩形の符号付き距離
            let qx = (px as f32 + 0.5 - cx).abs() - hx;
            let qy = (py as f32 + 0.5 - cy).abs() - hy;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let d = outside + qx.max(qy).min(0.0) - radius;
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            blend_pixel(img, px, py, color, coverage);
        }
    }
}

/// 1ピクセルをアルファ合成する。
///
/// 合成率には元ピクセルのアルファも掛けるため、牌画像の角丸の外
/// （透明部分）にはラベルがはみ出さず、牌のシルエットが保たれる。
/// 出力アルファは元の値を維持する。
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
