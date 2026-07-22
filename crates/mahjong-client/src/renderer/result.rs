//! Result and game-over overlay rendering.

use super::*;

const YAKU_FONT_SIZE: u16 = theme::font_size::LABEL;
pub(super) const YAKU_ROW_HEIGHT: f32 = 22.0;
pub(super) const WIN_HAND_TILE_W: f32 = 40.0;
pub(super) const WIN_HAND_TILE_H: f32 = 56.0;
pub(super) const WIN_HAND_ROW_HEIGHT: f32 = 64.0;
pub(super) const WIN_PANEL_WIDTH: f32 = 780.0;
pub(super) const WIN_PANEL_CONTENT_PADDING: f32 = 40.0;
pub(super) const WIN_PANEL_CONTENT_WIDTH: f32 = WIN_PANEL_WIDTH - 2.0 * WIN_PANEL_CONTENT_PADDING;
pub(super) const RESULT_INDICATOR_TILE_W: f32 = 30.0;
pub(super) const RESULT_INDICATOR_TILE_H: f32 = 42.0;
pub(super) const RESULT_INDICATOR_ROW_HEIGHT: f32 = 50.0;
pub(super) const WIN_TILE_OUTLINE_MARGIN: f32 = 2.0;
const WIN_TILE_GAP: f32 = 14.0;
const WIN_MELD_GAP: f32 = 4.0;
const RESULT_INDICATOR_GAP: f32 = 2.0;
const RESULT_INDICATOR_LABEL_GAP: f32 = 6.0;
const RESULT_INDICATOR_SECTION_GAP: f32 = 18.0;
const WIN_PANEL_BASE_HEIGHT: f32 = 340.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct WinHandLayout {
    pub(super) tile_w: f32,
    pub(super) tile_h: f32,
    pub(super) row_width: f32,
    pub(super) top_overhang: f32,
}

fn win_hand_row_width_at_size(
    hand_len: usize,
    has_win_tile: bool,
    melds: &[Meld],
    tile_w: f32,
    tile_h: f32,
) -> f32 {
    let hand_width = hand_len as f32 * tile_w;
    let win_width = if has_win_tile {
        WIN_TILE_GAP + tile_w
    } else {
        0.0
    };
    let meld_width: f32 = melds
        .iter()
        .map(|meld| calc_meld_width(meld, tile_w, tile_h) + WIN_MELD_GAP)
        .sum();
    hand_width + win_width + meld_width
}

/// Chooses the largest winning-hand tiles that fit the result panel.
pub(super) fn win_hand_layout(
    hand_len: usize,
    has_win_tile: bool,
    melds: &[Meld],
) -> WinHandLayout {
    let natural_width = win_hand_row_width_at_size(
        hand_len,
        has_win_tile,
        melds,
        WIN_HAND_TILE_W,
        WIN_HAND_TILE_H,
    );
    let fixed_gap_width =
        if has_win_tile { WIN_TILE_GAP } else { 0.0 } + melds.len() as f32 * WIN_MELD_GAP;
    let scalable_width = natural_width - fixed_gap_width;
    let scale = if natural_width > WIN_PANEL_CONTENT_WIDTH && scalable_width > 0.0 {
        ((WIN_PANEL_CONTENT_WIDTH - fixed_gap_width) / scalable_width).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let tile_w = WIN_HAND_TILE_W * scale;
    let tile_h = WIN_HAND_TILE_H * scale;
    let row_width = win_hand_row_width_at_size(hand_len, has_win_tile, melds, tile_w, tile_h);
    let has_stacked_kakan = melds
        .iter()
        .any(|meld| meld.category == MeldType::Kakan && meld.tiles.len() > 3);
    let top_overhang = if has_stacked_kakan {
        (2.0 * tile_w - tile_h).max(0.0)
    } else {
        0.0
    };

    WinHandLayout {
        tile_w,
        tile_h,
        row_width,
        top_overhang,
    }
}

/// Returns a shared baseline that visually centers both columns in a yaku row.
pub(super) fn yaku_row_baseline(
    row_top: f32,
    row_height: f32,
    name_dimensions: TextDimensions,
    han_dimensions: Option<TextDimensions>,
) -> f32 {
    let mut ascent = name_dimensions.offset_y;
    let mut descent = name_dimensions.height - name_dimensions.offset_y;
    if let Some(dimensions) = han_dimensions {
        ascent = ascent.max(dimensions.offset_y);
        descent = descent.max(dimensions.height - dimensions.offset_y);
    }
    descent += theme::TEXT_SHADOW_OFFSET;

    row_top + (row_height + ascent - descent) / 2.0
}

/// Right-aligns text including the shadow drawn outside its measured width.
pub(super) fn yaku_right_aligned_x(right: f32, dimensions: TextDimensions) -> f32 {
    right - dimensions.width - theme::TEXT_SHADOW_OFFSET
}

/// The result overlay: a structured panel for wins, a message panel
/// for draws.
pub(super) fn draw_result(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    draw_rectangle(
        0.0,
        0.0,
        DESIGN_W,
        DESIGN_H,
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    if state.current_win_result().is_some() {
        draw_win_panel(state, font, tile_textures);
    } else {
        draw_draw_panel(state, font);
    }
}

/// Draws the gold-framed overlay panel and returns the content's
/// left/right bounds.
pub(super) fn draw_overlay_panel(panel_w: f32, panel_h: f32) -> (f32, f32, f32) {
    let panel_x = (DESIGN_W - panel_w) / 2.0;
    let panel_y = ((DESIGN_H - panel_h) / 2.0).max(24.0);
    theme::draw_panel(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        12.0,
        theme::rgba(0x050e08, 0.5),
        theme::GOLD_DK,
    );
    (panel_x, panel_y, panel_x + panel_w)
}

/// Draws the "next" prompt at the panel bottom.
pub(super) fn draw_result_next_button(
    state: &GameState,
    font: Option<&Font>,
    cx: f32,
    y: f32,
    w: f32,
) {
    let h = 40.0;
    let x = cx - w / 2.0;
    theme::draw_rounded_rect(x, y, w, h, 6.0, theme::rgba(0xc8a227, 0.10));
    theme::draw_rounded_rect_lines(x, y, w, h, 6.0, 1.0, theme::GOLD_DK);
    let label = if state.win_result_index + 1 < state.win_results.len() {
        state.tr().get(Key::NextWin)
    } else {
        state.tr().get(Key::Next)
    };
    theme::draw_text_centered(
        font,
        label,
        cx,
        y + 25.0,
        theme::font_size::LABEL,
        theme::GOLD_LT,
    );
}

/// Draws a horizontal row of indicator tiles (dora / ura dora).
pub(super) fn draw_indicator_row(
    tiles: &[Tile],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    tile_textures: &TileTextures,
) -> f32 {
    let mut cx = x;
    for tile in tiles {
        draw_tile_sprite(tile_textures.for_tile(tile), cx, y, w, h, WHITE);
        cx += w + RESULT_INDICATOR_GAP;
    }
    cx
}

/// Width reserved for the centered dora and optional ura-dora row.
pub(super) fn result_indicator_rows_width(
    dora_count: usize,
    ura_count: usize,
    dora_label_width: f32,
    ura_label_width: f32,
) -> f32 {
    let dora_width = dora_label_width
        + RESULT_INDICATOR_LABEL_GAP
        + dora_count as f32 * (RESULT_INDICATOR_TILE_W + RESULT_INDICATOR_GAP);
    let ura_width = if ura_count == 0 {
        0.0
    } else {
        RESULT_INDICATOR_SECTION_GAP
            + ura_label_width
            + RESULT_INDICATOR_LABEL_GAP
            + ura_count as f32 * (RESULT_INDICATOR_TILE_W + RESULT_INDICATOR_GAP)
    };
    dora_width + ura_width
}

/// The win-result panel.
pub(super) fn draw_win_panel(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let wr = match state.current_win_result() {
        Some(w) => w,
        None => return,
    };
    let mut dora_tile_types = dora_tile_types(&state.dora_indicators, state.is_three_player());
    add_dora_tile_types(
        &mut dora_tile_types,
        &state.uradora_indicators,
        state.is_three_player(),
    );
    let tile_tint = TileTintContext::new(None, &dora_tile_types);
    let tr = state.tr();
    let yaku_count = wr.yaku.len().max(1);
    let kyoutaku_points = wr.riichi_points();
    let has_bonus_line = kyoutaku_points > 0 || wr.honba_points > 0;
    let hand_layout = win_hand_layout(
        state.win_hand.len(),
        state.win_tile.is_some(),
        &state.win_melds,
    );
    let panel_w = WIN_PANEL_WIDTH;
    let panel_h = WIN_PANEL_BASE_HEIGHT
        + hand_layout.top_overhang
        + yaku_count as f32 * YAKU_ROW_HEIGHT
        + if has_bonus_line { 26.0 } else { 0.0 };
    let (panel_x, panel_y, _) = draw_overlay_panel(panel_w, panel_h);
    let cx = panel_x + panel_w / 2.0;
    let content_l = panel_x + WIN_PANEL_CONTENT_PADDING;
    let content_r = content_l + WIN_PANEL_CONTENT_WIDTH;
    let mut y = panel_y + 28.0;

    let type_label = if wr.win_is_tsumo {
        tr.get(Key::Tsumo)
    } else {
        tr.get(Key::Ron)
    };
    theme::draw_text_centered(
        font,
        type_label,
        cx,
        y,
        theme::font_size::SMALL,
        theme::GOLD,
    );
    y += 22.0;

    // The winner; a ron also names the deal-in player.
    let winner_line = match &wr.loser_name {
        Some(loser) => format!("{} ← {}", wr.winner_name, loser),
        None => wr.winner_name.clone(),
    };
    theme::draw_text_centered(
        font,
        &winner_line,
        cx,
        y,
        theme::font_size::HEADING_LARGE,
        theme::TEXT_BR,
    );
    y += 24.0;

    // The hand plus the winning tile, centered.
    let tw = hand_layout.tile_w;
    let th = hand_layout.tile_h;
    let mut x = cx - hand_layout.row_width / 2.0;
    let hand_y = y + hand_layout.top_overhang;
    for tile in &state.win_hand {
        draw_tile_sprite(
            tile_textures.for_tile(tile),
            x,
            hand_y,
            tw,
            th,
            dora_tile_tint(tile, &dora_tile_types),
        );
        x += tw;
    }
    if let Some(win_tile) = &state.win_tile {
        x += WIN_TILE_GAP;
        theme::draw_rounded_rect_lines(
            x - WIN_TILE_OUTLINE_MARGIN,
            hand_y - WIN_TILE_OUTLINE_MARGIN,
            tw + 2.0 * WIN_TILE_OUTLINE_MARGIN,
            th + 2.0 * WIN_TILE_OUTLINE_MARGIN,
            4.0,
            2.0,
            theme::GOLD_LT,
        );
        draw_tile_sprite(
            tile_textures.for_tile(win_tile),
            x,
            hand_y,
            tw,
            th,
            dora_tile_tint(win_tile, &dora_tile_types),
        );
        x += tw;
    }
    for meld in &state.win_melds {
        x += WIN_MELD_GAP;
        draw_meld_group(meld, x, hand_y, tw, th, tile_textures, tile_tint);
        x += calc_meld_width(meld, tw, th);
    }
    y = hand_y + WIN_HAND_ROW_HEIGHT;

    let dw = RESULT_INDICATOR_TILE_W;
    let dh = RESULT_INDICATOR_TILE_H;
    let dora_text = DoraLabel::Dora.name(state.lang);
    let ura_text = DoraLabel::UraDora.name(state.lang);
    let dora_label_w = theme::measure_text_size(font, dora_text, theme::font_size::CAPTION).width;
    let ura_label_w = theme::measure_text_size(font, ura_text, theme::font_size::CAPTION).width;
    let total_w = result_indicator_rows_width(
        state.dora_indicators.len(),
        state.uradora_indicators.len(),
        dora_label_w,
        ura_label_w,
    );
    let mut dx = cx - total_w / 2.0;
    draw_jp_text(
        font,
        dora_text,
        dx,
        y + dh / 2.0 + 4.0,
        theme::font_size::CAPTION,
        theme::TEXT_DIM,
    );
    dx += dora_label_w + RESULT_INDICATOR_LABEL_GAP;
    dx = draw_indicator_row(&state.dora_indicators, dx, y, dw, dh, tile_textures);
    if !state.uradora_indicators.is_empty() {
        dx += RESULT_INDICATOR_SECTION_GAP;
        draw_jp_text(
            font,
            ura_text,
            dx,
            y + dh / 2.0 + 4.0,
            theme::font_size::CAPTION,
            theme::TEXT_DIM,
        );
        dx += ura_label_w + RESULT_INDICATOR_LABEL_GAP;
        draw_indicator_row(&state.uradora_indicators, dx, y, dw, dh, tile_textures);
    }
    y += RESULT_INDICATOR_ROW_HEIGHT;

    // The yaku list, separated by a rule.
    draw_rectangle(content_l, y, content_r - content_l, 1.0, theme::BORDER);
    y += 8.0;
    for (name, han) in &wr.yaku {
        let name_dimensions = theme::measure_text_size(font, name, YAKU_FONT_SIZE);
        let han_layout = if wr.yakuman_multiplier == 0 {
            let han_text = tr.han(*han);
            let dimensions = theme::measure_text_size(font, &han_text, YAKU_FONT_SIZE);
            Some((han_text, dimensions))
        } else {
            None
        };
        let baseline = yaku_row_baseline(
            y,
            YAKU_ROW_HEIGHT,
            name_dimensions,
            han_layout.as_ref().map(|(_, dimensions)| *dimensions),
        );

        draw_jp_text(font, name, content_l, baseline, YAKU_FONT_SIZE, theme::TEXT);
        if let Some((han_text, dimensions)) = han_layout {
            draw_jp_text(
                font,
                &han_text,
                yaku_right_aligned_x(content_r, dimensions),
                baseline,
                YAKU_FONT_SIZE,
                theme::GOLD_LT,
            );
        }
        draw_rectangle(
            content_l,
            y + YAKU_ROW_HEIGHT,
            content_r - content_l,
            1.0,
            theme::rgba(0xffffff, 0.04),
        );
        y += YAKU_ROW_HEIGHT;
    }
    if wr.yaku.is_empty() {
        y += YAKU_ROW_HEIGHT;
    }
    y += 8.0;

    // Totals: non-yakuman han/fu on the left, rank + big score on the right.
    if wr.rank != ScoreRank::Yakuman {
        let hanfu = tr.han_fu(wr.han, wr.fu);
        draw_jp_text(
            font,
            &hanfu,
            content_l,
            y + 24.0,
            theme::font_size::LABEL,
            theme::TEXT_DIM,
        );
    }

    // Keep the hand score separate from table bonuses, whose breakdown is
    // shown on the line below.
    let pts = tr.points(&format_score(wr.hand_points()));
    let pw = theme::measure_text_size(font, &pts, theme::font_size::DISPLAY).width;
    let pts_x = content_r - pw;
    draw_jp_text(
        font,
        &pts,
        pts_x,
        y + 28.0,
        theme::font_size::DISPLAY,
        theme::GOLD_LT,
    );

    // Mangan-and-up rank names match the score's size and color.
    if !wr.rank_name.is_empty() {
        let rw = theme::measure_text_size(font, &wr.rank_name, theme::font_size::DISPLAY).width;
        draw_jp_text(
            font,
            &wr.rank_name,
            pts_x - 14.0 - rw,
            y + 28.0,
            theme::font_size::DISPLAY,
            theme::GOLD_LT,
        );
    }
    y += 44.0;

    if has_bonus_line {
        y += 6.0;
        let mut bonus_parts = Vec::with_capacity(2);
        if kyoutaku_points > 0 {
            bonus_parts.push(tr.deposit_points(wr.riichi_sticks, &format_score(kyoutaku_points)));
        }
        if wr.honba_points > 0 {
            bonus_parts.push(tr.honba_points(wr.honba, &format_score(wr.honba_points)));
        }
        let bonus_text = bonus_parts.join(" ");
        let bw = theme::measure_text_size(font, &bonus_text, theme::font_size::LABEL).width;
        draw_jp_text(
            font,
            &bonus_text,
            content_r - bw,
            y,
            theme::font_size::LABEL,
            theme::TEXT_DIM,
        );
        y += 20.0;
    }

    draw_result_next_button(state, font, cx, y, panel_w - 80.0);
}

/// The draw panel.
pub(super) fn draw_draw_panel(state: &GameState, font: Option<&Font>) {
    let heading = state.message_result_heading();
    let lines: Vec<&str> = state
        .result_message
        .as_deref()
        .unwrap_or(heading)
        .lines()
        .collect();
    let panel_w = 560.0;
    let panel_h = 140.0 + lines.len() as f32 * 30.0;
    let (panel_x, panel_y, _) = draw_overlay_panel(panel_w, panel_h);
    let cx = panel_x + panel_w / 2.0;
    let mut y = panel_y + 40.0;

    theme::draw_text_centered(font, heading, cx, y, theme::font_size::SMALL, theme::GOLD);
    y += 30.0;
    for (i, line) in lines.iter().enumerate() {
        let (font_size, color) = if i == 0 {
            (theme::font_size::HEADING, theme::TEXT_BR)
        } else {
            (theme::font_size::LABEL, theme::TEXT)
        };
        theme::draw_text_centered(font, line, cx, y, font_size, color);
        y += 30.0;
    }
    y += 4.0;
    draw_result_next_button(state, font, cx, y, panel_w - 80.0);
}

pub(super) fn draw_game_over(state: &GameState, font: Option<&Font>) {
    draw_setup_background();

    let panel_w = 620.0;
    let panel_h = 420.0;
    let panel_x = (DESIGN_W - panel_w) / 2.0;
    let panel_y = (DESIGN_H - panel_h) / 2.0;
    theme::draw_panel(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        12.0,
        theme::PANEL_BG,
        theme::PANEL_BORDER,
    );
    let cx = panel_x + panel_w / 2.0;
    let tr = state.tr();

    theme::draw_text_centered(
        font,
        tr.get(Key::GameOver),
        cx,
        panel_y + 52.0,
        theme::font_size::TITLE,
        theme::TEXT_BR,
    );

    // Final standings; ties favor the seat nearer the starting dealer,
    // and three-player games skip the dummy seat.
    let rankings = state.rankings();

    // Rank colors: gold, silver, bronze, rest.
    let rank_colors = [
        theme::rgb_pub(0xe8c84a),
        theme::rgb_pub(0xb8c4cc),
        theme::rgb_pub(0xc48c60),
        theme::rgb_pub(0x708090),
    ];

    let row_x = panel_x + 40.0;
    let row_w = panel_w - 80.0;
    let row_h = 48.0;
    let row_gap = 10.0;
    let mut ry = panel_y + 92.0;

    for (rank, (player_idx, score)) in rankings.iter().enumerate() {
        let is_me = *player_idx == state.my_seat;
        let rc = rank_colors[rank.min(3)];

        let (fill, border) = if is_me {
            (theme::rgba(0xc8a227, 0.07), theme::rgba(0xc8a227, 0.20))
        } else {
            (
                Color::new(1.0, 1.0, 1.0, 0.03),
                Color::new(1.0, 1.0, 1.0, 0.05),
            )
        };
        theme::draw_rounded_rect(row_x, ry, row_w, row_h, 6.0, fill);
        theme::draw_rounded_rect_lines(row_x, ry, row_w, row_h, 6.0, 1.0, border);

        draw_jp_text(
            font,
            &format!("{}", rank + 1),
            row_x + 16.0,
            ry + 32.0,
            theme::font_size::TITLE_SMALL,
            rc,
        );
        draw_jp_text(
            font,
            tr.place_suffix(rank),
            row_x + 34.0,
            ry + 32.0,
            theme::font_size::CAPTION,
            rc,
        );

        // Names; CPU numbers match the score chips' relative order.
        let cpu_number = (*player_idx + state.player_count - state.my_seat) % state.player_count;
        let name = state.player_labels[*player_idx].name(cpu_number, state.lang);
        draw_jp_text(
            font,
            &name,
            row_x + 64.0,
            ry + 30.0,
            theme::font_size::LABEL,
            theme::TEXT,
        );

        let pts = tr.points(&format_score(*score));
        let pw = theme::measure_text_size(font, &pts, theme::font_size::SUBHEADING).width;
        draw_jp_text(
            font,
            &pts,
            row_x + row_w - pw - 8.0,
            ry + 32.0,
            theme::font_size::SUBHEADING,
            rc,
        );

        ry += row_h + row_gap;
    }

    let btn_w = 200.0;
    let btn_h = 50.0;
    let btn_x = cx - btn_w / 2.0;
    let btn_y = panel_y + panel_h - btn_h - 24.0;
    theme::draw_gradient_button(
        btn_x,
        btn_y,
        btn_w,
        btn_h,
        8.0,
        theme::rgb_pub(0x9a7a1a),
        theme::rgb_pub(0x6a5210),
        theme::GOLD,
        2.0,
    );
    theme::draw_text_centered(
        font,
        tr.get(Key::PlayAgain),
        cx,
        btn_y + 31.0,
        theme::font_size::BODY_LARGE,
        theme::GOLD_LT,
    );
}
