//! 局結果・対局終了オーバーレイの描画

use super::*;

/// 局結果オーバーレイ。和了は構造化パネル、流局はメッセージパネルを描画する。
pub(super) fn draw_result(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    // 全面を暗くする
    draw_rectangle(
        0.0,
        0.0,
        DESIGN_W,
        DESIGN_H,
        Color::new(0.0, 0.0, 0.0, 0.78),
    );

    if state.current_win_result().is_some() {
        draw_win_panel(state, font, tile_textures);
    } else {
        draw_draw_panel(state, font);
    }
}

/// 角丸ゴールド枠のオーバーレイパネルを描き、内側コンテンツ用の左端・右端を返す。
pub(super) fn draw_overlay_panel(panel_w: f32, panel_h: f32) -> (f32, f32, f32) {
    let panel_x = (DESIGN_W - panel_w) / 2.0;
    let panel_y = ((DESIGN_H - panel_h) / 2.0).max(24.0);
    theme::draw_panel(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        12.0,
        theme::rgba(0x050e08, 0.97),
        theme::GOLD_DK,
    );
    (panel_x, panel_y, panel_x + panel_w)
}

/// 結果パネル下部の「次へ」誘導ボタンを描画する。
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
    theme::draw_text_centered(font, label, cx, y + 25.0, 14, theme::GOLD_LT);
}

/// 横一列の表示牌を描く（ドラ／裏ドラ用）。
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
        cx += w + 2.0;
    }
    cx
}

/// 和了結果パネル。
pub(super) fn draw_win_panel(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let wr = match state.current_win_result() {
        Some(w) => w,
        None => return,
    };
    let tr = state.tr();
    let yaku_count = wr.yaku.len().max(1);
    let panel_w = 700.0;
    let panel_h = 326.0 + yaku_count as f32 * 22.0;
    let (panel_x, panel_y, panel_right) = draw_overlay_panel(panel_w, panel_h);
    let cx = panel_x + panel_w / 2.0;
    let content_l = panel_x + 40.0;
    let content_r = panel_right - 40.0;
    let mut y = panel_y + 28.0;

    // 種別（ツモ / ロン）
    let type_label = if wr.win_is_tsumo {
        tr.get(Key::Tsumo)
    } else {
        tr.get(Key::Ron)
    };
    theme::draw_text_centered(font, type_label, cx, y, 12, theme::GOLD);
    y += 22.0;

    // 和了者（ロンは放銃者を併記）
    let winner_line = match &wr.loser_name {
        Some(loser) => format!("{} ← {}", wr.winner_name, loser),
        None => wr.winner_name.clone(),
    };
    theme::draw_text_centered(font, &winner_line, cx, y, 21, theme::TEXT_BR);
    y += 24.0;

    // 手牌＋和了牌（センタリング）
    let tw = 26.0;
    let th = 36.0;
    let win_gap = 14.0;
    let meld_gap = 4.0;
    let hand_w = state.win_hand.len() as f32 * tw;
    let win_w = if state.win_tile.is_some() {
        win_gap + tw
    } else {
        0.0
    };
    let meld_w: f32 = state
        .win_melds
        .iter()
        .map(|m| calc_meld_width(m, tw, th) + meld_gap)
        .sum();
    let row_w = hand_w + win_w + meld_w;
    let mut x = cx - row_w / 2.0;
    let hand_y = y;
    for tile in &state.win_hand {
        draw_tile_sprite(tile_textures.for_tile(tile), x, hand_y, tw, th, WHITE);
        x += tw;
    }
    if let Some(win_tile) = &state.win_tile {
        x += win_gap;
        theme::draw_rounded_rect_lines(
            x - 2.0,
            hand_y - 2.0,
            tw + 4.0,
            th + 4.0,
            4.0,
            2.0,
            theme::GOLD_LT,
        );
        draw_tile_sprite(tile_textures.for_tile(win_tile), x, hand_y, tw, th, WHITE);
        x += tw;
    }
    for meld in &state.win_melds {
        x += meld_gap;
        draw_meld_group(meld, x, hand_y, tw, th, tile_textures);
        x += calc_meld_width(meld, tw, th);
    }
    y = hand_y + th + 20.0;

    // ドラ・裏ドラ
    let dw = 20.0;
    let dh = 28.0;
    let dora_text = DoraLabel::Dora.name(state.lang);
    let ura_text = DoraLabel::UraDora.name(state.lang);
    let dora_label_w = theme::measure_scaled(font, dora_text, 11).width;
    let dora_tiles_w = state.dora_indicators.len() as f32 * (dw + 2.0);
    let ura_block_w = if state.uradora_indicators.is_empty() {
        0.0
    } else {
        24.0 + theme::measure_scaled(font, ura_text, 11).width
            + 6.0
            + state.uradora_indicators.len() as f32 * (dw + 2.0)
    };
    let total_w = dora_label_w + 6.0 + dora_tiles_w + ura_block_w;
    let mut dx = cx - total_w / 2.0;
    draw_jp_text(font, dora_text, dx, y + dh / 2.0 + 4.0, 11, theme::TEXT_DIM);
    dx += dora_label_w + 6.0;
    dx = draw_indicator_row(&state.dora_indicators, dx, y, dw, dh, tile_textures);
    if !state.uradora_indicators.is_empty() {
        dx += 18.0;
        draw_jp_text(font, ura_text, dx, y + dh / 2.0 + 4.0, 11, theme::TEXT_DIM);
        dx += theme::measure_scaled(font, ura_text, 11).width + 6.0;
        draw_indicator_row(&state.uradora_indicators, dx, y, dw, dh, tile_textures);
    }
    y += dh + 16.0;

    // 役一覧（上に区切り線）
    draw_rectangle(content_l, y, content_r - content_l, 1.0, theme::BORDER);
    y += 8.0;
    for (name, han) in &wr.yaku {
        draw_jp_text(font, name, content_l, y + 14.0, 14, theme::TEXT);
        let han_text = tr.han(*han);
        let hw = theme::measure_scaled(font, &han_text, 14).width;
        draw_jp_text(
            font,
            &han_text,
            content_r - hw,
            y + 14.0,
            14,
            theme::GOLD_LT,
        );
        draw_rectangle(
            content_l,
            y + 22.0,
            content_r - content_l,
            1.0,
            theme::rgba(0xffffff, 0.04),
        );
        y += 22.0;
    }
    if wr.yaku.is_empty() {
        y += 22.0;
    }
    y += 8.0;

    // 合計（左に飜符の小さな表示、右に等級名＋大きな点数）
    let mut hanfu = tr.han_fu(wr.han, wr.fu);
    if wr.riichi_sticks > 0 {
        hanfu.push_str("  ");
        hanfu.push_str(&tr.riichi_deposit(wr.riichi_sticks));
    }
    draw_jp_text(font, &hanfu, content_l, y + 24.0, 13, theme::TEXT_DIM);

    // 大きな点数（右寄せ）
    let pts = tr.points(&format_score(wr.score_points));
    let pw = theme::measure_scaled(font, &pts, 28).width;
    let pts_x = content_r - pw;
    draw_jp_text(font, &pts, pts_x, y + 28.0, 28, theme::GOLD_LT);

    // 満貫以上の等級名は点数と同じ大きさ・色で点数の左に表示
    if !wr.rank_name.is_empty() {
        let rw = theme::measure_scaled(font, &wr.rank_name, 28).width;
        draw_jp_text(
            font,
            &wr.rank_name,
            pts_x - 14.0 - rw,
            y + 28.0,
            28,
            theme::GOLD_LT,
        );
    }
    y += 44.0;

    draw_result_next_button(state, font, cx, y, panel_w - 80.0);
}

/// 流局パネル。
pub(super) fn draw_draw_panel(state: &GameState, font: Option<&Font>) {
    let tr = state.tr();
    let lines: Vec<&str> = state
        .result_message
        .as_deref()
        .unwrap_or_else(|| tr.get(Key::RoundDraw))
        .lines()
        .collect();
    let panel_w = 560.0;
    let panel_h = 140.0 + lines.len() as f32 * 30.0;
    let (panel_x, panel_y, _) = draw_overlay_panel(panel_w, panel_h);
    let cx = panel_x + panel_w / 2.0;
    let mut y = panel_y + 40.0;

    theme::draw_text_centered(font, tr.get(Key::RoundDraw), cx, y, 12, theme::GOLD);
    y += 30.0;
    for (i, line) in lines.iter().enumerate() {
        let (size, color) = if i == 0 {
            (20, theme::TEXT_BR)
        } else {
            (14, theme::TEXT)
        };
        theme::draw_text_centered(font, line, cx, y, size, color);
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
        26,
        theme::TEXT_BR,
    );

    // 最終順位（同点は起家に近い席が上位。三麻はダミー席を除外）
    let rankings = state.final_rankings();

    // 順位の色（金・銀・銅・その他）
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

        // 順位
        draw_jp_text(
            font,
            &format!("{}", rank + 1),
            row_x + 16.0,
            ry + 32.0,
            24,
            rc,
        );
        draw_jp_text(font, tr.place_suffix(rank), row_x + 34.0, ry + 32.0, 11, rc);

        // 名前（CPU 番号は得点チップと同じ自分からの相対位置）
        let cpu_number = (*player_idx + state.player_count - state.my_seat) % state.player_count;
        let name = state.player_labels[*player_idx].name(cpu_number, state.lang);
        draw_jp_text(font, &name, row_x + 64.0, ry + 30.0, 14, theme::TEXT);

        // 得点
        let pts = tr.points(&format_score(*score));
        let pw = theme::measure_scaled(font, &pts, 17).width;
        draw_jp_text(font, &pts, row_x + row_w - pw - 8.0, ry + 32.0, 17, rc);

        ry += row_h + row_gap;
    }

    // もう一度ボタン
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
        16,
        theme::GOLD_LT,
    );
}
