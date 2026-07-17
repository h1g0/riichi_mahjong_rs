//! Board rendering: background, top bar, dora display, center panel,
//! and discard pools.

use super::*;

/// Game background: radial felt, brighter at the center.
pub(super) fn draw_felt_background() {
    theme::draw_radial_bg(
        DESIGN_W,
        DESIGN_H,
        DESIGN_W / 2.0,
        DESIGN_H * 0.46,
        DESIGN_W * 0.62,
        DESIGN_H * 0.62,
        theme::FELT,
        theme::FELT_EDGE,
    );
}

/// Setup/end-screen background: light green fading to dark.
pub(super) fn draw_setup_background() {
    theme::draw_radial_bg(
        DESIGN_W,
        DESIGN_H,
        DESIGN_W / 2.0,
        DESIGN_H * 0.42,
        DESIGN_W * 0.6,
        DESIGN_H * 0.6,
        theme::SETUP_BG_INNER,
        theme::FELT_EDGE,
    );
}

/// Draws the top bar: dora display, hand/remaining counters, and
/// per-player score chips.
pub(super) fn draw_top_bar(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    const BAR_H: f32 = 50.0;

    draw_rectangle(0.0, 0.0, DESIGN_W, BAR_H, Color::new(0.0, 0.0, 0.0, 0.48));
    draw_rectangle(0.0, BAR_H - 1.0, DESIGN_W, 1.0, theme::BORDER);

    draw_dora_panel(state, font, tile_textures);
    draw_round_center(state, font, BAR_H);
    draw_score_chips(state, font, BAR_H);
}

/// Top-bar left: dora indicators, riichi deposits, honba.
pub(super) fn draw_dora_panel(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) {
    let panel_x = 12.0;
    let panel_y = 8.0;
    let panel_h = 34.0;
    let dora_w = 20.0;
    let dora_h = 28.0;
    let tiles_x = panel_x + 44.0;
    let tiles_y = panel_y + 3.0;
    let sticks_x = tiles_x + 5.0 * (dora_w + 1.0) + 12.0;
    let panel_w = sticks_x + 64.0 - panel_x;

    theme::draw_panel(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        6.0,
        Color::new(0.0, 0.0, 0.0, 0.5),
        theme::rgba(0xc9a227, 0.18),
    );

    draw_jp_text(
        font,
        DoraLabel::Dora.name(state.lang),
        panel_x + 10.0,
        panel_y + 21.0,
        11,
        theme::TEXT_DIM,
    );

    let revealed = state.dora_indicators.len();
    let selected_tile_type = state.selected_tile_type();
    for i in 0..5 {
        let x = tiles_x + i as f32 * (dora_w + 1.0);
        if i < revealed {
            let tile = &state.dora_indicators[i];
            draw_tile_sprite(
                tile_textures.for_tile(tile),
                x,
                tiles_y,
                dora_w,
                dora_h,
                public_tile_tint(tile, selected_tile_type),
            );
        } else {
            draw_tile_sprite(&tile_textures.back, x, tiles_y, dora_w, dora_h, WHITE);
        }
    }

    // Deposits on the upper row, honba on the lower.
    draw_tile_sprite(
        &tile_textures.stick1000,
        sticks_x,
        panel_y + 8.0,
        34.0,
        5.0,
        Color::new(1.0, 1.0, 1.0, 0.75),
    );
    draw_jp_text(
        font,
        &format!("×{}", state.riichi_sticks),
        sticks_x + 38.0,
        panel_y + 14.0,
        11,
        theme::TEXT_DIM,
    );
    draw_tile_sprite(
        &tile_textures.stick100,
        sticks_x,
        panel_y + 22.0,
        34.0,
        5.0,
        Color::new(1.0, 1.0, 1.0, 0.75),
    );
    draw_jp_text(
        font,
        &format!("×{}", state.honba),
        sticks_x + 38.0,
        panel_y + 28.0,
        11,
        theme::TEXT_DIM,
    );
}

/// Top-bar center: rules (players + length), hand, honba.
pub(super) fn draw_round_center(state: &GameState, font: Option<&Font>, bar_h: f32) {
    let tr = state.tr();
    let rule_text = tr.get(state.setup_state.mode.label_key()).to_string();
    let round_text = tr.round_label(state.round_number, state.player_count);
    let honba_text = if state.honba > 0 {
        Some(tr.honba_suffix(state.honba))
    } else {
        None
    };

    let baseline = bar_h / 2.0 + 6.0;
    let gap = 12.0;
    let gdims = theme::measure_scaled(font, &rule_text, 16);
    let rdims = theme::measure_scaled(font, &round_text, 16);
    let hdims = honba_text
        .as_ref()
        .map(|t| theme::measure_scaled(font, t, 14));

    let mut total_w = gdims.width + gap + rdims.width;
    if let Some(hd) = &hdims {
        total_w += gap + hd.width;
    }
    let start_x = DESIGN_W / 2.0 - total_w / 2.0;

    draw_jp_text(font, &rule_text, start_x, baseline, 16, theme::GOLD_LT);
    let round_x = start_x + gdims.width + gap;
    draw_jp_text(font, &round_text, round_x, baseline, 16, theme::GOLD_LT);
    if let Some(honba_text) = &honba_text {
        draw_jp_text(
            font,
            honba_text,
            round_x + rdims.width + gap,
            baseline,
            14,
            theme::TEXT_DIM,
        );
    }
}

/// Top-bar right: per-player score chips, ours highlighted.
pub(super) fn draw_score_chips(state: &GameState, font: Option<&Font>, bar_h: f32) {
    const CHIP_W: f32 = 70.0;
    const CHIP_H: f32 = 38.0;
    const GAP: f32 = 7.0;
    let count = state.player_count;
    let total = count as f32 * CHIP_W + (count as f32 - 1.0) * GAP;
    let start_x = DESIGN_W - 14.0 - total;
    let chip_y = (bar_h - CHIP_H) / 2.0;

    for rel in 0..state.player_count {
        let seat = seat_at_relative_position(state.my_seat, rel, state.player_count);
        let is_me = seat == state.my_seat;
        let x = start_x + rel as f32 * (CHIP_W + GAP);

        let (fill, border) = if is_me {
            (theme::rgba(0xc8a227, 0.10), theme::rgba(0xc8a227, 0.28))
        } else {
            (
                Color::new(1.0, 1.0, 1.0, 0.04),
                Color::new(1.0, 1.0, 1.0, 0.06),
            )
        };
        theme::draw_rounded_rect(x, chip_y, CHIP_W, CHIP_H, 4.0, fill);
        theme::draw_rounded_rect_lines(x, chip_y, CHIP_W, CHIP_H, 4.0, 1.0, border);

        let name = short_player_name(&state.player_labels[seat], rel, state.lang);
        theme::draw_text_centered(
            font,
            &name,
            x + CHIP_W / 2.0,
            chip_y + 14.0,
            9,
            theme::TEXT_DIM,
        );
        let val = format_score(state.scores[seat]);
        let val_color = if is_me { theme::GOLD_LT } else { theme::TEXT };
        theme::draw_text_centered(font, &val, x + CHIP_W / 2.0, chip_y + 30.0, 13, val_color);
    }
}

/// Short player name for the score chips.
pub(super) fn short_player_name(
    label: &crate::game::PlayerLabel,
    rel: usize,
    lang: Lang,
) -> String {
    label.short_name(rel, lang)
}

/// Score text with digit grouping.
pub(super) fn format_score(score: i32) -> String {
    let neg = score < 0;
    let mut n = score.unsigned_abs();
    if n == 0 {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    parts.reverse();
    let mut joined = parts.join(",");
    joined = joined.trim_start_matches('0').to_string();
    if joined.starts_with(',') {
        joined = joined.trim_start_matches(',').to_string();
    }
    if neg { format!("-{}", joined) } else { joined }
}

/// Draws the center info panel: translucent square with each player's
/// wind and score plus the hand info.
pub(super) fn draw_center_panel(state: &GameState, font: Option<&Font>) {
    // A gold-framed panel sized to fit inside the discard pools.
    let panel_size: f32 = 160.0;
    let half = panel_size / 2.0;
    theme::draw_panel(
        BOARD_CENTER_X - half,
        BOARD_CENTER_Y - half,
        panel_size,
        panel_size,
        5.0,
        theme::rgba(0x030a06, 0.92),
        theme::PANEL_BORDER,
    );

    let my_wind_idx = state.my_wind_index();
    let my_initial_wind_idx = state.my_initial_wind_index();
    let label_dist: f32 = 64.0;

    for rel in 0..state.player_count {
        // rel is the position relative to us. scores/player_labels are
        // seat-ordered, so convert to the absolute seat first - keeping
        // things straight when a non-host sits off seat 0 online.
        let seat = seat_at_relative_position(state.my_seat, rel, state.player_count);
        let display_wind =
            mahjong_core::tile::Wind::from_index((my_wind_idx + rel) % state.player_count);
        let score = state.scores[seat];
        let rotation =
            PLAYER_ROTATIONS[rotation_index(rel, state.player_count, my_initial_wind_idx)];

        set_camera(&make_board_camera(rotation));

        // Glow the acting player's edge; drawn before the text to
        // avoid overlap.
        if state.turn_player == Some(display_wind) {
            draw_turn_indicator_edge(BOARD_CENTER_X - half, BOARD_CENTER_Y + half, panel_size);
        }

        theme::draw_text_centered(
            font,
            display_wind.name(state.lang),
            BOARD_CENTER_X,
            BOARD_CENTER_Y + label_dist,
            14,
            theme::GOLD_LT,
        );
        let score_label = format_score(score);
        theme::draw_text_centered(
            font,
            &score_label,
            BOARD_CENTER_X,
            BOARD_CENTER_Y + label_dist + 14.0,
            11,
            theme::TEXT_DIM,
        );

        // Show the CPU's level/personality (or the opponent's name) under
        // the wind and score; rel matches the score chips' CPU numbering.
        if let Some(detail) = state.player_labels[seat].detail(rel, state.lang) {
            theme::draw_text_centered(
                font,
                &detail,
                BOARD_CENTER_X,
                BOARD_CENTER_Y + label_dist + 28.0,
                11,
                theme::rgba(0x7a9880, 0.85),
            );
        }

        set_design_camera();
    }

    // The hand info, oriented for us to read.
    let round_text = state
        .tr()
        .round_label(state.round_number, state.player_count);
    let remaining_text = state.tr().wall_remaining(state.remaining_tiles);

    // Hand label small; remaining-tile count emphasized.
    theme::draw_text_centered(
        font,
        &round_text,
        BOARD_CENTER_X,
        BOARD_CENTER_Y - 8.0,
        13,
        theme::TEXT_DIM,
    );
    theme::draw_text_centered(
        font,
        &remaining_text,
        BOARD_CENTER_X,
        BOARD_CENTER_Y + 18.0,
        21,
        theme::TEXT_BR,
    );
}

/// Draws a slowly pulsing gold glow along the acting player's edge of
/// the center panel.
///
/// `(x, y)` is the edge's left end, `w` its length. Drawn on the near
/// (bottom) edge inside each player's rotated camera, so the caller
/// switches the rotation.
fn draw_turn_indicator_edge(x: f32, y: f32, w: f32) {
    // Pulse gently on a ~2-second cycle.
    let pulse = 0.775 + 0.225 * (get_time() * std::f64::consts::TAU / 2.0).sin() as f32;

    // Inset from the panel's rounded corners (radius 5).
    let inset = 6.0;
    let (x, w) = (x + inset, w - 2.0 * inset);

    // Stack bands fading outward from the center line for the glow.
    for i in 0..4 {
        let spread = 2.0 + i as f32 * 2.0;
        let alpha = 0.16 * pulse / (i as f32 + 1.0);
        draw_rectangle(x, y - spread, w, spread * 2.0, theme::rgba(0xffd84a, alpha));
    }
    draw_rectangle(x, y - 1.5, w, 3.0, theme::rgba(0xffe066, 0.95 * pulse));
}

pub(super) fn draw_discards(state: &GameState, tile_textures: &TileTextures) {
    let dtw: f32 = 32.0; // natural tile width
    let dth: f32 = 44.0; // natural tile height
    let col_step: f32 = dtw; // columns touch
    let row_step: f32 = dth; // rows touch

    // Normalized layout in our own view: left-to-right, rows downward.
    let half_width = 3.0 * col_step; // half of six tiles = 108px
    let stick_offset: f32 = 108.0; // center to the riichi stick
    let discard_offset: f32 = 130.0; // center to the first discard row
    // (leaves room for the stick)

    // Riichi stick draw size (source ~800x117px, shrunk horizontal).
    let stick_w: f32 = 100.0;
    let stick_h: f32 = 14.0;

    let start_x = BOARD_CENTER_X - half_width;
    let start_y = BOARD_CENTER_Y + discard_offset;

    let my_wind_idx = state.my_wind_index();
    let my_initial_wind_idx = state.my_initial_wind_index();
    let selected_tile_type = state.selected_tile_type();
    let dora_tile_types = dora_tile_types(&state.dora_indicators, state.is_three_player());

    for rel in 0..state.player_count {
        let discards = &state.discards[rel];
        let rotation =
            PLAYER_ROTATIONS[rotation_index(rel, state.player_count, my_initial_wind_idx)];

        set_camera(&make_board_camera(rotation));

        // Extracted North tiles (three-player) line up small to the
        // right of the discard area.
        if state.is_three_player() {
            let wind_idx = (my_wind_idx + rel) % state.player_count;
            let pei = state.pei_counts[wind_idx] as usize;
            let north = mahjong_core::tile::Tile::new(mahjong_core::tile::Tile::Z4);
            let kw = dtw * 0.75;
            let kh = dth * 0.75;
            for k in 0..pei {
                draw_tile_sprite(
                    tile_textures.for_tile(&north),
                    BOARD_CENTER_X + half_width + 10.0 + k as f32 * (kw * 0.6),
                    start_y,
                    kw,
                    kh,
                    public_tile_tint_with_base(&north, selected_tile_type, DORA_TILE_TINT),
                );
            }
        }

        let has_riichi = discards.iter().any(|d| d.is_riichi);
        if has_riichi {
            draw_tile_sprite(
                &tile_textures.stick1000,
                BOARD_CENTER_X - stick_w / 2.0,
                BOARD_CENTER_Y + stick_offset,
                stick_w,
                stick_h,
                WHITE,
            );
        }

        // Discards in normalized layout; the camera rotation orients
        // them per player.
        let mut col_offset: f32 = 0.0;
        let mut current_row: usize = 0;

        for (i, discard) in discards.iter().enumerate() {
            let row = i / 6;
            let mut tint = if discard.is_tsumogiri {
                Color::new(0.72, 0.72, 0.72, 1.0)
            } else {
                WHITE
            };
            // Called tiles draw almost transparent.
            if discard.is_called {
                tint.a = 0.28;
            }
            tint = visible_tile_tint_with_base(
                &discard.tile,
                selected_tile_type,
                &dora_tile_types,
                tint,
            );

            if row != current_row {
                col_offset = 0.0;
                current_row = row;
            }

            if discard.is_riichi {
                // The riichi tile lies sideways.
                let x = start_x + col_offset;
                let y = start_y + row as f32 * row_step + (dth - dtw) / 2.0;
                draw_tile_sprite_rotated(
                    tile_textures.for_tile(&discard.tile),
                    x,
                    y,
                    dtw,
                    dth,
                    tint,
                    -std::f32::consts::FRAC_PI_2,
                );
                col_offset += dth; // A sideways tile is dth wide.
            } else {
                let x = start_x + col_offset;
                let y = start_y + row as f32 * row_step;
                draw_tile_sprite(tile_textures.for_tile(&discard.tile), x, y, dtw, dth, tint);
                col_offset += col_step;
            }
        }

        set_design_camera();
    }
}

/// Draws a status badge (pill) above the hand and returns the next
/// badge's x.
pub(super) fn draw_badge(
    font: Option<&Font>,
    x: f32,
    y: f32,
    text: &str,
    fill: Color,
    border: Color,
    text_color: Color,
) -> f32 {
    let dims = theme::measure_scaled(font, text, 11);
    let pad = 8.0;
    let w = dims.width + pad * 2.0;
    let h = 18.0;
    theme::draw_rounded_rect(x, y, w, h, 3.0, fill);
    theme::draw_rounded_rect_lines(x, y, w, h, 3.0, 1.0, border);
    draw_jp_text(font, text, x + pad, y + 13.0, 11, text_color);
    x + w + 6.0
}
