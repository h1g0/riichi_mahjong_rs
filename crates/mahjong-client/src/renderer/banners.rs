//! Declaration banners (speech bubbles) for calls, riichi, wins, etc.
//!
//! Whenever a player would call out (pon, chii, kan, riichi, pei, ron,
//! tsumo, nine terminals) a bubble appears near their hand for a while.
//! `GameState::process_events` creates and expires the banners; this
//! module only draws `state.call_banners`.

use macroquad::prelude::*;

use super::{BOARD_CENTER_Y, DESIGN_W, rotation_index, theme};
use crate::game::{CALL_BANNER_SECS, GameState};

/// Banner font size; must be one of `USED_FONT_SIZES`.
const BANNER_FONT: u16 = 26;
/// Fade-out duration in seconds.
const FADE_SECS: f64 = 0.35;
/// Bubble height.
const BUBBLE_H: f32 = 46.0;
/// Length of the bubble's tail triangle.
const TAIL_LEN: f32 = 12.0;

/// Tail direction, pointing from the bubble towards the player's hand.
enum TailDir {
    Down,
    Right,
    Up,
    Left,
}

/// Bubble center and tail direction per draw slot, in
/// [`super::PLAYER_ROTATIONS`] order.
fn banner_anchor(slot: usize) -> (f32, f32, TailDir) {
    match slot {
        // Self: above the hand (y = 680).
        0 => (DESIGN_W / 2.0, 600.0, TailDir::Down),
        // Right player: inside their hand.
        1 => (890.0, BOARD_CENTER_Y, TailDir::Right),
        // Across player: below their hand.
        2 => (DESIGN_W / 2.0, 165.0, TailDir::Up),
        // Left player: inside their hand.
        _ => (390.0, BOARD_CENTER_Y, TailDir::Left),
    }
}

/// Draws every active declaration banner.
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
        let slot = rotation_index(rel, state.player_count, state.my_initial_wind_index());
        draw_banner_bubble(font, slot, tr.get(banner.label), alpha);
    }
}

/// Draws one bubble.
fn draw_banner_bubble(font: Option<&Font>, slot: usize, text: &str, alpha: f32) {
    let (cx, cy, tail) = banner_anchor(slot);

    let dims = theme::measure_scaled(font, text, BANNER_FONT);
    let w = dims.width + 44.0;
    let h = BUBBLE_H;
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;

    let fill = theme::rgba(0x050e08, 0.94 * alpha);
    let border = theme::rgba(0xc9a227, 0.95 * alpha);

    // The tail bites slightly into the body to avoid a seam.
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
