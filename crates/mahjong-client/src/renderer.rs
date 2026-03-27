//! 描画モジュール
//!
//! 埋め込みPNGを使って麻雀牌を描画する。

use macroquad::prelude::*;
use mahjong_core::tile::Tile;
use mahjong_server::cpu::client::CpuConfig;
use mahjong_server::protocol::AvailableCall;

use crate::game::{GamePhase, GameState, SetupState};

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

/// 盤面の中心点 — 捨て牌・他家手牌の回転の軸
const BOARD_CENTER_X: f32 = 500.0;
const BOARD_CENTER_Y: f32 = 380.0;

/// Camera2D の回転角度（度）— 自分(0°)、下家(90°)、対面(180°)、上家(-90°)
const PLAYER_ROTATIONS: [f32; 4] = [0.0, 90.0, 180.0, -90.0];

/// 盤面中心を軸に回転する Camera2D を生成する
fn make_board_camera(rotation_deg: f32) -> Camera2D {
    let sw = screen_width();
    let sh = screen_height();
    Camera2D {
        target: vec2(BOARD_CENTER_X, BOARD_CENTER_Y),
        rotation: rotation_deg,
        zoom: vec2(2.0 / sw, 2.0 / sh),
        offset: vec2(
            2.0 * BOARD_CENTER_X / sw - 1.0,
            1.0 - 2.0 * BOARD_CENTER_Y / sh,
        ),
        ..Default::default()
    }
}

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
    back: Texture2D,
    stick1000: Texture2D,
    stick100: Texture2D,
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
            stick1000: load_texture_from_png(include_bytes!(
                "../../../assets/others/stick1000.png"
            )),
            stick100: load_texture_from_png(include_bytes!(
                "../../../assets/others/stick100.png"
            )),
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
        GamePhase::Setup => {
            draw_setup(state, font);
        }
        GamePhase::WaitingForStart => {
            draw_jp_text(font, "ゲーム開始中...", 540.0, 400.0, 30, WHITE);
        }
        GamePhase::Playing => {
            draw_dora_indicators(state, font, tile_textures);
            draw_discards(state, tile_textures);
            draw_center_panel(state, font);
            draw_other_player_hands(state, tile_textures);
            draw_hand(state, font, tile_textures);
            draw_melds(state, tile_textures);
            draw_action_buttons(state, font);
        }
        GamePhase::RoundResult => {
            draw_dora_indicators(state, font, tile_textures);
            draw_discards(state, tile_textures);
            draw_center_panel(state, font);
            draw_other_player_hands(state, tile_textures);
            draw_hand(state, font, tile_textures);
            draw_melds(state, tile_textures);
            draw_result(state, font, tile_textures);
        }
        GamePhase::GameOver => {
            draw_game_over(state, font);
        }
    }
}

/// ドラ表示牌・供託棒・本場を画面左上に描画する
fn draw_dora_indicators(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) {
    let dora_w: f32 = 30.0;
    let dora_h: f32 = 42.0;
    let dora_step: f32 = dora_w; // 牌同士がくっつく（隙間なし）
    let padding: f32 = 6.0;
    let base_x: f32 = padding;
    let base_y: f32 = padding;
    let revealed_count = state.dora_indicators.len();

    // 供託棒・本場の表示パラメータ
    let stick_w: f32 = 50.0;
    let stick_h: f32 = 8.0;
    let stick_area_x = base_x + 5.0 * dora_step + 8.0; // ドラ牌の右側
    let stick_font: u16 = 14;

    // 背景の半透明黒四角
    let bg_w = stick_area_x + stick_w + 30.0 + padding;
    let bg_h = dora_h + padding * 2.0;
    draw_rectangle(0.0, 0.0, bg_w, bg_h, Color::new(0.0, 0.0, 0.0, 0.5));

    // ドラ表示牌（5枚: 表向き＋裏向き）
    for i in 0..5 {
        let x = base_x + i as f32 * dora_step;
        if i < revealed_count {
            draw_tile_sprite(
                tile_textures.for_tile(&state.dora_indicators[i]),
                x, base_y, dora_w, dora_h, WHITE,
            );
        } else {
            draw_tile_sprite(
                &tile_textures.back,
                x, base_y, dora_w, dora_h, WHITE,
            );
        }
    }

    // 千点棒 × 供託数（上段）
    let stick_y_top = base_y + 6.0;
    draw_tile_sprite(
        &tile_textures.stick1000,
        stick_area_x, stick_y_top, stick_w, stick_h, WHITE,
    );
    draw_jp_text(
        font,
        &format!("×{}", state.riichi_sticks),
        stick_area_x + stick_w + 2.0,
        stick_y_top + stick_h,
        stick_font,
        WHITE,
    );

    // 百点棒 × 本場数（下段）
    let stick_y_bottom = stick_y_top + stick_h + 10.0;
    draw_tile_sprite(
        &tile_textures.stick100,
        stick_area_x, stick_y_bottom, stick_w, stick_h, WHITE,
    );
    draw_jp_text(
        font,
        &format!("×{}", state.honba),
        stick_area_x + stick_w + 2.0,
        stick_y_bottom + stick_h,
        stick_font,
        WHITE,
    );
}

/// 盤面中央の情報パネル（半透明の黒い四角＋各家の風と得点＋局情報）を描画する
fn draw_center_panel(state: &GameState, font: Option<&Font>) {
    // 捨て牌の内側に収まるサイズの半透明黒四角
    let panel_size: f32 = 160.0;
    let half = panel_size / 2.0;
    draw_rectangle(
        BOARD_CENTER_X - half,
        BOARD_CENTER_Y - half,
        panel_size,
        panel_size,
        Color::new(0.0, 0.0, 0.0, 0.55),
    );

    // 各家の風と得点をそれぞれの向きで描画
    let my_wind_idx = state.seat_wind.map(|w| w.to_index()).unwrap_or(0);
    let label_dist: f32 = 64.0; // 中心からラベルまでの距離

    for player_idx in 0..4 {
        let display_wind = mahjong_core::tile::Wind::from_index(my_wind_idx + player_idx);
        let score = state.scores[player_idx];
        let label = format!("{} {}点", wind_to_str(display_wind), score);

        set_camera(&make_board_camera(PLAYER_ROTATIONS[player_idx]));

        let dims = measure_text(&label, font, SMALL_FONT, 1.0);
        draw_jp_text(
            font,
            &label,
            BOARD_CENTER_X - dims.width / 2.0,
            BOARD_CENTER_Y + label_dist,
            SMALL_FONT,
            Color::new(0.9, 0.9, 0.9, 1.0),
        );

        set_default_camera();
    }

    // 局情報（プレイヤー＝自分に読める方向で描画）
    let round_wind = match state.round_number / 4 {
        0 => "東",
        1 => "南",
        2 => "西",
        _ => "北",
    };
    let round_num = (state.round_number % 4) + 1;

    let round_text = format!("{}{}局", round_wind, round_num);
    let remaining_text = format!("残{}枚", state.remaining_tiles);

    let round_dims = measure_text(&round_text, font, FONT_SIZE, 1.0);
    let remain_dims = measure_text(&remaining_text, font, FONT_SIZE, 1.0);

    draw_jp_text(
        font,
        &round_text,
        BOARD_CENTER_X - round_dims.width / 2.0,
        BOARD_CENTER_Y - 6.0,
        FONT_SIZE,
        WHITE,
    );
    draw_jp_text(
        font,
        &remaining_text,
        BOARD_CENTER_X - remain_dims.width / 2.0,
        BOARD_CENTER_Y + round_dims.offset_y + 4.0,
        FONT_SIZE,
        WHITE,
    );
}

fn draw_discards(state: &GameState, tile_textures: &TileTextures) {
    let dtw: f32 = 32.0; // 牌の自然な幅
    let dth: f32 = 44.0; // 牌の自然な高さ
    let col_step: f32 = dtw; // 列方向（隙間なし）
    let row_step: f32 = dth; // 行方向（隙間なし）

    // 正規化された配置パラメータ（「自分」視点: 左→右、行は下方向）
    let half_width = 3.0 * col_step; // 6枚分の半幅 = 108px
    let stick_offset: f32 = 108.0; // 中心からリーチ棒までの距離
    let discard_offset: f32 = 130.0; // 中心から捨て牌開始までの距離（リーチ棒分のスペース確保）

    // リーチ棒の描画サイズ（元画像は約800×117px → 横向きで縮小）
    let stick_w: f32 = 100.0;
    let stick_h: f32 = 14.0;

    let start_x = BOARD_CENTER_X - half_width;
    let start_y = BOARD_CENTER_Y + discard_offset;

    for player_idx in 0..4 {
        let discards = &state.discards[player_idx];

        set_camera(&make_board_camera(PLAYER_ROTATIONS[player_idx]));

        // リーチ棒描画（リーチ宣言済みの場合のみ）
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

        // 捨て牌描画（正規化された配置: 左→右、行は下方向）
        // カメラ回転により各家の向きに自動変換される
        let mut col_offset: f32 = 0.0;
        let mut current_row: usize = 0;

        for (i, discard) in discards.iter().enumerate() {
            let row = i / 6;
            let tint = if discard.is_tsumogiri {
                Color::new(0.72, 0.72, 0.72, 1.0)
            } else {
                WHITE
            };

            if row != current_row {
                col_offset = 0.0;
                current_row = row;
            }

            if discard.is_riichi {
                // リーチ牌: 90°回転（横倒し）
                let x = start_x + col_offset;
                let y = start_y + row as f32 * row_step + (dth - dtw) / 2.0;
                draw_tile_sprite_rotated(
                    tile_textures.for_tile(&discard.tile),
                    x, y, dtw, dth, tint,
                    std::f32::consts::FRAC_PI_2,
                );
                col_offset += dth; // 横倒し牌の幅 = dth（隙間なし）
            } else {
                let x = start_x + col_offset;
                let y = start_y + row as f32 * row_step;
                draw_tile_sprite(
                    tile_textures.for_tile(&discard.tile),
                    x, y, dtw, dth, tint,
                );
                col_offset += col_step;
            }
        }

        set_default_camera();
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

/// 回転付きで牌スプライトを描画する
///
/// (vx, vy) は回転後の「見た目上の左上」座標。
/// テクスチャは常に自然なアスペクト比 (w, h) で描画し、
/// 回転による描画座標のずれを内部で補正する。
fn draw_tile_sprite_rotated(
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
fn draw_other_player_hands(state: &GameState, tile_textures: &TileTextures) {
    let tw: f32 = 28.0; // 牌の自然な幅
    let th: f32 = 40.0; // 牌の自然な高さ
    let meld_gap: f32 = 6.0;
    let tile_step: f32 = tw; // 牌同士がくっつく（隙間なし）
    let hand_distance: f32 = 290.0; // 中心から手牌までの距離

    let base_y = BOARD_CENTER_Y + hand_distance;

    for other_idx in 0..3 {
        let relative_idx = other_idx + 1; // 1=下家, 2=対面, 3=上家
        let other = &state.other_players[other_idx];

        // 手牌＋副露の合計幅を計算してセンタリング
        let hand_count = if other.revealed {
            other.hand.len()
        } else {
            other.concealed_count
        };
        let meld_tile_count: usize = other.melds.iter().map(|m| m.tiles.len()).sum();
        let meld_gaps = if other.melds.is_empty() {
            0.0
        } else {
            meld_gap + (other.melds.len() as f32 - 1.0) * meld_gap
        };
        let total_width =
            hand_count as f32 * tile_step + meld_tile_count as f32 * tile_step + meld_gaps;
        let start_x = BOARD_CENTER_X - total_width / 2.0;

        set_camera(&make_board_camera(PLAYER_ROTATIONS[relative_idx]));

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
        for (i, meld) in other.melds.iter().enumerate() {
            if i > 0 {
                x += meld_gap;
            }
            for tile in &meld.tiles {
                draw_tile_sprite(tile_textures.for_tile(tile), x, base_y, tw, th, WHITE);
                x += tile_step;
            }
        }

        set_default_camera();
    }
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

fn draw_result(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    draw_rectangle(150.0, 150.0, 980.0, 420.0, Color::new(0.0, 0.0, 0.0, 0.85));

    let tw: f32 = 24.0; // 小さめの牌サイズ
    let th: f32 = 34.0;
    let meld_gap: f32 = 4.0;
    let win_tile_gap: f32 = 6.0;

    // 手牌の合計幅を計算して開始位置を決定（ドラ表示もこの左端に揃える）
    let hand_tiles = state.win_hand.len() as f32;
    let win_tile_w = if state.win_tile.is_some() { tw + win_tile_gap } else { 0.0 };
    let meld_tiles: f32 = state.win_melds.iter().map(|m| m.len() as f32 * tw).sum();
    let meld_gaps = if state.win_melds.is_empty() {
        0.0
    } else {
        meld_gap + (state.win_melds.len() as f32 - 1.0) * meld_gap
    };
    let total_hand_w = hand_tiles * tw + win_tile_w + meld_tiles + meld_gaps;
    let dora_w = 5.0 * tw;
    let content_w = total_hand_w.max(dora_w);
    let start_x = (1120.0 - content_w).max(170.0);

    // ドラ表示牌（上段: 5枚）
    let dora_y: f32 = 170.0;
    let revealed_count = state.dora_indicators.len();
    for i in 0..5 {
        let x = start_x + i as f32 * tw;
        if i < revealed_count {
            draw_tile_sprite(
                tile_textures.for_tile(&state.dora_indicators[i]),
                x, dora_y, tw, th, WHITE,
            );
        } else {
            draw_tile_sprite(&tile_textures.back, x, dora_y, tw, th, WHITE);
        }
    }

    // 裏ドラ表示牌（リーチ和了時のみ表示）
    let mut next_y = dora_y + th;
    if !state.uradora_indicators.is_empty() {
        let ura_count = state.uradora_indicators.len();
        for i in 0..5 {
            let x = start_x + i as f32 * tw;
            if i < ura_count {
                draw_tile_sprite(
                    tile_textures.for_tile(&state.uradora_indicators[i]),
                    x, next_y, tw, th, WHITE,
                );
            } else {
                draw_tile_sprite(&tile_textures.back, x, next_y, tw, th, WHITE);
            }
        }
        next_y += th;
    }

    // 和了者の手牌＋副露＋和了牌を描画
    if !state.win_hand.is_empty() || !state.win_melds.is_empty() {
        next_y += 6.0;
        let mut x = start_x;

        // 手牌（閉じた部分）
        for tile in &state.win_hand {
            draw_tile_sprite(tile_textures.for_tile(tile), x, next_y, tw, th, WHITE);
            x += tw;
        }

        // 和了牌（少し離して描画）
        if let Some(win_tile) = &state.win_tile {
            x += win_tile_gap;
            let win_x = x;
            draw_tile_sprite(tile_textures.for_tile(win_tile), x, next_y, tw, th, WHITE);

            // 和了牌の下に「ツモ」or「ロン」
            let win_label = if state.win_is_tsumo { "ツモ" } else { "ロン" };
            let dims = measure_text(win_label, font, 12, 1.0);
            draw_jp_text(
                font,
                win_label,
                win_x + tw / 2.0 - dims.width / 2.0,
                next_y + th + 12.0,
                12,
                Color::new(1.0, 0.9, 0.3, 1.0),
            );
            x += tw;
        }

        // 副露
        if !state.win_melds.is_empty() {
            x += meld_gap;
            for (i, meld) in state.win_melds.iter().enumerate() {
                if i > 0 {
                    x += meld_gap;
                }
                for tile in meld {
                    draw_tile_sprite(tile_textures.for_tile(tile), x, next_y, tw, th, WHITE);
                    x += tw;
                }
            }
        }
    }

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

// ========== 設定画面 ==========

/// 設定画面のボタン領域
struct SetupButton {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl SetupButton {
    fn contains(&self, mx: f32, my: f32) -> bool {
        mx >= self.x && mx < self.x + self.w && my >= self.y && my < self.y + self.h
    }
}

/// 設定画面を描画する
fn draw_setup(state: &GameState, font: Option<&Font>) {
    let setup = &state.setup_state;

    // 背景パネル
    draw_rectangle(190.0, 80.0, 900.0, 640.0, Color::new(0.0, 0.0, 0.0, 0.85));
    draw_rectangle_lines(190.0, 80.0, 900.0, 640.0, 2.0, Color::new(0.5, 0.5, 0.5, 1.0));

    // タイトル
    draw_jp_text(font, "対局設定", 540.0, 130.0, 36, WHITE);

    let cpu_names = ["下家 (CPU1)", "対面 (CPU2)", "上家 (CPU3)"];
    let col_x = [250.0, 520.0, 790.0]; // 3列の左端X座標

    for (cpu_idx, &name) in cpu_names.iter().enumerate() {
        let cx = col_x[cpu_idx];
        let base_y = 180.0;

        // CPU名（ベースライン基準なので +24 で文字下端を揃える）
        draw_jp_text(font, name, cx + 30.0, base_y + 24.0, 24, Color::new(1.0, 0.9, 0.3, 1.0));

        // 強さ
        draw_jp_text(font, "強さ:", cx, base_y + 70.0, FONT_SIZE, Color::new(0.8, 0.8, 0.8, 1.0));
        for level_idx in 0..SetupState::level_count() {
            let btn_y = base_y + 80.0 + level_idx as f32 * 42.0;
            let selected = setup.cpu_levels[cpu_idx] == level_idx;
            let bg = if selected {
                Color::new(0.2, 0.5, 0.2, 1.0)
            } else {
                Color::new(0.25, 0.25, 0.25, 1.0)
            };
            draw_rectangle(cx, btn_y, 200.0, 34.0, bg);
            draw_rectangle_lines(cx, btn_y, 200.0, 34.0, 1.0, Color::new(0.5, 0.5, 0.5, 1.0));
            let label = SetupState::level_name(level_idx);
            // ボタン(34px)内でフォント(20px)を垂直中央: btn_y + (34+20)/2 = btn_y + 24
            draw_jp_text(font, label, cx + 10.0, btn_y + 24.0, FONT_SIZE, WHITE);
        }

        // 性格
        draw_jp_text(font, "性格:", cx, base_y + 230.0, FONT_SIZE, Color::new(0.8, 0.8, 0.8, 1.0));
        for pers_idx in 0..SetupState::personality_count() {
            let btn_y = base_y + 240.0 + pers_idx as f32 * 42.0;
            let selected = setup.cpu_personalities[cpu_idx] == pers_idx;
            let bg = if selected {
                Color::new(0.2, 0.3, 0.6, 1.0)
            } else {
                Color::new(0.25, 0.25, 0.25, 1.0)
            };
            draw_rectangle(cx, btn_y, 200.0, 34.0, bg);
            draw_rectangle_lines(cx, btn_y, 200.0, 34.0, 1.0, Color::new(0.5, 0.5, 0.5, 1.0));
            let label = SetupState::personality_name(pers_idx);
            draw_jp_text(font, label, cx + 10.0, btn_y + 24.0, FONT_SIZE, WHITE);
        }
    }

    // 対局開始ボタン
    let start_btn = SetupButton { x: 490.0, y: 630.0, w: 300.0, h: 56.0 };
    draw_rectangle(start_btn.x, start_btn.y, start_btn.w, start_btn.h, Color::new(0.6, 0.15, 0.15, 1.0));
    draw_rectangle_lines(start_btn.x, start_btn.y, start_btn.w, start_btn.h, 2.0, Color::new(0.9, 0.3, 0.3, 1.0));
    // ボタン(56px)内でフォント(28px)を垂直中央: btn_y + (56+28)/2 = btn_y + 38
    draw_jp_text(font, "対局開始", start_btn.x + 80.0, start_btn.y + 38.0, 28, WHITE);
}

/// 設定画面の入力を処理する。対局開始が押された場合 Some(configs) を返す。
pub fn handle_setup_input(state: &mut GameState, _font: Option<&Font>) -> Option<[CpuConfig; 3]> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }

    let (mx, my) = mouse_position();
    let setup = &mut state.setup_state;
    let col_x = [250.0, 520.0, 790.0];
    let base_y = 180.0;

    for cpu_idx in 0..3 {
        let cx = col_x[cpu_idx];

        // 強さボタン
        for level_idx in 0..SetupState::level_count() {
            let btn = SetupButton {
                x: cx, y: base_y + 80.0 + level_idx as f32 * 42.0,
                w: 200.0, h: 34.0,
            };
            if btn.contains(mx, my) {
                setup.cpu_levels[cpu_idx] = level_idx;
                return None;
            }
        }

        // 性格ボタン
        for pers_idx in 0..SetupState::personality_count() {
            let btn = SetupButton {
                x: cx, y: base_y + 240.0 + pers_idx as f32 * 42.0,
                w: 200.0, h: 34.0,
            };
            if btn.contains(mx, my) {
                setup.cpu_personalities[cpu_idx] = pers_idx;
                return None;
            }
        }
    }

    // 対局開始ボタン
    let start_btn = SetupButton { x: 490.0, y: 630.0, w: 300.0, h: 56.0 };
    if start_btn.contains(mx, my) {
        let configs = setup.build_configs();
        state.phase = GamePhase::WaitingForStart;
        return Some(configs);
    }

    None
}
