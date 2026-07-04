//! 牌・手牌・副露の描画

use super::*;

pub(super) fn draw_hand(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let hand_start_x = player_hand_start_x(state.hand.len());
    let hand_y = HAND_Y;
    let tr = state.tr();

    // 先に手牌を描画する（バッジは牌に隠れないよう後で描画する）。
    for (i, tile) in state.hand.iter().enumerate() {
        let x = hand_start_x + i as f32 * TILE_W;
        let selected = state.selected_tile == Some(i);
        let riichi_selectable =
            state.riichi_selection_mode && state.riichi_selectable_tiles.contains(&i);
        let y_offset = if selected { -14.0 } else { 0.0 };
        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;
        // 喰い替え禁止牌は打牌できないので無効表示する
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

        // 「ツモ」ラベル
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

    // 状態バッジ（フリテン・リーチ中・リーチ打牌選択中・喰い替え警告）は
    // 牌に隠れないよう、手牌を描画したあとに重ねて描く。
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

/// 選択中の牌の周囲にゴールドの縁取りを描く。
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

    // 最初に鳴いた牌を右端に、後の牌ほど左へ並べる。
    let xs = self_meld_x_positions(&state.melds, tw, th, meld_gap, right_edge);
    for (meld, &x) in state.melds.iter().zip(&xs) {
        draw_meld_group(meld, x, meld_y, tw, th, tile_textures);
    }
}

/// 自分の副露を「最初に鳴いた牌ほど右」に並べたときの各副露の左端 x 座標を返す。
///
/// 返り値は `melds` と同じ順（index 0 = 最初に鳴いた牌）。`right_edge` を右端として
/// 右→左に詰めていくため、`xs[0]` が最も大きく（右）、後の副露ほど小さく（左）なる。
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

/// 横向きの副露牌を描画する（90°回転）
pub(super) fn draw_meld_tile_sideways(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    tw: f32,
    th: f32,
    tile_textures: &TileTextures,
) {
    // 横向き牌のバウンディングボックス: 幅=th, 高さ=tw
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

/// 裏向きの副露牌を描画する（暗槓用）
pub(super) fn draw_meld_tile_back(x: f32, y: f32, w: f32, h: f32, tile_textures: &TileTextures) {
    draw_tile_sprite(&tile_textures.back, x, y, w - 2.0, h - 2.0, WHITE);
}

/// 鳴き元に応じて横向き牌の位置インデックスを返す
pub(super) fn sideways_index(from: MeldFrom, tile_count: usize) -> usize {
    match from {
        MeldFrom::Previous => 0,               // 上家: 左端
        MeldFrom::Opposite => 1,               // 対面: 左から2番目
        MeldFrom::Following => tile_count - 1, // 下家: 右端
        _ => 0,                                // Unknown/Myself: フォールバック
    }
}

/// 副露グループの描画幅を計算する
pub(super) fn calc_meld_width(meld: &Meld, tw: f32, th: f32) -> f32 {
    match meld.category {
        MeldType::Kan if meld.from == MeldFrom::Myself => {
            // 暗槓: 4枚すべて縦向き
            4.0 * tw
        }
        MeldType::Kakan => {
            // 加槓: 横向き牌の位置に2枚重ね（幅はth）、残りは縦向き
            2.0 * tw + th
        }
        MeldType::Chi | MeldType::Pon => {
            // チー/ポン: 1枚横向き（幅th）、残り2枚縦向き
            2.0 * tw + th
        }
        MeldType::Kan => {
            // 大明槓: 1枚横向き（幅th）、残り3枚縦向き
            3.0 * tw + th
        }
    }
}

/// 副露グループを描画する
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
            // 暗槓: 1,4枚目裏向き、2,3枚目表向き、全て縦向き
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
            // チー: 鳴いた牌を左端に横向き、残り2枚を順番に縦向き
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
            // ポン: 鳴き元に応じて横向き牌の位置を決定
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
            // 大明槓: 鳴き元に応じて横向き牌の位置を決定（4枚）
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
            // 加槓: ポンの横向き位置に2枚重ね
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

/// 回転付きで牌スプライトを描画する
///
/// (vx, vy) は回転後の「見た目上の左上」座標。
/// テクスチャは常に自然なアスペクト比 (w, h) で描画し、
/// 回転による描画座標のずれを内部で補正する。
pub(super) fn draw_tile_sprite_rotated(
    texture: &Texture2D,
    vx: f32,
    vy: f32,
    w: f32,
    h: f32,
    tint: Color,
    rotation: f32,
) {
    // 90度回転時、バウンディングボックスの左上が (w, h) の矩形中心を基準にずれる。
    // 回転後の見た目サイズ: 0°/180° → (w, h), ±90° → (h, w)
    // draw座標 = visual座標 + 補正
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

/// 他プレイヤー（CPU）の手牌を描画する
///
/// 捨て牌と同様に、正規化された「自分」視点（左→右）で描画し、
/// Camera2D で盤面中心を軸に回転させて各家の位置に配置する。
pub(super) fn draw_other_player_hands(state: &GameState, tile_textures: &TileTextures) {
    let tw: f32 = 28.0; // 牌の自然な幅
    let th: f32 = 40.0; // 牌の自然な高さ
    let meld_gap: f32 = 6.0;
    let tile_step: f32 = tw; // 牌同士がくっつく（隙間なし）
    let hand_distance: f32 = 290.0; // 中心から手牌までの距離

    let base_y = BOARD_CENTER_Y + hand_distance;

    for other_idx in 0..(state.player_count - 1) {
        let relative_idx = other_idx + 1; // 1=下家, 2=対面, 3=上家（三麻では 1=下家, 2=上家）
        let other = &state.other_players[other_idx];

        // 手牌＋副露の合計幅を計算してセンタリング
        let hand_count = if other.revealed {
            other.hand.len()
        } else {
            other.concealed_count
        };
        let meld_widths: f32 = other.melds.iter().map(|m| calc_meld_width(m, tw, th)).sum();
        let meld_gaps = if other.melds.is_empty() {
            0.0
        } else {
            meld_gap + (other.melds.len() as f32 - 1.0) * meld_gap
        };
        let total_width = hand_count as f32 * tile_step + meld_widths + meld_gaps;
        let start_x = BOARD_CENTER_X - total_width / 2.0;

        set_camera(&make_board_camera(
            PLAYER_ROTATIONS[rotation_index(relative_idx, state.player_count)],
        ));

        // 手牌描画（左→右）
        let mut x = start_x;
        if other.revealed {
            for tile in &other.hand {
                draw_tile_sprite(tile_textures.for_tile(tile), x, base_y, tw, th, WHITE);
                x += tile_step;
            }
        } else {
            for _ in 0..other.concealed_count {
                draw_tile_sprite(&tile_textures.back, x, base_y, tw, th, WHITE);
                x += tile_step;
            }
        }

        // 副露描画（手牌の続き）
        if !other.melds.is_empty() {
            x += meld_gap;
        }
        let xs = other_meld_x_positions(&other.melds, tw, th, meld_gap, x);
        for (meld, &mx) in other.melds.iter().zip(&xs) {
            draw_meld_group(meld, mx, base_y, tw, th, tile_textures);
        }

        set_design_camera();
    }
}

/// 他家の副露を、そのプレイヤーから見て「最初に鳴いた牌ほど右」に並べたときの
/// 各副露の左端 x 座標を返す。
///
/// 返り値は `melds` と同じ順（index 0 = 最初に鳴いた牌）。手牌の右隣の `start_x` を
/// 左端として左→右に詰めるが、最初に鳴いた牌が右端（x が大きい側）へ来るよう逆順で
/// 配置するため、`xs[0]` が最も大きくなる。
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
