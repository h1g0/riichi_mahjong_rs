//! Tile, hand, and meld rendering.

use super::*;

pub(super) fn draw_hand(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let hand_start_x = player_hand_start_x(state.hand.len());
    let hand_y = HAND_Y;
    let tr = state.tr();

    // Hand first; badges draw later so tiles cannot cover them.
    for (i, tile) in state.hand.iter().enumerate() {
        let x = hand_start_x + i as f32 * TILE_W;
        let selected = state.selected_tile == Some(i);
        let riichi_selectable =
            state.riichi_selection_mode && state.riichi_selectable_tiles.contains(&i);
        let y_offset = if selected { -14.0 } else { 0.0 };
        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;
        // Swap-calling-forbidden tiles render disabled.
        let swap_forbidden = state.forbidden_discards.contains(&tile.get());
        if selected {
            draw_tile_highlight(x, hand_y + y_offset);
        }
        draw_tile(
            x,
            hand_y + y_offset,
            tile,
            riichi_disabled || swap_forbidden,
            tile_textures,
        );
    }

    if let Some(drawn) = &state.drawn {
        let drawn_x = hand_start_x + state.hand.len() as f32 * TILE_W + DRAWN_GAP;
        let selected = state.selected_drawn;
        let riichi_selectable = state.riichi_selection_mode && state.riichi_selectable_drawn;
        let y_offset = if selected { -14.0 } else { 0.0 };
        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;

        theme::draw_text_centered(
            font,
            tr.get(Key::Tsumo),
            drawn_x + TILE_W / 2.0,
            hand_y + y_offset - 8.0,
            11,
            theme::GOLD_LT,
        );

        if selected {
            draw_tile_highlight(drawn_x, hand_y + y_offset);
        }
        draw_tile(
            drawn_x,
            hand_y + y_offset,
            drawn,
            riichi_disabled,
            tile_textures,
        );
    }

    // Status badges (furiten, riichi, riichi-discard choice, swap-calling
    // warning) draw after the hand so tiles cannot cover them.
    let badge_y = hand_y - 26.0;
    let mut bx = hand_start_x;
    if state.is_furiten {
        bx = draw_badge(
            font,
            bx,
            badge_y,
            tr.get(Key::Furiten),
            theme::rgba(0xcc2828, 0.18),
            theme::RED,
            theme::RED_LT,
        );
    }
    if state.is_riichi {
        bx = draw_badge(
            font,
            bx,
            badge_y,
            tr.get(Key::RiichiActive),
            theme::rgba(0xcc2828, 0.12),
            theme::rgba(0xcc2828, 0.35),
            theme::RED,
        );
    }
    if state.riichi_selection_mode {
        bx = draw_badge(
            font,
            bx,
            badge_y,
            tr.get(Key::SelectDiscard),
            theme::rgba(0xc8a227, 0.12),
            theme::rgba(0xc8a227, 0.35),
            theme::GOLD_LT,
        );
    }
    if state.selected_forbidden_swap {
        bx = draw_badge(
            font,
            bx,
            badge_y,
            tr.get(Key::IsSwapCalling),
            theme::rgba(0xcc2828, 0.18),
            theme::RED,
            theme::RED_LT,
        );
    }
    if state.selected_would_cause_furiten && (state.selected_tile.is_some() || state.selected_drawn)
    {
        draw_badge(
            font,
            bx,
            badge_y,
            tr.get(Key::WillBeFuriten),
            theme::rgba(0xcc6411, 0.18),
            theme::rgba(0xe88a1a, 0.6),
            Color::new(1.0, 0.7, 0.3, 1.0),
        );
    }
}

/// Gold outline around the selected tile.
pub(super) fn draw_tile_highlight(x: f32, y: f32) {
    theme::draw_rounded_rect_lines(
        x - 2.0,
        y - 2.0,
        TILE_W - 2.0 + 4.0,
        TILE_H - 2.0 + 4.0,
        4.0,
        2.0,
        theme::GOLD_LT,
    );
}

pub(super) fn draw_melds(state: &GameState, tile_textures: &TileTextures) {
    if state.melds.is_empty() {
        return;
    }

    let tw: f32 = 40.0;
    let th: f32 = 56.0;
    let meld_y: f32 = 692.0;
    let meld_gap: f32 = 12.0;
    let right_edge: f32 = 1220.0;

    // Earliest meld on the right, later melds further left.
    let xs = self_meld_x_positions(&state.melds, tw, th, meld_gap, right_edge);
    for (meld, &x) in state.melds.iter().zip(&xs) {
        draw_meld_group(meld, x, meld_y, tw, th, tile_textures);
    }
}

/// Left x of each of our melds when laid out earliest-rightmost.
///
/// Returned in `melds` order (index 0 = earliest). Packed right-to-left
/// from `right_edge`, so `xs[0]` is the largest (rightmost).
pub(super) fn self_meld_x_positions(
    melds: &[Meld],
    tw: f32,
    th: f32,
    gap: f32,
    right_edge: f32,
) -> Vec<f32> {
    let mut xs = Vec::with_capacity(melds.len());
    let mut x = right_edge;
    for meld in melds.iter() {
        x -= calc_meld_width(meld, tw, th);
        xs.push(x);
        x -= gap;
    }
    xs
}

pub(super) fn draw_meld_tile(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    w: f32,
    h: f32,
    tile_textures: &TileTextures,
) {
    draw_tile_sprite(tile_textures.for_tile(tile), x, y, w - 2.0, h - 2.0, WHITE);
}

/// Draws a sideways (90-degree) meld tile.
pub(super) fn draw_meld_tile_sideways(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    tw: f32,
    th: f32,
    tile_textures: &TileTextures,
) {
    // A sideways tile's bounding box is th wide and tw tall.
    draw_tile_sprite_rotated(
        tile_textures.for_tile(tile),
        x,
        y,
        tw - 2.0,
        th - 2.0,
        WHITE,
        -std::f32::consts::FRAC_PI_2,
    );
}

/// Draws a face-down meld tile (concealed kans).
pub(super) fn draw_meld_tile_back(x: f32, y: f32, w: f32, h: f32, tile_textures: &TileTextures) {
    draw_tile_sprite(&tile_textures.back, x, y, w - 2.0, h - 2.0, WHITE);
}

/// Position index of the sideways tile, by call source.
pub(super) fn sideways_index(from: MeldFrom, tile_count: usize) -> usize {
    match from {
        MeldFrom::Previous => 0,               // left player: leftmost
        MeldFrom::Opposite => 1,               // across: second from left
        MeldFrom::Following => tile_count - 1, // right player: rightmost
        _ => 0,                                // Unknown/Myself: fallback
    }
}

/// Rendered width of a meld group.
pub(super) fn calc_meld_width(meld: &Meld, tw: f32, th: f32) -> f32 {
    match meld.category {
        MeldType::Kan if meld.from == MeldFrom::Myself => {
            // Concealed kan: four upright tiles.
            4.0 * tw
        }
        MeldType::Kakan => {
            // Kakan: two stacked at the sideways slot (th wide),
            // the rest upright.
            2.0 * tw + th
        }
        MeldType::Chi | MeldType::Pon => {
            // Chii/pon: one sideways (th wide), two upright.
            2.0 * tw + th
        }
        MeldType::Kan => {
            // Called kan: one sideways (th wide), three upright.
            3.0 * tw + th
        }
    }
}

/// Draws a meld group.
pub(super) fn draw_meld_group(
    meld: &Meld,
    base_x: f32,
    base_y: f32,
    tw: f32,
    th: f32,
    tile_textures: &TileTextures,
) {
    match meld.category {
        MeldType::Kan if meld.from == MeldFrom::Myself => {
            // Concealed kan: tiles 1 and 4 face down, 2 and 3 face up.
            for i in 0..4 {
                let x = base_x + i as f32 * tw;
                if i == 0 || i == 3 {
                    draw_meld_tile_back(x, base_y, tw, th, tile_textures);
                } else {
                    draw_meld_tile(x, base_y, &meld.tiles[i], tw, th, tile_textures);
                }
            }
        }
        MeldType::Chi => {
            // Chii: the called tile sideways on the left, the rest upright.
            let mut sorted_tiles = meld.tiles.clone();
            sorted_tiles.sort();
            let called = meld.called_tile;

            let mut x = base_x;
            if let Some(ct) = called {
                draw_meld_tile_sideways(x, base_y + (th - tw), &ct, tw, th, tile_textures);
                x += th;
                let mut skipped = false;
                for tile in &sorted_tiles {
                    if !skipped && tile.get() == ct.get() {
                        skipped = true;
                        continue;
                    }
                    draw_meld_tile(x, base_y, tile, tw, th, tile_textures);
                    x += tw;
                }
            } else {
                for tile in &sorted_tiles {
                    draw_meld_tile(x, base_y, tile, tw, th, tile_textures);
                    x += tw;
                }
            }
        }
        MeldType::Pon => {
            // Pon: the sideways slot follows the call source.
            let side_idx = sideways_index(meld.from, 3);
            let mut x = base_x;
            for i in 0..3 {
                if i == side_idx {
                    draw_meld_tile_sideways(
                        x,
                        base_y + (th - tw),
                        &meld.tiles[i],
                        tw,
                        th,
                        tile_textures,
                    );
                    x += th;
                } else {
                    draw_meld_tile(x, base_y, &meld.tiles[i], tw, th, tile_textures);
                    x += tw;
                }
            }
        }
        MeldType::Kan => {
            // Called kan: same, with four tiles.
            let side_idx = sideways_index(meld.from, 4);
            let mut x = base_x;
            for i in 0..4 {
                if i == side_idx {
                    draw_meld_tile_sideways(
                        x,
                        base_y + (th - tw),
                        &meld.tiles[i],
                        tw,
                        th,
                        tile_textures,
                    );
                    x += th;
                } else {
                    draw_meld_tile(x, base_y, &meld.tiles[i], tw, th, tile_textures);
                    x += tw;
                }
            }
        }
        MeldType::Kakan => {
            // Kakan: two tiles stacked at the pon's sideways slot.
            let side_idx = sideways_index(meld.from, 3);
            let mut x = base_x;
            for i in 0..3 {
                if i == side_idx {
                    draw_meld_tile_sideways(
                        x,
                        base_y + (th - tw),
                        &meld.tiles[i],
                        tw,
                        th,
                        tile_textures,
                    );
                    if meld.tiles.len() > 3 {
                        draw_meld_tile_sideways(
                            x,
                            base_y + (th - tw) - tw,
                            &meld.tiles[3],
                            tw,
                            th,
                            tile_textures,
                        );
                    }
                    x += th;
                } else {
                    draw_meld_tile(x, base_y, &meld.tiles[i], tw, th, tile_textures);
                    x += tw;
                }
            }
        }
    }
}

pub(super) fn draw_tile(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    riichi_disabled: bool,
    tile_textures: &TileTextures,
) {
    let tint = if riichi_disabled {
        RIICHI_DISABLED_TINT
    } else {
        WHITE
    };
    draw_tile_sprite(
        tile_textures.for_tile(tile),
        x,
        y,
        TILE_W - 2.0,
        TILE_H - 2.0,
        tint,
    );
}

pub(super) fn draw_tile_sprite(texture: &Texture2D, x: f32, y: f32, w: f32, h: f32, tint: Color) {
    draw_texture_ex(
        texture,
        x,
        y,
        tint,
        DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            ..Default::default()
        },
    );
}

/// Draws a tile sprite with rotation.
///
/// (vx, vy) is the visual top-left after rotation. The texture always
/// draws at its natural aspect (w, h); the rotation-induced offset is
/// corrected internally.
pub(super) fn draw_tile_sprite_rotated(
    texture: &Texture2D,
    vx: f32,
    vy: f32,
    w: f32,
    h: f32,
    tint: Color,
    rotation: f32,
) {
    // At 90 degrees the bounding box's top-left shifts about the (w, h)
    // rect center; the visual size becomes (h, w). Draw position =
    // visual position + correction.
    let is_90 = (rotation.abs() - std::f32::consts::FRAC_PI_2).abs() < 0.01;
    let (dx, dy) = if is_90 {
        ((h - w) / 2.0, (w - h) / 2.0)
    } else {
        (0.0, 0.0)
    };
    let x = vx + dx;
    let y = vy + dy;

    draw_texture_ex(
        texture,
        x,
        y,
        tint,
        DrawTextureParams {
            dest_size: Some(vec2(w, h)),
            rotation,
            pivot: Some(vec2(x + w / 2.0, y + h / 2.0)),
            ..Default::default()
        },
    );
}

/// Gap for an opponent's drawn tile, matching their reduced tile size.
pub(super) const OTHER_DRAWN_GAP: f32 = 8.0;
/// Hand-discard animation: how long the vacated gap shows (seconds).
pub(super) const TEDASHI_GAP_HOLD_SECS: f64 = 0.25;
/// Hand-discard animation: the gap-closing slide duration (seconds).
pub(super) const TEDASHI_SLIDE_SECS: f64 = 0.3;

/// Progress of the hand-discard animation, 0.0 (gap showing) to
/// 1.0 (slide finished).
pub(super) fn tedashi_progress(elapsed: f64) -> f32 {
    if elapsed < TEDASHI_GAP_HOLD_SECS {
        return 0.0;
    }
    let t = (((elapsed - TEDASHI_GAP_HOLD_SECS) / TEDASHI_SLIDE_SECS).clamp(0.0, 1.0)) as f32;
    // Ease-out: decelerate to a stop.
    1.0 - (1.0 - t) * (1.0 - t)
}

/// How far the centering anchor (hand left edge) moved across a hand
/// discard (pre-discard X minus post-discard X).
///
/// With a drawn tile the count is unchanged and there is no shift;
/// without one (a post-call discard) the hand shrinks by one and the
/// centering shifts half a tile.
fn tedashi_start_shift(had_drawn: bool, tile_step: f32) -> f32 {
    if had_drawn { 0.0 } else { -tile_step / 2.0 }
}

/// Per-tile X offsets (added to post-discard positions) during the
/// hand-discard animation.
///
/// - Tiles left of the gap stay put (centering shift only).
/// - Tiles right of the gap start one tile right (their pre-discard
///   spot) and slide left.
/// - A drawn tile that was hanging out slides from its slot into the
///   hand's right end.
pub(super) fn tedashi_tile_offset(
    final_index: usize,
    hand_count: usize,
    gap_index: usize,
    had_drawn: bool,
    tile_step: f32,
    drawn_gap: f32,
    progress: f32,
) -> f32 {
    let shift = tedashi_start_shift(had_drawn, tile_step);
    let from = if final_index < gap_index {
        shift
    } else if had_drawn && final_index + 1 == hand_count {
        shift + tile_step + drawn_gap
    } else {
        shift + tile_step
    };
    from * (1.0 - progress)
}

/// Draws the other players' hands.
///
/// Like the discards, drawn in the normalized self view (left to right)
/// and rotated into place with a Camera2D about the board center.
///
/// A hidden hand always reserves the drawn-tile slot at its right end,
/// so centering never shifts the whole hand on each draw/discard; the
/// drawn tile hangs out with a gap, as with our own hand.
pub(super) fn draw_other_player_hands(state: &GameState, tile_textures: &TileTextures) {
    let tw: f32 = 28.0; // natural tile width
    let th: f32 = 40.0; // natural tile height
    let meld_gap: f32 = 6.0;
    let tile_step: f32 = tw; // tiles touch
    let hand_distance: f32 = 290.0; // center to the hand

    let base_y = BOARD_CENTER_Y + hand_distance;
    let now = get_time();

    for other_idx in 0..(state.player_count - 1) {
        let relative_idx = other_idx + 1; // 1 right, 2 across, 3 left
        // (three-player: 1 right, 2 left)
        let other = &state.other_players[other_idx];

        // Center on the total width: hand + drawn-tile slot + melds.
        let hand_count = if other.revealed {
            other.hand.len()
        } else {
            other.concealed_count
        };
        let drawn_slot = if other.revealed {
            0.0
        } else {
            OTHER_DRAWN_GAP + tile_step
        };
        let meld_widths: f32 = other.melds.iter().map(|m| calc_meld_width(m, tw, th)).sum();
        let meld_gaps = if other.melds.is_empty() {
            0.0
        } else {
            meld_gap + (other.melds.len() as f32 - 1.0) * meld_gap
        };
        let total_width = hand_count as f32 * tile_step + drawn_slot + meld_widths + meld_gaps;
        let start_x = BOARD_CENTER_X - total_width / 2.0;

        set_camera(&make_board_camera(
            PLAYER_ROTATIONS[rotation_index(
                relative_idx,
                state.player_count,
                state.my_initial_wind_index(),
            )],
        ));

        // The in-flight discard animation, ignored once finished.
        let anim = other.tedashi_anim.and_then(|a| {
            let p = tedashi_progress(now - a.started_at);
            (p < 1.0 && !other.revealed).then_some((a, p))
        });

        let mut x = start_x;
        if other.revealed {
            for tile in &other.hand {
                draw_tile_sprite(tile_textures.for_tile(tile), x, base_y, tw, th, WHITE);
                x += tile_step;
            }
        } else {
            for i in 0..hand_count {
                let offset = anim.map_or(0.0, |(a, p)| {
                    tedashi_tile_offset(
                        i,
                        hand_count,
                        a.gap_index,
                        a.had_drawn,
                        tile_step,
                        OTHER_DRAWN_GAP,
                        p,
                    )
                });
                draw_tile_sprite(&tile_textures.back, x + offset, base_y, tw, th, WHITE);
                x += tile_step;
            }

            if other.has_drawn {
                draw_tile_sprite(
                    &tile_textures.back,
                    x + OTHER_DRAWN_GAP,
                    base_y,
                    tw,
                    th,
                    WHITE,
                );
            }
            x += drawn_slot;
        }

        // Melds continue past the drawn-tile slot; on a post-call discard
        // they slide together with the hand by the centering shift.
        let meld_offset = anim.map_or(0.0, |(a, p)| {
            tedashi_start_shift(a.had_drawn, tile_step) * (1.0 - p)
        });
        if !other.melds.is_empty() {
            x += meld_gap;
        }
        let xs = other_meld_x_positions(&other.melds, tw, th, meld_gap, x + meld_offset);
        for (meld, &mx) in other.melds.iter().zip(&xs) {
            draw_meld_group(meld, mx, base_y, tw, th, tile_textures);
        }

        set_design_camera();
    }
}

/// Left x of each of an opponent's melds, laid out earliest-rightmost
/// from their point of view.
///
/// Returned in `melds` order (index 0 = earliest). Packed left-to-right
/// from `start_x` beside the hand, but in reverse order so the earliest
/// meld lands rightmost; `xs[0]` is the largest.
pub(super) fn other_meld_x_positions(
    melds: &[Meld],
    tw: f32,
    th: f32,
    gap: f32,
    start_x: f32,
) -> Vec<f32> {
    let mut xs = vec![0.0; melds.len()];
    let mut x = start_x;
    for (draw_i, meld) in melds.iter().rev().enumerate() {
        if draw_i > 0 {
            x += gap;
        }
        let call_i = melds.len() - 1 - draw_i;
        xs[call_i] = x;
        x += calc_meld_width(meld, tw, th);
    }
    xs
}
