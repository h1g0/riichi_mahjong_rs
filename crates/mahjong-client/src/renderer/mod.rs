//! 描画モジュール
//!
//! 埋め込みPNGを使って麻雀牌を描画する。

mod online;
mod overlay;
mod theme;
pub use online::{
    OnlineLobbyAction, OnlineMenuAction, handle_online_lobby_input, handle_online_menu_input,
};
pub use overlay::OverlayClick;

use macroquad::prelude::*;
use mahjong_core::tile::Tile;
use mahjong_server::cpu::client::CpuConfig;

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

use crate::game::{GamePhase, GameState, SetupState};

const RIICHI_DISABLED_TINT: Color = Color::new(0.45, 0.45, 0.42, 1.0);

const TILE_W: f32 = 48.0;
const TILE_H: f32 = 68.0;
const FONT_SIZE: u16 = 20;
const SMALL_FONT: u16 = 16;
const AGARI_FONT: u16 = 32;

/// 設計上の基準解像度。すべての UI 座標はこの仮想キャンバス上で定義され、
/// 実際のウィンドウ／キャンバスサイズに合わせて一様に拡大・縮小される。
/// （HTML 側でキャンバスのアスペクト比を DESIGN_W:DESIGN_H に固定しているため歪まない）
pub const DESIGN_W: f32 = 1280.0;
pub const DESIGN_H: f32 = 800.0;

/// 盤面の中心点 — 捨て牌・他家手牌の回転の軸（画面横中央に合わせる）
const BOARD_CENTER_X: f32 = DESIGN_W / 2.0;
const BOARD_CENTER_Y: f32 = 380.0;

/// 自分の手牌の Y 座標（上端）
pub const HAND_Y: f32 = 680.0;
/// ツモ牌を手牌の右に離して置くときの間隔
pub const DRAWN_GAP: f32 = 20.0;

/// 自分の手牌（伏せ牌 `hand_len` 枚）を画面中央に揃えるための左端 X を返す。
///
/// ツモ牌は中央寄せの基準には含めず、手牌の右側に張り出す（一般的な配置）。
/// 描画（[`draw_hand`]）とクリック判定（`GameState::handle_input`）で共有する。
pub fn player_hand_start_x(hand_len: usize) -> f32 {
    let hand_w = hand_len as f32 * TILE_W;
    (DESIGN_W - hand_w) / 2.0
}

/// Camera2D の回転角度（度）— 自分(0°)、下家(-90°)、対面(180°)、上家(90°)
const PLAYER_ROTATIONS: [f32; 4] = [0.0, -90.0, 180.0, 90.0];

/// 自分から見た相対位置(0=自分,1=下家,2=対面,3=上家)を固定の座席インデックスへ変換する。
/// `scores` や `player_labels` は座席インデックス順に並ぶため、画面の各向きへ描画する際は
/// この変換を通す（オンライン非ホストで自分の座席が0以外でも正しい席に表示される）。
fn seat_at_relative_position(my_seat: usize, relative_idx: usize) -> usize {
    (my_seat + relative_idx) % 4
}

/// 設計座標 (0,0)-(DESIGN_W,DESIGN_H) をキャンバス全体に写すカメラ。
/// 実バッファ解像度に依存しないため、ウィンドウサイズが変わっても
/// レイアウトはそのまま拡大・縮小される。
///
/// 画面へ直接描画する場合、`Camera2D::from_display_rect`（zoom.y が負）だと
/// 上下反転してしまうため、盤面カメラと同じく zoom.y を正にして上向きに合わせる。
fn design_camera() -> Camera2D {
    Camera2D {
        target: vec2(DESIGN_W / 2.0, DESIGN_H / 2.0),
        zoom: vec2(2.0 / DESIGN_W, 2.0 / DESIGN_H),
        ..Default::default()
    }
}

/// フレーム冒頭やオーバーレイ描画で使う、設計座標系のデフォルトカメラを適用する。
pub fn set_design_camera() {
    set_camera(&design_camera());
}

/// 設計座標 → 実バッファ座標の拡大率。キャンバスはアスペクト比固定なので
/// 横・縦どちらで割っても同じ値になる（横を採用）。
fn design_scale() -> f32 {
    screen_width() / DESIGN_W
}

/// マウス座標を実バッファ座標から設計座標へ変換して返す。
/// クリック判定はすべて設計座標で行うため、入力側もここで合わせる。
pub fn mouse_position_design() -> (f32, f32) {
    let (mx, my) = mouse_position();
    let scale = design_scale();
    (mx / scale, my / scale)
}

/// 盤面中心を軸に回転する Camera2D を生成する
fn make_board_camera(rotation_deg: f32) -> Camera2D {
    Camera2D {
        target: vec2(BOARD_CENTER_X, BOARD_CENTER_Y),
        rotation: rotation_deg,
        zoom: vec2(2.0 / DESIGN_W, 2.0 / DESIGN_H),
        offset: vec2(
            2.0 * BOARD_CENTER_X / DESIGN_W - 1.0,
            1.0 - 2.0 * BOARD_CENTER_Y / DESIGN_H,
        ),
        ..Default::default()
    }
}

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
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/1m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/2m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/3m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/4m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/5m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/6m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/7m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/8m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/9m.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/1p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/2p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/3p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/4p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/5p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/6p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/7p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/8p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/9p.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/1s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/2s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/3s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/4s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/5s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/6s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/7s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/8s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/9s.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/1z.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/2z.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/3z.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/4z.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/5z.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/6z.png")),
            load_texture_from_png(include_bytes!("../../../../assets/images/tiles/7z.png")),
        ];

        Self {
            standard_tiles,
            red_5m: load_texture_from_png(include_bytes!(
                "../../../../assets/images/tiles/r5m.png"
            )),
            red_5p: load_texture_from_png(include_bytes!(
                "../../../../assets/images/tiles/r5p.png"
            )),
            red_5s: load_texture_from_png(include_bytes!(
                "../../../../assets/images/tiles/r5s.png"
            )),
            back: load_texture_from_png(include_bytes!("../../../../assets/images/tiles/back.png")),
            stick1000: load_texture_from_png(include_bytes!(
                "../../../../assets/images/sticks/stick1000.png"
            )),
            stick100: load_texture_from_png(include_bytes!(
                "../../../../assets/images/sticks/stick100.png"
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
    theme::draw_text(font, text, x, y, font_size, color);
}

pub fn draw_game(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) -> Option<OverlayClick> {
    match state.phase {
        GamePhase::Setup => {
            draw_setup(state, font);
            None
        }
        GamePhase::OnlineMenu => {
            online::draw_online_menu(state, font);
            None
        }
        GamePhase::OnlineLobby => {
            online::draw_online_lobby(state, font);
            None
        }
        GamePhase::WaitingForStart => {
            draw_setup_background();
            theme::draw_text_centered(
                font,
                "ゲーム開始中...",
                DESIGN_W / 2.0,
                400.0,
                28,
                theme::TEXT_BR,
            );
            None
        }
        GamePhase::Playing => {
            draw_felt_background();
            draw_discards(state, tile_textures);
            draw_center_panel(state, font);
            draw_other_player_hands(state, tile_textures);
            draw_melds(state, tile_textures);
            draw_hand(state, font, tile_textures);
            draw_top_bar(state, font, tile_textures);
            let click = overlay::draw_action_buttons(state, font, tile_textures);
            online::draw_connection_banner(state, font);
            online::draw_turn_timer(state, font);
            click
        }
        GamePhase::RoundResult => {
            draw_felt_background();
            draw_discards(state, tile_textures);
            draw_center_panel(state, font);
            draw_other_player_hands(state, tile_textures);
            draw_melds(state, tile_textures);
            draw_hand(state, font, tile_textures);
            draw_top_bar(state, font, tile_textures);
            draw_result(state, font, tile_textures);
            online::draw_connection_banner(state, font);
            None
        }
        GamePhase::GameOver => {
            draw_game_over(state, font);
            None
        }
    }
}

/// 対局画面の背景（中央が明るい放射状のフェルト）。
fn draw_felt_background() {
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

/// 設定・終了画面の背景（やや明るい緑から暗緑へ）。
fn draw_setup_background() {
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

/// 画面上部のバー（ドラ表示・局/残り枚数・各家の得点チップ）を描画する。
fn draw_top_bar(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    const BAR_H: f32 = 50.0;
    // バー背景＋下境界線
    draw_rectangle(0.0, 0.0, DESIGN_W, BAR_H, Color::new(0.0, 0.0, 0.0, 0.48));
    draw_rectangle(0.0, BAR_H - 1.0, DESIGN_W, 1.0, theme::BORDER);

    draw_dora_panel(state, font, tile_textures);
    draw_round_center(state, font, BAR_H);
    draw_score_chips(state, font, BAR_H);
}

/// 上部バー左側：ドラ表示牌＋供託リーチ棒＋本場。
fn draw_dora_panel(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
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

    // 「ドラ」ラベル
    draw_jp_text(
        font,
        "ドラ",
        panel_x + 10.0,
        panel_y + 21.0,
        11,
        theme::TEXT_DIM,
    );

    let revealed = state.dora_indicators.len();
    for i in 0..5 {
        let x = tiles_x + i as f32 * (dora_w + 1.0);
        if i < revealed {
            draw_tile_sprite(
                tile_textures.for_tile(&state.dora_indicators[i]),
                x,
                tiles_y,
                dora_w,
                dora_h,
                WHITE,
            );
        } else {
            draw_tile_sprite(&tile_textures.back, x, tiles_y, dora_w, dora_h, WHITE);
        }
    }

    // 供託リーチ棒（上段）／本場（下段）
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

/// 上部バー中央：局表示と残り枚数。
fn draw_round_center(state: &GameState, font: Option<&Font>, bar_h: f32) {
    let round_wind = match state.round_number / 4 {
        0 => "東",
        1 => "南",
        2 => "西",
        _ => "北",
    };
    let round_num = (state.round_number % 4) + 1;
    let round_text = format!("{}{}局", round_wind, round_num);
    let remain_text = format!("{}枚", state.remaining_tiles);

    let baseline = bar_h / 2.0 + 6.0;
    let rdims = theme::measure_scaled(font, &round_text, 16);
    let gap = 12.0;
    let rmdims = theme::measure_scaled(font, &remain_text, 14);
    let total_w = rdims.width + gap + rmdims.width;
    let start_x = DESIGN_W / 2.0 - total_w / 2.0;

    draw_jp_text(font, &round_text, start_x, baseline, 16, theme::GOLD_LT);
    draw_jp_text(
        font,
        &remain_text,
        start_x + rdims.width + gap,
        baseline,
        14,
        theme::TEXT_DIM,
    );
}

/// 上部バー右側：各家の得点チップ（自分を強調）。
fn draw_score_chips(state: &GameState, font: Option<&Font>, bar_h: f32) {
    const CHIP_W: f32 = 70.0;
    const CHIP_H: f32 = 38.0;
    const GAP: f32 = 7.0;
    let count = 4;
    let total = count as f32 * CHIP_W + (count as f32 - 1.0) * GAP;
    let start_x = DESIGN_W - 14.0 - total;
    let chip_y = (bar_h - CHIP_H) / 2.0;

    for rel in 0..4 {
        let seat = seat_at_relative_position(state.my_seat, rel);
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

        let name = short_player_name(&state.player_labels[seat], rel);
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

/// 得点チップ用の短いプレイヤー名。
fn short_player_name(label: &crate::game::PlayerLabel, rel: usize) -> String {
    use crate::game::PlayerLabel;
    match label {
        PlayerLabel::Me => "あなた".to_string(),
        PlayerLabel::Human(name) => {
            let mut s: String = name.chars().take(5).collect();
            if name.chars().count() > 5 {
                s.push('…');
            }
            s
        }
        PlayerLabel::Cpu { .. } => format!("CPU{}", rel),
    }
}

/// 桁区切り付きの得点表記。
fn format_score(score: i32) -> String {
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
    // 先頭の余分なゼロを除去
    let mut joined = parts.join(",");
    joined = joined.trim_start_matches('0').to_string();
    if joined.starts_with(',') {
        joined = joined.trim_start_matches(',').to_string();
    }
    if neg { format!("-{}", joined) } else { joined }
}

/// 盤面中央の情報パネル（半透明の黒い四角＋各家の風と得点＋局情報）を描画する
fn draw_center_panel(state: &GameState, font: Option<&Font>) {
    // 捨て牌の内側に収まる角丸のゴールド枠パネル
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

    // 各家の風と得点をそれぞれの向きで描画
    let my_wind_idx = state.seat_wind.map(|w| w.to_index()).unwrap_or(0);
    let label_dist: f32 = 64.0; // 中心からラベルまでの距離

    for (player_idx, &rotation) in PLAYER_ROTATIONS.iter().enumerate() {
        // player_idx は自分から見た相対位置(0=自分,1=下家,2=対面,3=上家)。
        // scores / player_labels は固定の座席インデックス順なので、相対位置を
        // 絶対座席へ変換してから引く(オンライン非ホストで自分の座席が0以外でもずれない)。
        let seat = seat_at_relative_position(state.my_seat, player_idx);
        let display_wind = mahjong_core::tile::Wind::from_index(my_wind_idx + player_idx);
        let score = state.scores[seat];

        set_camera(&make_board_camera(rotation));

        // 風（ゴールド）＋得点（千点単位）を中心の各方向に描画
        theme::draw_text_centered(
            font,
            wind_to_str(display_wind),
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

        // CPU の強さ・性格（または相手の名前）を風・得点の下に表示する。
        // player_idx は自分からの相対位置で、得点チップの CPU 番号と一致する。
        if let Some(detail) = state.player_labels[seat].detail(player_idx) {
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

    // 局表示は小さく、残数表示を大きく強調する
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

    for (player_idx, &rotation) in PLAYER_ROTATIONS.iter().enumerate() {
        let discards = &state.discards[player_idx];

        set_camera(&make_board_camera(rotation));

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
            let mut tint = if discard.is_tsumogiri {
                Color::new(0.72, 0.72, 0.72, 1.0)
            } else {
                WHITE
            };
            // 鳴かれた牌はごく薄い半透明で描く
            if discard.is_called {
                tint.a = 0.28;
            }

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
                    x,
                    y,
                    dtw,
                    dth,
                    tint,
                    -std::f32::consts::FRAC_PI_2,
                );
                col_offset += dth; // 横倒し牌の幅 = dth（隙間なし）
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

/// 手牌の上に表示する状態バッジ（ピル）を描画し、次のバッジの x を返す。
fn draw_badge(
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

fn draw_hand(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let hand_start_x = player_hand_start_x(state.hand.len());
    let hand_y = HAND_Y;

    // 状態バッジ（フリテン・リーチ中・リーチ打牌選択中）
    let badge_y = hand_y - 26.0;
    let mut bx = hand_start_x;
    if state.is_furiten {
        bx = draw_badge(
            font,
            bx,
            badge_y,
            "振聴",
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
            "リーチ中",
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
            "打牌を選択",
            theme::rgba(0xc8a227, 0.12),
            theme::rgba(0xc8a227, 0.35),
            theme::GOLD_LT,
        );
    }
    if state.selected_would_cause_furiten && (state.selected_tile.is_some() || state.selected_drawn)
    {
        draw_badge(
            font,
            bx,
            badge_y,
            "振聴になります！",
            theme::rgba(0xcc6411, 0.18),
            theme::rgba(0xe88a1a, 0.6),
            Color::new(1.0, 0.7, 0.3, 1.0),
        );
    }

    for (i, tile) in state.hand.iter().enumerate() {
        let x = hand_start_x + i as f32 * TILE_W;
        let selected = state.selected_tile == Some(i);
        let riichi_selectable =
            state.riichi_selection_mode && state.riichi_selectable_tiles.contains(&i);
        let y_offset = if selected { -14.0 } else { 0.0 };
        let riichi_disabled = state.riichi_selection_mode && !riichi_selectable;
        if selected {
            draw_tile_highlight(x, hand_y + y_offset);
        }
        draw_tile(x, hand_y + y_offset, tile, riichi_disabled, tile_textures);
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
            "ツモ",
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
}

/// 選択中の牌の周囲にゴールドの縁取りを描く。
fn draw_tile_highlight(x: f32, y: f32) {
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

fn draw_melds(state: &GameState, tile_textures: &TileTextures) {
    if state.melds.is_empty() {
        return;
    }

    let tw: f32 = 40.0;
    let th: f32 = 56.0;
    let meld_y: f32 = 692.0;
    let meld_gap: f32 = 12.0;
    let mut x = 1220.0;

    for meld in state.melds.iter().rev() {
        let meld_width = calc_meld_width(meld, tw, th);
        x -= meld_width;
        draw_meld_group(meld, x, meld_y, tw, th, tile_textures);
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
    draw_tile_sprite(tile_textures.for_tile(tile), x, y, w - 2.0, h - 2.0, WHITE);
}

/// 横向きの副露牌を描画する（90°回転）
fn draw_meld_tile_sideways(
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
fn draw_meld_tile_back(x: f32, y: f32, w: f32, h: f32, tile_textures: &TileTextures) {
    draw_tile_sprite(&tile_textures.back, x, y, w - 2.0, h - 2.0, WHITE);
}

/// 鳴き元に応じて横向き牌の位置インデックスを返す
fn sideways_index(from: MeldFrom, tile_count: usize) -> usize {
    match from {
        MeldFrom::Previous => 0,               // 上家: 左端
        MeldFrom::Opposite => 1,               // 対面: 左から2番目
        MeldFrom::Following => tile_count - 1, // 下家: 右端
        _ => 0,                                // Unknown/Myself: フォールバック
    }
}

/// 副露グループの描画幅を計算する
fn calc_meld_width(meld: &Meld, tw: f32, th: f32) -> f32 {
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
fn draw_meld_group(
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

fn draw_tile(
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
        let meld_widths: f32 = other.melds.iter().map(|m| calc_meld_width(m, tw, th)).sum();
        let meld_gaps = if other.melds.is_empty() {
            0.0
        } else {
            meld_gap + (other.melds.len() as f32 - 1.0) * meld_gap
        };
        let total_width = hand_count as f32 * tile_step + meld_widths + meld_gaps;
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
            draw_meld_group(meld, x, base_y, tw, th, tile_textures);
            x += calc_meld_width(meld, tw, th);
        }

        set_design_camera();
    }
}

/// 局結果オーバーレイ。和了は構造化パネル、流局はメッセージパネルを描画する。
fn draw_result(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
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
fn draw_overlay_panel(panel_w: f32, panel_h: f32) -> (f32, f32, f32) {
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
fn draw_result_next_button(state: &GameState, font: Option<&Font>, cx: f32, y: f32, w: f32) {
    let h = 40.0;
    let x = cx - w / 2.0;
    theme::draw_rounded_rect(x, y, w, h, 6.0, theme::rgba(0xc8a227, 0.10));
    theme::draw_rounded_rect_lines(x, y, w, h, 6.0, 1.0, theme::GOLD_DK);
    let label = if state.win_result_index + 1 < state.win_results.len() {
        "次の和了へ →"
    } else {
        "次へ →"
    };
    theme::draw_text_centered(font, label, cx, y + 25.0, 14, theme::GOLD_LT);
}

/// 横一列の表示牌を描く（ドラ／裏ドラ用）。
fn draw_indicator_row(
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
fn draw_win_panel(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let wr = match state.current_win_result() {
        Some(w) => w,
        None => return,
    };
    let yaku_count = wr.yaku.len().max(1);
    let panel_w = 700.0;
    let panel_h = 326.0 + yaku_count as f32 * 22.0;
    let (panel_x, panel_y, panel_right) = draw_overlay_panel(panel_w, panel_h);
    let cx = panel_x + panel_w / 2.0;
    let content_l = panel_x + 40.0;
    let content_r = panel_right - 40.0;
    let mut y = panel_y + 28.0;

    // 種別（ツモ / ロン）
    let type_label = if wr.win_is_tsumo { "ツモ" } else { "ロン" };
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
    let dora_label_w = theme::measure_scaled(font, "ドラ", 11).width;
    let dora_tiles_w = state.dora_indicators.len() as f32 * (dw + 2.0);
    let ura_block_w = if state.uradora_indicators.is_empty() {
        0.0
    } else {
        24.0 + theme::measure_scaled(font, "裏ドラ", 11).width
            + 6.0
            + state.uradora_indicators.len() as f32 * (dw + 2.0)
    };
    let total_w = dora_label_w + 6.0 + dora_tiles_w + ura_block_w;
    let mut dx = cx - total_w / 2.0;
    draw_jp_text(font, "ドラ", dx, y + dh / 2.0 + 4.0, 11, theme::TEXT_DIM);
    dx += dora_label_w + 6.0;
    dx = draw_indicator_row(&state.dora_indicators, dx, y, dw, dh, tile_textures);
    if !state.uradora_indicators.is_empty() {
        dx += 18.0;
        draw_jp_text(font, "裏ドラ", dx, y + dh / 2.0 + 4.0, 11, theme::TEXT_DIM);
        dx += theme::measure_scaled(font, "裏ドラ", 11).width + 6.0;
        draw_indicator_row(&state.uradora_indicators, dx, y, dw, dh, tile_textures);
    }
    y += dh + 16.0;

    // 役一覧（上に区切り線）
    draw_rectangle(content_l, y, content_r - content_l, 1.0, theme::BORDER);
    y += 8.0;
    for (name, han) in &wr.yaku {
        draw_jp_text(font, name, content_l, y + 14.0, 14, theme::TEXT);
        let han_text = format!("{}飜", han);
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

    // 合計（飜符 ＋ 大きな点数）
    let mut hanfu = if wr.rank_name.is_empty() {
        format!("{}飜 {}符", wr.han, wr.fu)
    } else {
        format!("{}飜 {}符 · {}", wr.han, wr.fu, wr.rank_name)
    };
    if wr.riichi_sticks > 0 {
        hanfu.push_str(&format!("  供託 {}本", wr.riichi_sticks));
    }
    draw_jp_text(font, &hanfu, content_l, y + 24.0, 13, theme::TEXT_DIM);
    let pts = format!("{}点", format_score(wr.score_points));
    let pw = theme::measure_scaled(font, &pts, 28).width;
    draw_jp_text(font, &pts, content_r - pw, y + 28.0, 28, theme::GOLD_LT);
    y += 44.0;

    draw_result_next_button(state, font, cx, y, panel_w - 80.0);
}

/// 流局パネル。
fn draw_draw_panel(state: &GameState, font: Option<&Font>) {
    let lines: Vec<&str> = state
        .result_message
        .as_deref()
        .unwrap_or("流局")
        .lines()
        .collect();
    let panel_w = 560.0;
    let panel_h = 140.0 + lines.len() as f32 * 30.0;
    let (panel_x, panel_y, _) = draw_overlay_panel(panel_w, panel_h);
    let cx = panel_x + panel_w / 2.0;
    let mut y = panel_y + 40.0;

    theme::draw_text_centered(font, "流局", cx, y, 12, theme::GOLD);
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

fn draw_game_over(state: &GameState, font: Option<&Font>) {
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

    theme::draw_text_centered(font, "ゲーム終了", cx, panel_y + 52.0, 26, theme::TEXT_BR);

    let mut rankings: Vec<(usize, i32)> = state
        .scores
        .iter()
        .enumerate()
        .map(|(i, &s)| (i, s))
        .collect();
    rankings.sort_by_key(|r| std::cmp::Reverse(r.1));

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
        draw_jp_text(font, "位", row_x + 34.0, ry + 32.0, 11, rc);

        // 名前（CPU 番号は得点チップと同じ自分からの相対位置）
        let cpu_number = (*player_idx + 4 - state.my_seat) % 4;
        let name = state.player_labels[*player_idx].name(cpu_number);
        draw_jp_text(font, &name, row_x + 64.0, ry + 30.0, 14, theme::TEXT);

        // 得点
        let pts = format!("{}点", format_score(*score));
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
    theme::draw_text_centered(font, "もう一度", cx, btn_y + 31.0, 16, theme::GOLD_LT);
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

// 設定画面のレイアウト定数（描画と入力判定で共有する）
const SETUP_PANEL_W: f32 = 980.0;
const SETUP_PANEL_Y: f32 = 56.0;
const SETUP_PANEL_H: f32 = 612.0;
const SETUP_CARD_PAD: f32 = 40.0;
const SETUP_CARD_GAP: f32 = 20.0;
const SETUP_CARD_Y: f32 = 142.0;
const SETUP_CARD_H: f32 = 348.0;
const SETUP_OPT_H: f32 = 28.0;
const SETUP_OPT_STEP: f32 = 32.0;

// CPU カードの表示ラベル
const SETUP_WINDS: [&str; 3] = ["南", "西", "北"];
const SETUP_CPU_JP: [&str; 3] = ["下家", "対面", "上家"];
const STRENGTH_LABELS: [&str; 3] = ["弱い", "普通", "強い"];
const PERSONALITY_LABELS: [&str; 4] = ["バランス", "スピード", "高得点", "守備的"];

fn setup_panel_x() -> f32 {
    (DESIGN_W - SETUP_PANEL_W) / 2.0
}

fn setup_card_w() -> f32 {
    (SETUP_PANEL_W - 2.0 * SETUP_CARD_PAD - 2.0 * SETUP_CARD_GAP) / 3.0
}

fn setup_card_x(i: usize) -> f32 {
    setup_panel_x() + SETUP_CARD_PAD + i as f32 * (setup_card_w() + SETUP_CARD_GAP)
}

fn setup_opt_rect(cpu_idx: usize, base_offset: f32, opt_idx: usize) -> SetupButton {
    let card_x = setup_card_x(cpu_idx);
    SetupButton {
        x: card_x + 14.0,
        y: SETUP_CARD_Y + base_offset + opt_idx as f32 * SETUP_OPT_STEP,
        w: setup_card_w() - 28.0,
        h: SETUP_OPT_H,
    }
}

const SETUP_STR_OFFSET: f32 = 84.0;
const SETUP_PERS_OFFSET: f32 = 210.0;

fn setup_start_rect() -> SetupButton {
    let y = SETUP_CARD_Y + SETUP_CARD_H + 18.0;
    SetupButton {
        x: DESIGN_W / 2.0 - 120.0,
        y,
        w: 240.0,
        h: 56.0,
    }
}

fn setup_online_rect() -> SetupButton {
    let s = setup_start_rect();
    SetupButton {
        x: DESIGN_W / 2.0 - 110.0,
        y: s.y + s.h + 12.0,
        w: 220.0,
        h: 38.0,
    }
}

/// 設定画面のオプションボタンを 1 個描画する。
fn draw_setup_option(font: Option<&Font>, btn: &SetupButton, label: &str, selected: bool) {
    let (fill, border, text_color) = if selected {
        (theme::rgba(0xc8a227, 0.14), theme::GOLD_DK, theme::GOLD_LT)
    } else {
        (
            Color::new(1.0, 1.0, 1.0, 0.04),
            Color::new(1.0, 1.0, 1.0, 0.07),
            theme::TEXT_DIM,
        )
    };
    theme::draw_rounded_rect(btn.x, btn.y, btn.w, btn.h, 4.0, fill);
    theme::draw_rounded_rect_lines(btn.x, btn.y, btn.w, btn.h, 4.0, 1.0, border);
    draw_jp_text(font, label, btn.x + 12.0, btn.y + 18.0, 13, text_color);
}

/// 設定画面を描画する
fn draw_setup(state: &GameState, font: Option<&Font>) {
    draw_setup_background();
    let setup = &state.setup_state;
    let panel_x = setup_panel_x();

    // パネル背景
    theme::draw_panel(
        panel_x,
        SETUP_PANEL_Y,
        SETUP_PANEL_W,
        SETUP_PANEL_H,
        12.0,
        theme::PANEL_BG,
        theme::PANEL_BORDER,
    );

    // タイトル
    let cx = DESIGN_W / 2.0;
    theme::draw_text_centered(
        font,
        "対局設定",
        cx,
        SETUP_PANEL_Y + 52.0,
        26,
        theme::TEXT_BR,
    );

    // CPU カード
    let card_w = setup_card_w();
    for cpu_idx in 0..3 {
        let card_x = setup_card_x(cpu_idx);
        theme::draw_rounded_rect(
            card_x,
            SETUP_CARD_Y,
            card_w,
            SETUP_CARD_H,
            8.0,
            theme::rgba(0xffffff, 0.03),
        );
        theme::draw_rounded_rect_lines(
            card_x,
            SETUP_CARD_Y,
            card_w,
            SETUP_CARD_H,
            8.0,
            1.0,
            theme::BORDER,
        );

        // ヘッダー：風リング＋名前
        let ring_cx = card_x + 16.0 + 18.0;
        let ring_cy = SETUP_CARD_Y + 16.0 + 18.0;
        draw_circle(ring_cx, ring_cy, 18.0, theme::rgba(0x9a7a1a, 0.30));
        draw_circle_lines(ring_cx, ring_cy, 18.0, 1.5, theme::GOLD_DK);
        theme::draw_text_centered(
            font,
            SETUP_WINDS[cpu_idx],
            ring_cx,
            ring_cy + 6.0,
            16,
            theme::GOLD_LT,
        );
        draw_jp_text(
            font,
            SETUP_CPU_JP[cpu_idx],
            card_x + 56.0,
            SETUP_CARD_Y + 39.0,
            15,
            theme::TEXT,
        );

        // 強さ
        draw_jp_text(
            font,
            "強さ",
            card_x + 14.0,
            SETUP_CARD_Y + 76.0,
            10,
            theme::TEXT_DIM,
        );
        for (level_idx, &label) in STRENGTH_LABELS.iter().enumerate() {
            let btn = setup_opt_rect(cpu_idx, SETUP_STR_OFFSET, level_idx);
            draw_setup_option(font, &btn, label, setup.cpu_levels[cpu_idx] == level_idx);
        }

        // 性格
        draw_jp_text(
            font,
            "性格",
            card_x + 14.0,
            SETUP_CARD_Y + 202.0,
            10,
            theme::TEXT_DIM,
        );
        for (pers_idx, &label) in PERSONALITY_LABELS.iter().enumerate() {
            let btn = setup_opt_rect(cpu_idx, SETUP_PERS_OFFSET, pers_idx);
            draw_setup_option(
                font,
                &btn,
                label,
                setup.cpu_personalities[cpu_idx] == pers_idx,
            );
        }
    }

    // 対局開始ボタン（ゴールド）
    let s = setup_start_rect();
    theme::draw_gradient_button(
        s.x,
        s.y,
        s.w,
        s.h,
        8.0,
        theme::rgb_pub(0x9a7a1a),
        theme::rgb_pub(0x6a5210),
        theme::GOLD,
        2.0,
    );
    theme::draw_text_centered(font, "対局開始", cx, s.y + 34.0, 20, theme::GOLD_LT);

    // オンライン対戦ボタン
    let o = setup_online_rect();
    theme::draw_rounded_rect(o.x, o.y, o.w, o.h, 6.0, theme::rgba(0xffffff, 0.05));
    theme::draw_rounded_rect_lines(o.x, o.y, o.w, o.h, 6.0, 1.0, theme::rgba(0xc8a227, 0.3));
    theme::draw_text_centered(font, "オンライン対戦", cx, o.y + 24.0, 14, theme::TEXT);
}

/// 設定画面での操作
pub enum SetupAction {
    /// ローカル対局を開始する
    StartLocal([CpuConfig; 3]),
    /// オンライン対戦メニューへ
    GoOnline,
}

/// 設定画面の入力を処理する。ボタンが押された場合 Some(action) を返す。
pub fn handle_setup_input(state: &mut GameState, _font: Option<&Font>) -> Option<SetupAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }

    let (mx, my) = mouse_position_design();
    let setup = &mut state.setup_state;

    for cpu_idx in 0..3 {
        // 強さボタン
        for level_idx in 0..SetupState::level_count() {
            if setup_opt_rect(cpu_idx, SETUP_STR_OFFSET, level_idx).contains(mx, my) {
                setup.cpu_levels[cpu_idx] = level_idx;
                return None;
            }
        }
        // 性格ボタン
        for pers_idx in 0..SetupState::personality_count() {
            if setup_opt_rect(cpu_idx, SETUP_PERS_OFFSET, pers_idx).contains(mx, my) {
                setup.cpu_personalities[cpu_idx] = pers_idx;
                return None;
            }
        }
    }

    // 対局開始ボタン
    if setup_start_rect().contains(mx, my) {
        let configs = setup.build_configs();
        state.phase = GamePhase::WaitingForStart;
        return Some(SetupAction::StartLocal(configs));
    }

    // オンライン対戦ボタン
    if setup_online_rect().contains(mx, my) {
        return Some(SetupAction::GoOnline);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{PLAYER_ROTATIONS, seat_at_relative_position};

    #[test]
    fn player_rotations_place_turn_order_counterclockwise() {
        assert_eq!(PLAYER_ROTATIONS, [0.0, -90.0, 180.0, 90.0]);
    }

    #[test]
    fn seat0_relative_positions_match_seat_indices() {
        // 自分が座席0なら相対位置と座席インデックスは一致する（ローカル対局）。
        for rel in 0..4 {
            assert_eq!(seat_at_relative_position(0, rel), rel);
        }
    }

    #[test]
    fn nonzero_seat_maps_relative_positions_to_correct_seats() {
        // オンライン非ホスト: 自分の座席が0以外でも、相対位置→絶対座席へ正しく回る。
        // 自分(0)=座席2, 下家(1)=座席3, 対面(2)=座席0, 上家(3)=座席1。
        assert_eq!(seat_at_relative_position(2, 0), 2);
        assert_eq!(seat_at_relative_position(2, 1), 3);
        assert_eq!(seat_at_relative_position(2, 2), 0);
        assert_eq!(seat_at_relative_position(2, 3), 1);
    }
}
