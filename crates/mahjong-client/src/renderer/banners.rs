//! 鳴き・リーチ・和了などの宣言バナー（吹き出し）の描画
//!
//! 各家が発声すべき場面（ポン・チー・カン・リーチ・北抜き・ロン・ツモ・
//! 九種九牌）で、その家の手牌・捨て牌の近くに吹き出しを一定時間表示する。
//! バナーの生成と寿命管理は `GameState::process_events` が行い、ここでは
//! `state.call_banners` を描画するだけ。

use macroquad::prelude::*;

use super::{BOARD_CENTER_Y, DESIGN_W, rotation_index, theme};
use crate::game::{CALL_BANNER_SECS, GameState};

/// バナー文字サイズ（`USED_FONT_SIZES` に含まれるサイズを使うこと）
const BANNER_FONT: u16 = 26;
/// フェードアウトにかける時間（秒）
const FADE_SECS: f64 = 0.35;
/// 吹き出しの高さ
const BUBBLE_H: f32 = 46.0;
/// 吹き出しのしっぽ（三角形）の長さ
const TAIL_LEN: f32 = 12.0;

/// しっぽの向き（吹き出しからプレイヤーの手牌方向を指す）
enum TailDir {
    Down,
    Right,
    Up,
    Left,
}

/// 描画スロット（[`super::PLAYER_ROTATIONS`] と同じ並び）ごとの
/// 吹き出し中心座標としっぽの向き。
fn banner_anchor(slot: usize) -> (f32, f32, TailDir) {
    match slot {
        // 自分: 手牌（y=680）の上
        0 => (DESIGN_W / 2.0, 600.0, TailDir::Down),
        // 下家: 右側の手牌の内側
        1 => (890.0, BOARD_CENTER_Y, TailDir::Right),
        // 対面: 上側の手牌の下
        2 => (DESIGN_W / 2.0, 165.0, TailDir::Up),
        // 上家: 左側の手牌の内側
        _ => (390.0, BOARD_CENTER_Y, TailDir::Left),
    }
}

/// アクティブな宣言バナーをすべて描画する。
pub(super) fn draw_call_banners(state: &GameState, font: Option<&Font>) {
    let now = get_time();
    let tr = state.tr();

    for rel in 0..state.player_count {
        let Some(banner) = &state.call_banners[rel] else {
            continue;
        };
        let age = now - banner.shown_at;
        if !(0.0..CALL_BANNER_SECS).contains(&age) {
            continue;
        }
        let alpha = ((CALL_BANNER_SECS - age) / FADE_SECS).clamp(0.0, 1.0) as f32;
        let slot = rotation_index(rel, state.player_count);
        draw_banner_bubble(font, slot, tr.get(banner.label), alpha);
    }
}

/// 吹き出しを1つ描画する。
fn draw_banner_bubble(font: Option<&Font>, slot: usize, text: &str, alpha: f32) {
    let (cx, cy, tail) = banner_anchor(slot);

    let dims = theme::measure_scaled(font, text, BANNER_FONT);
    let w = dims.width + 44.0;
    let h = BUBBLE_H;
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;

    let fill = theme::rgba(0x050e08, 0.94 * alpha);
    let border = theme::rgba(0xc9a227, 0.95 * alpha);

    // しっぽ（本体に少し食い込ませて隙間をなくす）
    let (t1, t2, t3) = match tail {
        TailDir::Down => (
            vec2(cx - 9.0, y + h - 1.0),
            vec2(cx + 9.0, y + h - 1.0),
            vec2(cx, y + h + TAIL_LEN),
        ),
        TailDir::Up => (
            vec2(cx - 9.0, y + 1.0),
            vec2(cx + 9.0, y + 1.0),
            vec2(cx, y - TAIL_LEN),
        ),
        TailDir::Right => (
            vec2(x + w - 1.0, cy - 9.0),
            vec2(x + w - 1.0, cy + 9.0),
            vec2(x + w + TAIL_LEN, cy),
        ),
        TailDir::Left => (
            vec2(x + 1.0, cy - 9.0),
            vec2(x + 1.0, cy + 9.0),
            vec2(x - TAIL_LEN, cy),
        ),
    };
    draw_triangle(t1, t2, t3, border);

    theme::draw_rounded_rect(x, y, w, h, 10.0, fill);
    theme::draw_rounded_rect_lines(x, y, w, h, 10.0, 2.0, border);

    let text_color = Color::new(theme::TEXT_BR.r, theme::TEXT_BR.g, theme::TEXT_BR.b, alpha);
    theme::draw_text_centered(font, text, cx, cy + 9.0, BANNER_FONT, text_color);
}
