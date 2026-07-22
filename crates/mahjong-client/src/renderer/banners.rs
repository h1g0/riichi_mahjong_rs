//! Declaration banners (speech bubbles) for calls, riichi, wins, etc.
//!
//! Whenever a player would call out (pon, chii, kan, riichi, pei, ron,
//! tsumo, nine terminals, tenpai, noten) a bubble appears near their hand
//! for a while.
//! `GameState::process_events` creates and expires the banners; this
//! module only draws `state.call_banners`.

use macroquad::prelude::*;

use super::{BOARD_CENTER_Y, DESIGN_W, rotation_index, theme};
use crate::game::{CALL_BANNER_SECS, GameState};

/// Banner font size from the shared typography scale.
const BANNER_FONT: u16 = theme::font_size::TITLE;
/// Fade-out duration in seconds.
const FADE_SECS: f64 = 0.35;
/// Bubble height.
const BUBBLE_H: f32 = 46.0;
/// Length of the bubble's tail triangle.
const TAIL_LEN: f32 = 12.0;
/// Width of the black outline outside the gold frame.
const OUTLINE_W: f32 = 1.0;

/// Tail direction, pointing from the bubble towards the player's hand.
#[derive(Clone, Copy)]
enum TailDir {
    Down,
    Right,
    Up,
    Left,
}

fn tail_triangle(x: f32, y: f32, w: f32, h: f32, tail: TailDir, outset: f32) -> [Vec2; 3] {
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let half_base = 9.0 + outset;

    match tail {
        TailDir::Down => [
            vec2(cx - half_base, y + h - 1.0),
            vec2(cx + half_base, y + h - 1.0),
            vec2(cx, y + h + TAIL_LEN + outset),
        ],
        TailDir::Up => [
            vec2(cx - half_base, y + 1.0),
            vec2(cx + half_base, y + 1.0),
            vec2(cx, y - TAIL_LEN - outset),
        ],
        TailDir::Right => [
            vec2(x + w - 1.0, cy - half_base),
            vec2(x + w - 1.0, cy + half_base),
            vec2(x + w + TAIL_LEN + outset, cy),
        ],
        TailDir::Left => [
            vec2(x + 1.0, cy - half_base),
            vec2(x + 1.0, cy + half_base),
            vec2(x - TAIL_LEN - outset, cy),
        ],
    }
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

    let dims = theme::measure_text_size(font, text, BANNER_FONT);
    let w = dims.width + 44.0;
    let h = BUBBLE_H;
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;

    let fill = theme::rgba(0x050e08, 0.94 * alpha);
    let border = theme::rgba(0xc9a227, 0.95 * alpha);
    let outline = theme::rgba(0x000000, 0.95 * alpha);

    let [ot1, ot2, ot3] = tail_triangle(x, y, w, h, tail, OUTLINE_W);
    draw_triangle(ot1, ot2, ot3, outline);
    theme::draw_rounded_rect(
        x - OUTLINE_W,
        y - OUTLINE_W,
        w + OUTLINE_W * 2.0,
        h + OUTLINE_W * 2.0,
        10.0 + OUTLINE_W,
        outline,
    );

    let [t1, t2, t3] = tail_triangle(x, y, w, h, tail, 0.0);
    draw_triangle(t1, t2, t3, border);

    theme::draw_rounded_rect(x, y, w, h, 10.0, fill);
    theme::draw_rounded_rect_lines(x, y, w, h, 10.0, 2.0, border);

    let text_color = Color::new(theme::TEXT_BR.r, theme::TEXT_BR.g, theme::TEXT_BR.b, alpha);
    theme::draw_text_centered(font, text, cx, cy + 9.0, BANNER_FONT, text_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_outline_surrounds_every_direction() {
        let x = 100.0;
        let y = 200.0;
        let w = 80.0;
        let h = BUBBLE_H;

        for tail in [TailDir::Down, TailDir::Right, TailDir::Up, TailDir::Left] {
            let inner = tail_triangle(x, y, w, h, tail, 0.0);
            let outer = tail_triangle(x, y, w, h, tail, OUTLINE_W);
            let bounds = |points: [Vec2; 3]| {
                points.into_iter().fold(
                    (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                    |(min_x, min_y, max_x, max_y), point| {
                        (
                            min_x.min(point.x),
                            min_y.min(point.y),
                            max_x.max(point.x),
                            max_y.max(point.y),
                        )
                    },
                )
            };
            let (inner_min_x, inner_min_y, inner_max_x, inner_max_y) = bounds(inner);
            let (outer_min_x, outer_min_y, outer_max_x, outer_max_y) = bounds(outer);

            assert!(outer_min_x <= inner_min_x);
            assert!(outer_min_y <= inner_min_y);
            assert!(outer_max_x >= inner_max_x);
            assert!(outer_max_y >= inner_max_y);
            assert!(
                outer_min_x < inner_min_x
                    || outer_min_y < inner_min_y
                    || outer_max_x > inner_max_x
                    || outer_max_y > inner_max_y
            );
        }
    }
}
