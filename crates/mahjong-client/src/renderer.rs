//! 描画モジュール
//!
//! テキストベースのシンプルな麻雀卓UI描画。
//! MVPではテキストで牌を表示する。

use macroquad::prelude::*;

use mahjong_server::protocol::AvailableCall;

use crate::game::{GamePhase, GameState};

/// 牌を描画する色
const TILE_BG: Color = Color::new(1.0, 1.0, 0.9, 1.0); // アイボリー
const TILE_BORDER: Color = Color::new(0.3, 0.3, 0.3, 1.0);
const TILE_TEXT: Color = Color::new(0.1, 0.1, 0.1, 1.0);
const TILE_RED: Color = Color::new(0.9, 0.1, 0.1, 1.0);
const SELECTED_BG: Color = Color::new(0.8, 1.0, 0.8, 1.0);
const RIICHI_SELECTABLE_BG: Color = Color::new(1.0, 0.96, 0.72, 1.0);
const RIICHI_DISABLED_BG: Color = Color::new(0.78, 0.78, 0.72, 1.0);
const RIICHI_DISABLED_TEXT: Color = Color::new(0.45, 0.45, 0.42, 1.0);

const TILE_W: f32 = 48.0;
const TILE_H: f32 = 68.0;
const FONT_SIZE: u16 = 20;
const SMALL_FONT: u16 = 16;

/// テキスト描画ヘルパー（カスタムフォント対応）
fn draw_jp_text(font: Option<&Font>, text: &str, x: f32, y: f32, font_size: u16, color: Color) {
    let params = TextParams {
        font,
        font_size,
        color,
        ..Default::default()
    };
    draw_text_ex(text, x, y, params);
}

/// ゲーム全体を描画する
pub fn draw_game(state: &GameState, font: Option<&Font>) {
    match state.phase {
        GamePhase::WaitingForStart => {
            draw_jp_text(font, "ゲーム開始中...", 540.0, 400.0, 30, WHITE);
        }
        GamePhase::Playing => {
            draw_info_panel(state, font);
            draw_discards(state, font);
            draw_hand(state, font);
            draw_melds(state, font);
            draw_action_buttons(state, font);
        }
        GamePhase::RoundResult => {
            draw_info_panel(state, font);
            draw_discards(state, font);
            draw_hand(state, font);
            draw_melds(state, font);
            draw_result(state, font);
        }
        GamePhase::GameOver => {
            draw_game_over(state, font);
        }
    }
}

/// 情報パネル（場風、自風、残り枚数、点数）
fn draw_info_panel(state: &GameState, font: Option<&Font>) {
    // 背景
    draw_rectangle(0.0, 0.0, 1280.0, 50.0, Color::new(0.0, 0.0, 0.0, 0.5));

    let seat = match state.seat_wind {
        Some(w) => wind_to_str(w),
        None => "?",
    };

    // 局表示（東1局 0本場 など）
    let round_wind = match state.round_number / 4 {
        0 => "東",
        1 => "南",
        2 => "西",
        _ => "北",
    };
    let round_num = (state.round_number % 4) + 1;
    let honba_text = if state.honba > 0 {
        format!(" {}本場", state.honba)
    } else {
        String::new()
    };
    let riichi_marker = if state.is_riichi {
        " 【リーチ】"
    } else {
        ""
    };

    draw_jp_text(
        font,
        &format!(
            "{}{}局{}  自風: {}  残り: {}枚{}",
            round_wind, round_num, honba_text, seat, state.remaining_tiles, riichi_marker
        ),
        20.0,
        35.0,
        FONT_SIZE,
        WHITE,
    );

    // 点数表示
    let wind_names = ["東", "南", "西", "北"];
    let mut score_text = String::new();
    for i in 0..4 {
        if i > 0 {
            score_text.push_str("  ");
        }
        score_text.push_str(&format!("{}:{}", wind_names[i], state.scores[i]));
    }
    draw_jp_text(font, &score_text, 600.0, 35.0, SMALL_FONT, WHITE);

    // ドラ表示
    if !state.dora_indicators.is_empty() {
        let dora_text: Vec<String> = state
            .dora_indicators
            .iter()
            .map(|t| t.to_string())
            .collect();
        draw_jp_text(
            font,
            &format!("ドラ表示: {}", dora_text.join(" ")),
            20.0,
            70.0,
            SMALL_FONT,
            Color::new(1.0, 0.9, 0.3, 1.0),
        );
    }
}

/// 捨て牌を描画する
fn draw_discards(state: &GameState, font: Option<&Font>) {
    let positions: [(f32, f32); 4] = [
        (400.0, 500.0), // 自分（下）
        (900.0, 300.0), // 下家（右）
        (400.0, 100.0), // 対面（上）
        (100.0, 300.0), // 上家（左）
    ];

    let my_wind_idx = state.seat_wind.map(|w| w.to_index()).unwrap_or(0);

    for player_idx in 0..4 {
        let (base_x, base_y) = positions[player_idx];
        let discards = &state.discards[player_idx];
        let display_wind = mahjong_core::tile::Wind::from_index(my_wind_idx + player_idx);
        let score = state.scores[player_idx];
        let label = format!("{} {}点", wind_to_str(display_wind), score);

        draw_jp_text(
            font,
            &label,
            base_x,
            base_y - 5.0,
            SMALL_FONT,
            Color::new(0.8, 0.8, 0.8, 1.0),
        );

        for (i, discard) in discards.iter().enumerate() {
            let col = i % 6;
            let row = i / 6;
            let x = base_x + col as f32 * 36.0;
            let y = base_y + row as f32 * 30.0;

            let text = discard.tile.to_string();
            let color = if discard.is_tsumogiri {
                Color::new(0.7, 0.7, 0.7, 1.0)
            } else {
                WHITE
            };
            draw_jp_text(font, &text, x, y + 20.0, SMALL_FONT, color);
        }
    }
}

/// 手牌を描画する
fn draw_hand(state: &GameState, font: Option<&Font>) {
    let hand_start_x = 100.0;
    let hand_y = 680.0;

    // 手牌
    for (i, tile) in state.hand.iter().enumerate() {
        let x = hand_start_x + i as f32 * TILE_W;
        let selected = state.selected_tile == Some(i);
        let riichi_selectable =
            state.riichi_selection_mode && state.riichi_selectable_tiles.contains(&i);
        let y_offset = if selected { -10.0 } else { 0.0 };

        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;
        draw_tile(
            x,
            hand_y + y_offset,
            tile,
            selected,
            riichi_selectable,
            riichi_disabled,
            font,
        );
    }

    // ツモ牌（少し間隔を開けて表示）
    if let Some(drawn) = &state.drawn {
        let drawn_x = hand_start_x + state.hand.len() as f32 * TILE_W + 20.0;
        let selected = state.selected_drawn;
        let riichi_selectable = state.riichi_selection_mode && state.riichi_selectable_drawn;
        let y_offset = if selected { -10.0 } else { 0.0 };
        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;
        draw_tile(
            drawn_x,
            hand_y + y_offset,
            drawn,
            selected,
            riichi_selectable,
            riichi_disabled,
            font,
        );

        // ツモ牌ラベル
        draw_jp_text(
            font,
            "ツモ",
            drawn_x,
            hand_y + y_offset - 5.0,
            SMALL_FONT,
            Color::new(1.0, 0.9, 0.3, 1.0),
        );
    }
}

/// 副露（鳴き）を手牌の右端に描画する
fn draw_melds(state: &GameState, font: Option<&Font>) {
    if state.melds.is_empty() {
        return;
    }

    let meld_tile_w: f32 = 40.0;
    let meld_tile_h: f32 = 56.0;
    let meld_y: f32 = 692.0; // 手牌より少し下に揃える
    let meld_gap: f32 = 12.0; // 副露グループ間の間隔

    // 右端から左へ描画
    let mut x = 1220.0;

    for meld in state.melds.iter().rev() {
        let tile_count = meld.tiles.len();
        let meld_width = tile_count as f32 * meld_tile_w;
        x -= meld_width;

        for (i, tile) in meld.tiles.iter().enumerate() {
            let tx = x + i as f32 * meld_tile_w;
            draw_meld_tile(tx, meld_y, tile, meld_tile_w, meld_tile_h, font);
        }

        x -= meld_gap;
    }
}

/// 副露の牌1枚を描画する（少し小さめ）
fn draw_meld_tile(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    w: f32,
    h: f32,
    font: Option<&Font>,
) {
    let bg = Color::new(0.9, 0.95, 1.0, 1.0); // 薄い青系で副露を区別
    draw_rectangle(x, y, w - 2.0, h - 2.0, bg);
    draw_rectangle_lines(x, y, w - 2.0, h - 2.0, 2.0, TILE_BORDER);

    let text = tile.to_string();
    let color = if tile.is_red_dora() {
        TILE_RED
    } else {
        TILE_TEXT
    };

    let chars: Vec<char> = text.chars().collect();
    if chars.len() >= 2 {
        let num_str = chars[0].to_string();
        let suit_str = chars[1].to_string();
        draw_jp_text(font, &num_str, x + 10.0, y + 22.0, 20, color);
        draw_jp_text(font, &suit_str, x + 10.0, y + 42.0, 20, color);
    } else {
        draw_jp_text(font, &text, x + 6.0, y + 34.0, 20, color);
    }
}

/// 牌1枚を描画する
fn draw_tile(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    selected: bool,
    riichi_selectable: bool,
    riichi_disabled: bool,
    font: Option<&Font>,
) {
    // 背景
    let bg = if selected {
        SELECTED_BG
    } else if riichi_selectable {
        RIICHI_SELECTABLE_BG
    } else if riichi_disabled {
        RIICHI_DISABLED_BG
    } else {
        TILE_BG
    };
    draw_rectangle(x, y, TILE_W - 2.0, TILE_H - 2.0, bg);
    draw_rectangle_lines(x, y, TILE_W - 2.0, TILE_H - 2.0, 2.0, TILE_BORDER);

    // 牌の文字列
    let text = tile.to_string();
    let color = if riichi_disabled {
        RIICHI_DISABLED_TEXT
    } else if tile.is_red_dora() {
        TILE_RED
    } else {
        TILE_TEXT
    };

    // 2文字（数字+スーツ）を上下に配置
    let chars: Vec<char> = text.chars().collect();
    if chars.len() >= 2 {
        let num_str = chars[0].to_string();
        let suit_str = chars[1].to_string();
        draw_jp_text(font, &num_str, x + 14.0, y + 28.0, 24, color);
        draw_jp_text(font, &suit_str, x + 14.0, y + 52.0, 24, color);
    } else {
        draw_jp_text(font, &text, x + 8.0, y + 40.0, 24, color);
    }
}

/// アクションボタンを描画する
fn draw_action_buttons(state: &GameState, font: Option<&Font>) {
    // 鳴き選択肢がある場合はそちらを描画
    if !state.available_calls.is_empty() {
        draw_call_buttons(state, font);
        return;
    }

    if !state.is_my_turn {
        draw_jp_text(
            font,
            "他のプレイヤーの手番です...",
            480.0,
            640.0,
            FONT_SIZE,
            Color::new(0.8, 0.8, 0.8, 0.7),
        );
        return;
    }

    if state.riichi_selection_mode {
        draw_jp_text(
            font,
            "【リーチ】聴牌になる牌を選んで打牌",
            330.0,
            640.0,
            FONT_SIZE,
            Color::new(1.0, 0.9, 0.3, 1.0),
        );
    } else if state.is_riichi {
        draw_jp_text(
            font,
            "【リーチ中】自動ツモ切り",
            400.0,
            640.0,
            FONT_SIZE,
            Color::new(1.0, 0.3, 0.3, 1.0),
        );
    }

    if state.drawn.is_some() {
        // ツモ和了ボタン
        if state.can_tsumo {
            let tsumo_bg = Color::new(0.9, 0.1, 0.1, 1.0);
            draw_rectangle(900.0, 720.0, 80.0, 40.0, tsumo_bg);
            draw_rectangle_lines(900.0, 720.0, 80.0, 40.0, 2.0, WHITE);
            draw_jp_text(font, "ツモ", 916.0, 747.0, FONT_SIZE, WHITE);
        }

        // リーチボタン
        if state.can_riichi {
            let riichi_bg = Color::new(0.1, 0.6, 0.1, 1.0);
            draw_rectangle(1000.0, 720.0, 80.0, 40.0, riichi_bg);
            draw_rectangle_lines(1000.0, 720.0, 80.0, 40.0, 2.0, WHITE);
            draw_jp_text(font, "リーチ", 1008.0, 747.0, SMALL_FONT, WHITE);
        }

        for (idx, tile) in state.self_kan_options.iter().enumerate() {
            let x = 720.0 + idx as f32 * 110.0;
            let kan_bg = Color::new(0.1, 0.3, 0.8, 1.0);
            draw_rectangle(x, 670.0, 100.0, 40.0, kan_bg);
            draw_rectangle_lines(x, 670.0, 100.0, 40.0, 2.0, WHITE);
            draw_jp_text(
                font,
                &format!("{}カン", tile.to_string()),
                x + 10.0,
                697.0,
                SMALL_FONT,
                WHITE,
            );
        }

        // 操作説明
        if state.riichi_selection_mode {
            draw_jp_text(
                font,
                "黄色の牌だけがリーチ打牌できます。リーチボタンでも解除できます。",
                100.0,
                770.0,
                SMALL_FONT,
                Color::new(0.9, 0.9, 0.5, 0.8),
            );
        } else if !state.is_riichi {
            draw_jp_text(
                font,
                "牌をクリックで選択、もう一度クリックで打牌",
                100.0,
                770.0,
                SMALL_FONT,
                Color::new(0.8, 0.8, 0.8, 0.7),
            );
        }
    }
}

/// 鳴き選択ボタンを描画する
fn draw_call_buttons(state: &GameState, font: Option<&Font>) {
    // 鳴き対象の牌を表示
    if let Some(target) = &state.call_target_tile {
        let tile_str = crate::game::tile_to_string(*target);
        draw_jp_text(
            font,
            &format!("捨て牌: {}  鳴きますか？", tile_str),
            400.0,
            600.0,
            FONT_SIZE,
            Color::new(1.0, 0.9, 0.3, 1.0),
        );
    }

    let base_x = 400.0;
    let base_y = 620.0;
    let btn_w = 100.0;
    let btn_h = 40.0;
    let btn_spacing = 10.0;

    let call_btn_bg = Color::new(0.8, 0.2, 0.2, 1.0); // 赤系
    let ron_btn_bg = Color::new(0.9, 0.1, 0.1, 1.0); // 濃い赤
    let pass_btn_bg = Color::new(0.4, 0.4, 0.4, 1.0); // グレー

    let mut btn_idx = 0;

    for call in &state.available_calls {
        let x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
        match call {
            AvailableCall::Ron => {
                draw_rectangle(x, base_y, btn_w, btn_h, ron_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "ロン", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
            }
            AvailableCall::Pon => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "ポン", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
            }
            AvailableCall::Daiminkan => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "カン", x + 18.0, base_y + 27.0, SMALL_FONT, WHITE);
            }
            AvailableCall::Chi { .. } => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "チー", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
            }
        }
        btn_idx += 1;
    }

    // パスボタン（最後に配置）
    let pass_x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
    draw_rectangle(pass_x, base_y, btn_w, btn_h, pass_btn_bg);
    draw_rectangle_lines(pass_x, base_y, btn_w, btn_h, 2.0, WHITE);
    draw_jp_text(font, "パス", pass_x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
}

/// 局の結果を表示する
fn draw_result(state: &GameState, font: Option<&Font>) {
    // 半透明背景（大きめ）
    draw_rectangle(150.0, 150.0, 980.0, 420.0, Color::new(0.0, 0.0, 0.0, 0.85));

    if let Some(msg) = &state.result_message {
        let lines: Vec<&str> = msg.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            // 1行目は大きく、それ以降は少し小さく
            let (font_size, color) = if i == 0 {
                (28, WHITE)
            } else {
                (22, Color::new(0.9, 0.9, 0.7, 1.0))
            };
            draw_jp_text(font, line, 220.0, 240.0 + i as f32 * 35.0, font_size, color);
        }
    }

    draw_jp_text(
        font,
        "クリックで次の局へ",
        480.0,
        530.0,
        FONT_SIZE,
        Color::new(0.8, 0.8, 0.8, 0.7),
    );
}

/// ゲーム終了画面
fn draw_game_over(state: &GameState, font: Option<&Font>) {
    draw_rectangle(200.0, 150.0, 880.0, 500.0, Color::new(0.0, 0.0, 0.0, 0.9));

    draw_jp_text(font, "ゲーム終了", 520.0, 250.0, 36, WHITE);

    // 最終順位を表示
    let wind_names = ["プレイヤー", "CPU1", "CPU2", "CPU3"];
    let mut rankings: Vec<(usize, i32)> = state
        .scores
        .iter()
        .enumerate()
        .map(|(i, &s)| (i, s))
        .collect();
    rankings.sort_by(|a, b| b.1.cmp(&a.1));

    for (rank, (player_idx, score)) in rankings.iter().enumerate() {
        let color = if *player_idx == 0 {
            Color::new(1.0, 0.9, 0.3, 1.0) // 自分は黄色
        } else {
            WHITE
        };
        draw_jp_text(
            font,
            &format!("{}位: {} {}点", rank + 1, wind_names[*player_idx], score),
            440.0,
            330.0 + rank as f32 * 40.0,
            24,
            color,
        );
    }

    draw_jp_text(
        font,
        "クリックで新しいゲーム",
        480.0,
        530.0,
        FONT_SIZE,
        Color::new(0.8, 0.8, 0.8, 0.7),
    );
}

/// Wind を日本語文字列に変換
fn wind_to_str(wind: mahjong_core::tile::Wind) -> &'static str {
    match wind {
        mahjong_core::tile::Wind::East => "東",
        mahjong_core::tile::Wind::South => "南",
        mahjong_core::tile::Wind::West => "西",
        mahjong_core::tile::Wind::North => "北",
    }
}
