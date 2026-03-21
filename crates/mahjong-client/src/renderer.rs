//! 描画モジュール
//!
//! 埋め込みPNGを使って麻雀牌を描画する。

use macroquad::prelude::*;
use mahjong_core::tile::Tile;
use mahjong_server::protocol::AvailableCall;

use crate::game::{GamePhase, GameState};

/// 牌を描画する色
const TILE_BG: Color = Color::new(1.0, 1.0, 0.9, 1.0);
const TILE_BORDER: Color = Color::new(0.3, 0.3, 0.3, 1.0);
const SELECTED_BG: Color = Color::new(0.8, 1.0, 0.8, 1.0);
const RIICHI_SELECTABLE_BG: Color = Color::new(1.0, 0.96, 0.72, 1.0);
const RIICHI_DISABLED_BG: Color = Color::new(0.78, 0.78, 0.72, 1.0);
const RIICHI_DISABLED_TINT: Color = Color::new(0.45, 0.45, 0.42, 1.0);

const TILE_W: f32 = 48.0;
const TILE_H: f32 = 68.0;
const FONT_SIZE: u16 = 20;
const SMALL_FONT: u16 = 16;
const AGARI_FONT: u16 = 32;

/// 和了ボタンの定数（描画・入力の両方で使用）
pub const AGARI_BTN_X: f32 = 346.0;
pub const AGARI_BTN_Y: f32 = 600.0;
pub const AGARI_BTN_W: f32 = 200.0;
pub const AGARI_BTN_H: f32 = 60.0;

pub struct TileTextures {
    standard_tiles: Vec<Texture2D>,
    red_5m: Texture2D,
    red_5p: Texture2D,
    red_5s: Texture2D,
    #[allow(dead_code)]
    back: Texture2D,
}

impl TileTextures {
    pub fn load() -> Self {
        let standard_tiles = vec![
            load_texture_from_png(include_bytes!("../../../assets/tiles/1m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/2m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/3m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/4m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/5m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/6m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/7m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/8m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/9m.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/1p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/2p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/3p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/4p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/5p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/6p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/7p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/8p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/9p.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/1s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/2s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/3s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/4s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/5s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/6s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/7s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/8s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/9s.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/1z.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/2z.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/3z.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/4z.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/5z.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/6z.png")),
            load_texture_from_png(include_bytes!("../../../assets/tiles/7z.png")),
        ];

        Self {
            standard_tiles,
            red_5m: load_texture_from_png(include_bytes!("../../../assets/tiles/r5m.png")),
            red_5p: load_texture_from_png(include_bytes!("../../../assets/tiles/r5p.png")),
            red_5s: load_texture_from_png(include_bytes!("../../../assets/tiles/r5s.png")),
            back: load_texture_from_png(include_bytes!("../../../assets/tiles/back.png")),
        }
    }

    fn for_tile(&self, tile: &Tile) -> &Texture2D {
        if tile.is_red_dora() {
            match tile.get() {
                Tile::M5 => return &self.red_5m,
                Tile::P5 => return &self.red_5p,
                Tile::S5 => return &self.red_5s,
                _ => {}
            }
        }

        &self.standard_tiles[tile.get() as usize]
    }
}

fn load_texture_from_png(bytes: &[u8]) -> Texture2D {
    let texture = Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png));
    texture.set_filter(FilterMode::Linear);
    texture
}

fn draw_jp_text(font: Option<&Font>, text: &str, x: f32, y: f32, font_size: u16, color: Color) {
    let params = TextParams {
        font,
        font_size,
        color,
        ..Default::default()
    };
    draw_text_ex(text, x, y, params);
}

pub fn draw_game(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    match state.phase {
        GamePhase::WaitingForStart => {
            draw_jp_text(font, "ゲーム開始中...", 540.0, 400.0, 30, WHITE);
        }
        GamePhase::Playing => {
            draw_info_panel(state, font, tile_textures);
            draw_discards(state, font, tile_textures);
            draw_hand(state, font, tile_textures);
            draw_melds(state, tile_textures);
            draw_action_buttons(state, font);
        }
        GamePhase::RoundResult => {
            draw_info_panel(state, font, tile_textures);
            draw_discards(state, font, tile_textures);
            draw_hand(state, font, tile_textures);
            draw_melds(state, tile_textures);
            draw_result(state, font);
        }
        GamePhase::GameOver => {
            draw_game_over(state, font);
        }
    }
}

fn draw_info_panel(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    draw_rectangle(0.0, 0.0, 1280.0, 94.0, Color::new(0.0, 0.0, 0.0, 0.5));

    let seat = match state.seat_wind {
        Some(w) => wind_to_str(w),
        None => "?",
    };

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
    let riichi_sticks_text = if state.riichi_sticks > 0 {
        format!(" 供託:{}本", state.riichi_sticks)
    } else {
        String::new()
    };
    let riichi_marker = if state.is_riichi { " 【リーチ】" } else { "" };

    draw_jp_text(
        font,
        &format!(
            "{}{}局{}{}  自風: {}  残り: {}枚{}",
            round_wind,
            round_num,
            honba_text,
            riichi_sticks_text,
            seat,
            state.remaining_tiles,
            riichi_marker
        ),
        20.0,
        35.0,
        FONT_SIZE,
        WHITE,
    );

    let wind_names = ["東", "南", "西", "北"];
    let mut score_text = String::new();
    for i in 0..4 {
        if i > 0 {
            score_text.push_str("  ");
        }
        score_text.push_str(&format!("{}:{}", wind_names[i], state.scores[i]));
    }
    draw_jp_text(font, &score_text, 600.0, 35.0, SMALL_FONT, WHITE);

    if !state.dora_indicators.is_empty() {
        draw_jp_text(
            font,
            "ドラ表示:",
            20.0,
            70.0,
            SMALL_FONT,
            Color::new(1.0, 0.9, 0.3, 1.0),
        );

        for (i, tile) in state.dora_indicators.iter().enumerate() {
            draw_tile_sprite(
                tile_textures.for_tile(tile),
                112.0 + i as f32 * 34.0,
                42.0,
                30.0,
                42.0,
                WHITE,
            );
        }
    }
}

fn draw_discards(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let positions: [(f32, f32); 4] = [
        (400.0, 500.0),
        (900.0, 300.0),
        (400.0, 100.0),
        (100.0, 300.0),
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
            let tint = if discard.is_tsumogiri {
                Color::new(0.72, 0.72, 0.72, 1.0)
            } else {
                WHITE
            };
            draw_tile_sprite(tile_textures.for_tile(&discard.tile), x, y, 32.0, 44.0, tint);
        }
    }
}

fn draw_hand(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let hand_start_x = 100.0;
    let hand_y = 680.0;

    // フリテン状態の表示
    if state.is_furiten {
        draw_jp_text(
            font,
            "！！振聴です！！",
            hand_start_x,
            hand_y - 20.0,
            FONT_SIZE,
            Color::new(1.0, 0.2, 0.2, 1.0),
        );
    }

    // 選択中の牌を捨てるとフリテンになる場合の警告
    if state.selected_would_cause_furiten
        && (state.selected_tile.is_some() || state.selected_drawn)
    {
        draw_jp_text(
            font,
            "振聴になります！",
            hand_start_x + 200.0,
            hand_y - 20.0,
            FONT_SIZE,
            Color::new(1.0, 0.6, 0.1, 1.0),
        );
    }

    for (i, tile) in state.hand.iter().enumerate() {
        let x = hand_start_x + i as f32 * TILE_W;
        let selected = state.selected_tile == Some(i);
        let riichi_selectable = state.riichi_selection_mode && state.riichi_selectable_tiles.contains(&i);
        let y_offset = if selected { -10.0 } else { 0.0 };
        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;
        draw_tile(
            x,
            hand_y + y_offset,
            tile,
            selected,
            riichi_selectable,
            riichi_disabled,
            tile_textures,
        );
    }

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
            tile_textures,
        );

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

fn draw_melds(state: &GameState, tile_textures: &TileTextures) {
    if state.melds.is_empty() {
        return;
    }

    let meld_tile_w: f32 = 40.0;
    let meld_tile_h: f32 = 56.0;
    let meld_y: f32 = 692.0;
    let meld_gap: f32 = 12.0;
    let mut x = 1220.0;

    for meld in state.melds.iter().rev() {
        let tile_count = meld.tiles.len();
        let meld_width = tile_count as f32 * meld_tile_w;
        x -= meld_width;

        for (i, tile) in meld.tiles.iter().enumerate() {
            let tx = x + i as f32 * meld_tile_w;
            draw_meld_tile(tx, meld_y, tile, meld_tile_w, meld_tile_h, tile_textures);
        }

        x -= meld_gap;
    }
}

fn draw_meld_tile(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    w: f32,
    h: f32,
    tile_textures: &TileTextures,
) {
    let bg = Color::new(0.9, 0.95, 1.0, 1.0);
    draw_rectangle(x, y, w - 2.0, h - 2.0, bg);
    draw_rectangle_lines(x, y, w - 2.0, h - 2.0, 2.0, TILE_BORDER);
    draw_tile_sprite(tile_textures.for_tile(tile), x + 2.0, y + 1.0, w - 6.0, h - 6.0, WHITE);
}

fn draw_tile(
    x: f32,
    y: f32,
    tile: &mahjong_core::tile::Tile,
    selected: bool,
    riichi_selectable: bool,
    riichi_disabled: bool,
    tile_textures: &TileTextures,
) {
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

    let tint = if riichi_disabled {
        RIICHI_DISABLED_TINT
    } else {
        WHITE
    };
    draw_tile_sprite(
        tile_textures.for_tile(tile),
        x + 2.0,
        y + 1.0,
        TILE_W - 6.0,
        TILE_H - 6.0,
        tint,
    );
}

fn draw_tile_sprite(texture: &Texture2D, x: f32, y: f32, w: f32, h: f32, tint: Color) {
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

/// 和了ボタンを描画する（ロン・ツモ共通）
fn draw_agari_button(font: Option<&Font>) {
    let bg = Color::new(0.9, 0.05, 0.05, 1.0);
    let border = Color::new(1.0, 0.85, 0.0, 1.0);

    // 背景（角丸風に外枠を太くして目立たせる）
    draw_rectangle(AGARI_BTN_X, AGARI_BTN_Y, AGARI_BTN_W, AGARI_BTN_H, bg);
    draw_rectangle_lines(AGARI_BTN_X, AGARI_BTN_Y, AGARI_BTN_W, AGARI_BTN_H, 4.0, border);

    // テキスト「和 了」を大きく中央に表示
    draw_jp_text(
        font,
        "和　了",
        AGARI_BTN_X + 50.0,
        AGARI_BTN_Y + 42.0,
        AGARI_FONT,
        WHITE,
    );
}

fn draw_action_buttons(state: &GameState, font: Option<&Font>) {
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
        // 和了ボタン（ツモ）を目立つ位置に表示
        if state.can_tsumo {
            draw_agari_button(font);
        }

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

fn draw_call_buttons(state: &GameState, font: Option<&Font>) {
    let has_ron = state
        .available_calls
        .iter()
        .any(|c| matches!(c, AvailableCall::Ron));
    let has_non_ron = state
        .available_calls
        .iter()
        .any(|c| !matches!(c, AvailableCall::Ron));

    // ロンがある場合は和了ボタンを大きく表示
    if has_ron {
        draw_agari_button(font);
    }

    // ロン以外の鳴きがある場合のみ「鳴きますか？」を表示
    if has_non_ron {
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
    }

    // ロンがある場合は和了ボタンの右側に配置、なければ従来位置
    let base_x = if has_ron {
        AGARI_BTN_X + AGARI_BTN_W + 20.0
    } else {
        400.0
    };
    let base_y = if has_ron { AGARI_BTN_Y + 10.0 } else { 620.0 };
    let btn_w = 100.0;
    let btn_h = 40.0;
    let btn_spacing = 10.0;

    let call_btn_bg = Color::new(0.8, 0.2, 0.2, 1.0);
    let pass_btn_bg = Color::new(0.4, 0.4, 0.4, 1.0);

    let mut btn_idx = 0;

    for call in &state.available_calls {
        // ロンは和了ボタンで表示済みなのでスキップ
        if matches!(call, AvailableCall::Ron) {
            continue;
        }
        let x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
        match call {
            AvailableCall::Ron => unreachable!(),
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

    let pass_x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
    draw_rectangle(pass_x, base_y, btn_w, btn_h, pass_btn_bg);
    draw_rectangle_lines(pass_x, base_y, btn_w, btn_h, 2.0, WHITE);
    draw_jp_text(font, "パス", pass_x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
}

fn draw_result(state: &GameState, font: Option<&Font>) {
    draw_rectangle(150.0, 150.0, 980.0, 420.0, Color::new(0.0, 0.0, 0.0, 0.85));

    if let Some(msg) = &state.result_message {
        let lines: Vec<&str> = msg.lines().collect();
        for (i, line) in lines.iter().enumerate() {
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

fn draw_game_over(state: &GameState, font: Option<&Font>) {
    draw_rectangle(200.0, 150.0, 880.0, 500.0, Color::new(0.0, 0.0, 0.0, 0.9));

    draw_jp_text(font, "ゲーム終了", 520.0, 250.0, 36, WHITE);

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
            Color::new(1.0, 0.9, 0.3, 1.0)
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

fn wind_to_str(wind: mahjong_core::tile::Wind) -> &'static str {
    match wind {
        mahjong_core::tile::Wind::East => "東",
        mahjong_core::tile::Wind::South => "南",
        mahjong_core::tile::Wind::West => "西",
        mahjong_core::tile::Wind::North => "北",
    }
}
