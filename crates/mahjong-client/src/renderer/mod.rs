//! 描画モジュール
//!
//! 埋め込みPNGを使って麻雀牌を描画する。

mod banners;
mod board;
mod labels;
mod menu;
mod online;
mod overlay;
mod result;
mod setup;
#[cfg(test)]
mod tests;
mod theme;
mod tiles;

pub use menu::{ModeSelectAction, TopMenuAction, handle_mode_select_input, handle_top_menu_input};
pub use setup::{SetupAction, handle_setup_input};

use board::*;
pub use online::{
    OnlineLobbyAction, OnlineMenuAction, handle_online_lobby_input, handle_online_menu_input,
};
pub use overlay::OverlayClick;
use result::*;
use setup::*;
use tiles::*;

use macroquad::prelude::*;
use mahjong_core::scoring::score::DoraLabel;
use mahjong_core::settings::Lang;
use mahjong_core::tile::Tile;
use mahjong_server::cpu::client::CpuConfig;

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

use crate::game::{GamePhase, GameState, SetupState};
use crate::i18n::Key;

const RIICHI_DISABLED_TINT: Color = Color::new(0.45, 0.45, 0.42, 1.0);

const TILE_W: f32 = 48.0;
const TILE_H: f32 = 68.0;
const FONT_SIZE: u16 = 20;
const SMALL_FONT: u16 = 16;
const AGARI_FONT: u16 = 32;

/// アプリ全体で実際に使われている基準フォントサイズ（`draw_text`/`draw_text_centered`
/// 呼び出しの `base_size` 引数を網羅した一覧。新しいサイズを使う描画を追加したら
/// ここにも追加すること）。
///
/// [`prewarm_fonts`] と `cache_dynamic_text` はこの一覧のサイズだけを事前キャッシュする。
/// かつては `8..=AGARI_FONT`（25通り）を無差別に総当たりしていたが、実際に使うのは
/// この16通りだけであり、無駄な水増しがフォントアトラスの肥大化を招いていた
/// （下記コメント参照）。
const USED_FONT_SIZES: [u16; 16] = [
    9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 20, 21, 24, 26, 28, 32,
];

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

/// 相対位置を回転テーブル [`PLAYER_ROTATIONS`] のインデックスへ変換する。
///
/// 四麻: そのまま（0=自分, 1=下家=右, 2=対面=上, 3=上家=左）。
/// 三麻: 相対2が上家になるため、左（90°）の描画パスを再利用する
/// （自分=下、下家=右、上家=左、上辺は空席）。
fn rotation_index(relative_idx: usize, player_count: usize) -> usize {
    if player_count == 3 && relative_idx == 2 {
        3
    } else {
        relative_idx
    }
}

/// 自分から見た相対位置(0=自分,1=下家,2=対面,3=上家)を固定の座席インデックスへ変換する。
/// `scores` や `player_labels` は座席インデックス順に並ぶため、画面の各向きへ描画する際は
/// この変換を通す（オンライン非ホストで自分の座席が0以外でも正しい席に表示される）。
fn seat_at_relative_position(my_seat: usize, relative_idx: usize, player_count: usize) -> usize {
    (my_seat + relative_idx) % player_count
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

/// 牌の表面PNG（インデックスは [`Tile`] の種別値と一致）。
const TILE_PNGS: [&[u8]; Tile::LEN] = [
    include_bytes!("../../../../assets/images/tiles/1m.png"),
    include_bytes!("../../../../assets/images/tiles/2m.png"),
    include_bytes!("../../../../assets/images/tiles/3m.png"),
    include_bytes!("../../../../assets/images/tiles/4m.png"),
    include_bytes!("../../../../assets/images/tiles/5m.png"),
    include_bytes!("../../../../assets/images/tiles/6m.png"),
    include_bytes!("../../../../assets/images/tiles/7m.png"),
    include_bytes!("../../../../assets/images/tiles/8m.png"),
    include_bytes!("../../../../assets/images/tiles/9m.png"),
    include_bytes!("../../../../assets/images/tiles/1p.png"),
    include_bytes!("../../../../assets/images/tiles/2p.png"),
    include_bytes!("../../../../assets/images/tiles/3p.png"),
    include_bytes!("../../../../assets/images/tiles/4p.png"),
    include_bytes!("../../../../assets/images/tiles/5p.png"),
    include_bytes!("../../../../assets/images/tiles/6p.png"),
    include_bytes!("../../../../assets/images/tiles/7p.png"),
    include_bytes!("../../../../assets/images/tiles/8p.png"),
    include_bytes!("../../../../assets/images/tiles/9p.png"),
    include_bytes!("../../../../assets/images/tiles/1s.png"),
    include_bytes!("../../../../assets/images/tiles/2s.png"),
    include_bytes!("../../../../assets/images/tiles/3s.png"),
    include_bytes!("../../../../assets/images/tiles/4s.png"),
    include_bytes!("../../../../assets/images/tiles/5s.png"),
    include_bytes!("../../../../assets/images/tiles/6s.png"),
    include_bytes!("../../../../assets/images/tiles/7s.png"),
    include_bytes!("../../../../assets/images/tiles/8s.png"),
    include_bytes!("../../../../assets/images/tiles/9s.png"),
    include_bytes!("../../../../assets/images/tiles/1z.png"),
    include_bytes!("../../../../assets/images/tiles/2z.png"),
    include_bytes!("../../../../assets/images/tiles/3z.png"),
    include_bytes!("../../../../assets/images/tiles/4z.png"),
    include_bytes!("../../../../assets/images/tiles/5z.png"),
    include_bytes!("../../../../assets/images/tiles/6z.png"),
    include_bytes!("../../../../assets/images/tiles/7z.png"),
];

pub struct TileTextures {
    standard_tiles: Vec<Texture2D>,
    /// 英語UI用: 右上にインデックスラベルを焼き込んだ牌（[`labels`] 参照）
    labeled_tiles: Vec<Texture2D>,
    red_5m: Texture2D,
    red_5p: Texture2D,
    red_5s: Texture2D,
    labeled_red_5m: Texture2D,
    labeled_red_5p: Texture2D,
    labeled_red_5s: Texture2D,
    back: Texture2D,
    stick1000: Texture2D,
    stick100: Texture2D,
    logo: Texture2D,
    /// ラベル付きセットを使うか（言語設定から毎フレーム更新される）
    labels_enabled: std::cell::Cell<bool>,
}

impl TileTextures {
    /// 全テクスチャを読み込む。`font_bytes` はラベル焼き込みに使う TTF
    /// （読めない場合はラベルなしのテクスチャで代替する）。
    pub fn load(font_bytes: &[u8]) -> Self {
        let label_font =
            fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()).ok();

        let mut standard_tiles = Vec::with_capacity(Tile::LEN);
        let mut labeled_tiles = Vec::with_capacity(Tile::LEN);
        for (tile_type, png) in TILE_PNGS.iter().enumerate() {
            let img = image_from_png(png);
            standard_tiles.push(texture_from_image(&img));
            labeled_tiles.push(labeled_texture(
                img,
                tile_type as mahjong_core::tile::TileType,
                label_font.as_ref(),
            ));
        }

        let red_5m_img = image_from_png(include_bytes!("../../../../assets/images/tiles/r5m.png"));
        let red_5p_img = image_from_png(include_bytes!("../../../../assets/images/tiles/r5p.png"));
        let red_5s_img = image_from_png(include_bytes!("../../../../assets/images/tiles/r5s.png"));

        Self {
            standard_tiles,
            labeled_tiles,
            red_5m: texture_from_image(&red_5m_img),
            red_5p: texture_from_image(&red_5p_img),
            red_5s: texture_from_image(&red_5s_img),
            labeled_red_5m: labeled_texture(red_5m_img, Tile::M5, label_font.as_ref()),
            labeled_red_5p: labeled_texture(red_5p_img, Tile::P5, label_font.as_ref()),
            labeled_red_5s: labeled_texture(red_5s_img, Tile::S5, label_font.as_ref()),
            back: load_texture_from_png(include_bytes!("../../../../assets/images/tiles/back.png")),
            stick1000: load_texture_from_png(include_bytes!(
                "../../../../assets/images/sticks/stick1000.png"
            )),
            stick100: load_texture_from_png(include_bytes!(
                "../../../../assets/images/sticks/stick100.png"
            )),
            logo: load_texture_from_png(include_bytes!(
                "../../../../assets/images/others/logo.png"
            )),
            labels_enabled: std::cell::Cell::new(false),
        }
    }

    /// トップ画面ロゴのテクスチャ
    pub fn logo(&self) -> &Texture2D {
        &self.logo
    }

    /// ラベル付きセットを使うかを切り替える（[`draw_game`] が言語設定から
    /// 毎フレーム設定するため、実行中の言語切替にも即応する）。
    fn set_labels_enabled(&self, enabled: bool) {
        self.labels_enabled.set(enabled);
    }

    fn for_tile(&self, tile: &Tile) -> &Texture2D {
        let labeled = self.labels_enabled.get();
        if tile.is_red_dora() {
            match (tile.get(), labeled) {
                (Tile::M5, false) => return &self.red_5m,
                (Tile::P5, false) => return &self.red_5p,
                (Tile::S5, false) => return &self.red_5s,
                (Tile::M5, true) => return &self.labeled_red_5m,
                (Tile::P5, true) => return &self.labeled_red_5p,
                (Tile::S5, true) => return &self.labeled_red_5s,
                _ => {}
            }
        }

        if labeled {
            &self.labeled_tiles[tile.get() as usize]
        } else {
            &self.standard_tiles[tile.get() as usize]
        }
    }
}

fn image_from_png(bytes: &[u8]) -> Image {
    Image::from_file_with_format(bytes, Some(ImageFormat::Png))
        .expect("組み込み牌PNGのデコードに失敗")
}

fn texture_from_image(img: &Image) -> Texture2D {
    let texture = Texture2D::from_image(img);
    texture.set_filter(FilterMode::Linear);
    texture
}

/// 牌画像へインデックスラベルを焼き込んでテクスチャ化する。
/// フォントがない場合はラベルなしでテクスチャ化する。
fn labeled_texture(
    mut img: Image,
    tile_type: mahjong_core::tile::TileType,
    font: Option<&fontdue::Font>,
) -> Texture2D {
    if let Some(font) = font {
        let (label, color) = labels::tile_index_label(tile_type);
        labels::bake_label(&mut img, label, color, font);
    }
    texture_from_image(&img)
}

fn load_texture_from_png(bytes: &[u8]) -> Texture2D {
    let texture = Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png));
    texture.set_filter(FilterMode::Linear);
    texture
}

fn draw_jp_text(font: Option<&Font>, text: &str, x: f32, y: f32, font_size: u16, color: Color) {
    theme::draw_text(font, text, x, y, font_size, color);
}

/// 起動時にフォントアトラスを必要なグリフ・サイズで作り切る（ネイティブ専用）。
///
/// ネイティブ(OpenGL)では、回転カメラ下のテキスト描画中にフォントアトラスが
/// 拡張されると、アトラステクスチャが delete→再生成され、未フラッシュの描画
/// バッチが壊れて文字が黒い■に化ける（対局画面はカメラを切り替えながら描く
/// ため発症する）。描画を伴わない `measure` で事前にアトラスを最終サイズまで
/// 構築しておけば、フレーム途中での拡張が起きず発症しない。
///
/// WASM では呼ばない（`#[cfg(not(target_arch = "wasm32"))]`）。一度に大量の
/// グリフ×サイズをキャッシュしようとすると、macroquad 0.4.15 のフォント
/// アトラスが grow を繰り返し、wasm32 では usize が 32bit のため
/// `Image::gen_image_color` の `width as usize * height as usize * 4` が
/// 32768×32768 でちょうど 2^32 に達してオーバーフローし、0 バイトのバッファを
/// 確保してしまって直後の書き込みで境界チェックパニックが発生する（macroquad
/// 側の既存バグ）。この成長は内部で `HashMap` の反復順に依存して非決定的な
/// ため、キャッシュするグリフ数・サイズ数を減らしても確率が下がるだけで
/// 根絶はできない。WASM では prewarm を行わず、実際に画面へ現れた文字を
/// その都度キャッシュさせることで、一度に大量の再パッキングが必要になる
/// 事態そのものを避ける（黒■化はネイティブ限定の問題であり、WASM で
/// prewarm を省いても発生しないことを実機で確認済み）。
#[cfg(not(target_arch = "wasm32"))]
pub fn prewarm_fonts(font: Option<&Font>) {
    // UI に現れる全グリフ（生成スクリプト: scripts/extract_glyphs.py）
    let mut glyphs: String = include_str!("../../glyphs.txt").to_string();
    // ASCII（数字・記号・CPU などのラテン文字）も網羅する
    for c in 0x20u8..0x7f {
        glyphs.push(c as char);
    }
    for &base in &USED_FONT_SIZES {
        let _ = theme::measure_scaled(font, &glyphs, base);
    }
}

/// 動的テキスト（対戦相手の名前・入力欄・接続状態・ルームコードなど、外部由来で
/// 任意のグリフを含み得る文字列）を、このフレームの描画前に事前キャッシュする。
///
/// [`prewarm_fonts`] は固定 UI 文言しか網羅できないため、こうした文字は毎フレーム
/// measure してアトラスへ載せておく。これにより、描画途中（特に対局画面のカメラ
/// 切り替え中）にアトラスが拡張されて文字が黒い■に化けるのを防ぐ。
///
/// キャッシュするサイズは [`prewarm_fonts`] と同様 [`USED_FONT_SIZES`] のみに限定する
/// （理由も同関数のコメントを参照）。
pub fn cache_dynamic_text(font: Option<&Font>, state: &GameState) {
    use crate::game::PlayerLabel;
    let cache = |s: &str| {
        if s.is_empty() {
            return;
        }
        for &base in &USED_FONT_SIZES {
            let _ = theme::measure_scaled(font, s, base);
        }
    };
    for label in &state.player_labels {
        if let PlayerLabel::Human(name) = label {
            cache(name);
        }
    }
    let online = &state.online_state;
    cache(&online.name_input);
    cache(&online.code_input);
    if let Some(status) = &online.status_line {
        cache(status);
    }
    if let Some(room) = &online.room {
        cache(&room.code);
        for label in &room.seat_labels {
            cache(label);
        }
    }

    // 局結果（役名・等級名・和了者名・流局メッセージ）も外部由来なので備える
    if let Some(message) = &state.result_message {
        cache(message);
    }
    if let Some(result) = state.current_win_result() {
        cache(&result.winner_name);
        if let Some(loser) = &result.loser_name {
            cache(loser);
        }
        cache(&result.rank_name);
        for (name, _) in &result.yaku {
            cache(name);
        }
    }
}

pub fn draw_game(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) -> Option<OverlayClick> {
    // 英語UIでは牌の右上にインデックスラベル付きのテクスチャを使う
    tile_textures.set_labels_enabled(state.tr().lang() == Lang::En);

    match state.phase {
        GamePhase::TopMenu => {
            menu::draw_top_menu(state, font, tile_textures);
            None
        }
        GamePhase::ModeSelect(origin) => {
            menu::draw_mode_select(state, font, origin);
            None
        }
        GamePhase::CpuSetup(origin) => {
            draw_setup(state, font, origin);
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
                state.tr().get(Key::GameStarting),
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
            banners::draw_call_banners(state, font);
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
            // 結果画面へ切り替わった直後もフェード中のロン・ツモ宣言を出し切る
            banners::draw_call_banners(state, font);
            online::draw_connection_banner(state, font);
            None
        }
        GamePhase::GameOver => {
            draw_game_over(state, font);
            None
        }
    }
}
