//! Rendering: draws the tiles from embedded PNGs.

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
use mahjong_core::scoring::score::{DoraLabel, ScoreRank};
use mahjong_core::settings::Lang;
use mahjong_core::tile::{Tile, TileType, dora_indicator_to_dora_in};
use mahjong_server::cpu::client::CpuConfig;

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};

use crate::game::{GamePhase, GameState, SetupState};
use crate::i18n::Key;

const RIICHI_DISABLED_TINT: Color = Color::new(0.45, 0.45, 0.42, 1.0);
const PUBLIC_TILE_HIGHLIGHT_TINT: Color = Color::new(0.48, 0.70, 1.0, 1.0);
const DORA_TILE_TINT: Color = Color::new(1.0, 0.96, 0.72, 1.0);

type DoraTileTypes = [bool; Tile::LEN];

fn add_dora_tile_types(
    dora_tile_types: &mut DoraTileTypes,
    indicators: &[Tile],
    three_player: bool,
) {
    for indicator in indicators {
        let tile_type = dora_indicator_to_dora_in(indicator.get(), three_player);
        dora_tile_types[tile_type as usize] = true;
    }
}

fn dora_tile_types(indicators: &[Tile], three_player: bool) -> DoraTileTypes {
    let mut result = [false; Tile::LEN];
    add_dora_tile_types(&mut result, indicators, three_player);
    result
}

fn dora_tile_tint(tile: &Tile, dora_tile_types: &DoraTileTypes) -> Color {
    dora_tile_tint_with_base(tile, dora_tile_types, WHITE)
}

fn dora_tile_tint_with_base(tile: &Tile, dora_tile_types: &DoraTileTypes, base: Color) -> Color {
    if tile.is_red_dora() || dora_tile_types[tile.get() as usize] {
        Color::new(DORA_TILE_TINT.r, DORA_TILE_TINT.g, DORA_TILE_TINT.b, base.a)
    } else {
        base
    }
}

/// Light complementary-blue tint for a publicly visible tile matching
/// the selected hand tile. Matching is by tile kind, so red and normal
/// fives correspond.
fn public_tile_tint(tile: &Tile, selected_tile_type: Option<TileType>) -> Color {
    public_tile_tint_with_base(tile, selected_tile_type, WHITE)
}

/// Applies the public-tile highlight while preserving an existing
/// transparency such as a called discard's dimmed appearance.
fn public_tile_tint_with_base(
    tile: &Tile,
    selected_tile_type: Option<TileType>,
    base: Color,
) -> Color {
    if selected_tile_type == Some(tile.get()) {
        Color::new(
            PUBLIC_TILE_HIGHLIGHT_TINT.r,
            PUBLIC_TILE_HIGHLIGHT_TINT.g,
            PUBLIC_TILE_HIGHLIGHT_TINT.b,
            base.a,
        )
    } else {
        base
    }
}

fn visible_tile_tint(
    tile: &Tile,
    selected_tile_type: Option<TileType>,
    dora_tile_types: &DoraTileTypes,
) -> Color {
    visible_tile_tint_with_base(tile, selected_tile_type, dora_tile_types, WHITE)
}

fn visible_tile_tint_with_base(
    tile: &Tile,
    selected_tile_type: Option<TileType>,
    dora_tile_types: &DoraTileTypes,
    base: Color,
) -> Color {
    if selected_tile_type == Some(tile.get()) {
        public_tile_tint_with_base(tile, selected_tile_type, base)
    } else {
        dora_tile_tint_with_base(tile, dora_tile_types, base)
    }
}

#[derive(Clone, Copy)]
struct TileTintContext<'a> {
    selected_tile_type: Option<TileType>,
    dora_tile_types: &'a DoraTileTypes,
}

impl<'a> TileTintContext<'a> {
    fn new(selected_tile_type: Option<TileType>, dora_tile_types: &'a DoraTileTypes) -> Self {
        Self {
            selected_tile_type,
            dora_tile_types,
        }
    }

    fn tint(self, tile: &Tile) -> Color {
        visible_tile_tint(tile, self.selected_tile_type, self.dora_tile_types)
    }
}

const TILE_W: f32 = 48.0;
const TILE_H: f32 = 68.0;
const FONT_SIZE: u16 = 20;
const SMALL_FONT: u16 = 16;
const AGARI_FONT: u16 = 32;

/// Every base font size actually used in the app (the `base_size`
/// arguments of `draw_text`/`draw_text_centered`; add new sizes here
/// when introducing them).
///
/// [`prewarm_fonts`] and `cache_dynamic_text` precache exactly these.
/// Brute-forcing `8..=AGARI_FONT` (25 sizes) used to bloat the font
/// atlas for the 16 sizes really in use (see the comment below).
const USED_FONT_SIZES: [u16; 16] = [
    9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 20, 21, 24, 26, 28, 32,
];

/// Design resolution. All UI coordinates live on this virtual canvas and
/// scale to the real window. The HTML keeps this aspect ratio; native
/// windows may be resized to a different one.
pub const DESIGN_W: f32 = 1280.0;
pub const DESIGN_H: f32 = 800.0;

/// Board center: the rotation axis for discard pools and opponents'
/// hands, aligned to the horizontal middle.
const BOARD_CENTER_X: f32 = DESIGN_W / 2.0;
const BOARD_CENTER_Y: f32 = 380.0;

/// Top Y of our hand.
pub const HAND_Y: f32 = 680.0;
/// Gap between the hand and the drawn tile to its right.
pub const DRAWN_GAP: f32 = 20.0;

/// Left X that centers our `hand_len`-tile hand on screen.
///
/// The drawn tile is excluded from centering and hangs to the right, as
/// is conventional. Shared by rendering ([`draw_hand`]) and hit testing
/// (`GameState::handle_input`).
pub fn player_hand_start_x(hand_len: usize) -> f32 {
    let hand_w = hand_len as f32 * TILE_W;
    (DESIGN_W - hand_w) / 2.0
}

/// Camera2D rotations in degrees: self 0, right -90, across 180, left 90.
const PLAYER_ROTATIONS: [f32; 4] = [0.0, -90.0, 180.0, 90.0];

/// Maps a relative position to a [`PLAYER_ROTATIONS`] index.
fn rotation_index(relative_idx: usize, player_count: usize, my_initial_wind_idx: usize) -> usize {
    if player_count == 3 && relative_idx > 0 {
        let their_wind_idx = (my_initial_wind_idx + relative_idx) % 3;
        (their_wind_idx + 4 - my_initial_wind_idx) % 4
    } else {
        relative_idx
    }
}

/// Maps a relative position (0 self, 1 right, 2 across, 3 left) to the
/// fixed seat index. `scores` and `player_labels` are seat-ordered, so
/// every per-direction draw goes through this - keeping seats right even
/// when a non-host sits somewhere other than seat 0 online.
fn seat_at_relative_position(my_seat: usize, relative_idx: usize, player_count: usize) -> usize {
    (my_seat + relative_idx) % player_count
}

/// Camera mapping design space (0,0)-(DESIGN_W,DESIGN_H) onto the whole
/// canvas, independent of the real buffer resolution, so layout scales
/// with the window.
///
/// `Camera2D::from_display_rect` (negative zoom.y) would flip vertically
/// when drawing straight to the screen, so zoom.y stays positive like
/// the board camera.
fn design_camera() -> Camera2D {
    Camera2D {
        target: vec2(DESIGN_W / 2.0, DESIGN_H / 2.0),
        zoom: vec2(2.0 / DESIGN_W, 2.0 / DESIGN_H),
        ..Default::default()
    }
}

/// Applies the default design-space camera (frame start, overlays).
pub fn set_design_camera() {
    set_camera(&design_camera());
}

/// Converts a buffer-space point to the coordinates used by the design
/// camera. X and Y need independent scales when a native window is
/// resized away from the design aspect ratio.
fn buffer_to_design(mx: f32, my: f32, buffer_w: f32, buffer_h: f32) -> (f32, f32) {
    if buffer_w <= 0.0 || buffer_h <= 0.0 {
        return (0.0, 0.0);
    }
    (mx * DESIGN_W / buffer_w, my * DESIGN_H / buffer_h)
}

/// Mouse position converted from buffer to design coordinates, where all
/// hit testing happens.
pub fn mouse_position_design() -> (f32, f32) {
    let (mx, my) = mouse_position();
    buffer_to_design(mx, my, screen_width(), screen_height())
}

/// Camera2D rotating about the board center.
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

/// Tile-face PNGs, indexed by [`Tile`] kind value.
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
    /// English-UI set with index labels baked into the corner
    /// (see [`labels`])
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
    /// Whether the labeled set is active (refreshed from the language
    /// every frame)
    labels_enabled: std::cell::Cell<bool>,
}

impl TileTextures {
    /// Loads every texture. `font_bytes` is the TTF used to bake labels;
    /// when unavailable the unlabeled set stands in.
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

    /// The title-screen logo texture.
    pub fn logo(&self) -> &Texture2D {
        &self.logo
    }

    /// Switches the labeled set on or off; [`draw_game`] sets it from the
    /// language every frame, so runtime language switches apply at once.
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

/// Bakes index labels into the tile images and uploads textures;
/// without a font the plain images are uploaded.
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

/// Builds the font atlas to its final size at startup (native only).
///
/// On native (OpenGL), if the atlas grows while text is being drawn
/// under a rotated camera, the atlas texture is deleted and recreated,
/// corrupting the unflushed batch and rendering glyphs as black squares
/// (the game screen hits this because it switches cameras mid-frame).
/// Pre-building the atlas with draw-less `measure` calls prevents any
/// mid-frame growth.
///
/// Never call this on WASM (`#[cfg(not(target_arch = "wasm32"))]`).
/// Caching many glyph/size combinations at once makes macroquad
/// 0.4.15's atlas grow repeatedly, and on wasm32 (32-bit usize)
/// `Image::gen_image_color`'s `width * height * 4` hits exactly 2^32 at
/// 32768x32768, overflows to a zero-length buffer, and the next write
/// panics on the bounds check (a macroquad bug). The growth depends on
/// `HashMap` iteration order and is nondeterministic, so trimming the
/// glyph/size set only lowers the odds. Skipping the prewarm lets WASM
/// cache glyphs one at a time as they appear, avoiding mass repacking
/// entirely (the black-square issue is native-only; verified absent on
/// WASM without the prewarm).
#[cfg(not(target_arch = "wasm32"))]
pub fn prewarm_fonts(font: Option<&Font>) {
    // Every glyph the UI shows (generated by scripts/extract_glyphs.py).
    let mut glyphs: String = include_str!("../../glyphs.txt").to_string();
    // ASCII too: digits, punctuation, Latin text like "CPU".
    for c in 0x20u8..0x7f {
        glyphs.push(c as char);
    }
    for &base in &USED_FONT_SIZES {
        let _ = theme::measure_scaled(font, &glyphs, base);
    }
}

/// Precaches dynamic text (opponent names, input fields, connection
/// status, room codes - external strings that may contain any glyph)
/// before this frame draws.
///
/// [`prewarm_fonts`] only covers fixed UI strings, so these are measured
/// into the atlas every frame, preventing mid-draw atlas growth (and the
/// black-square glyphs) especially during the game screen's camera
/// switches.
///
/// Sizes are limited to [`USED_FONT_SIZES`], as in [`prewarm_fonts`]
/// (see that function's comment for why).
pub fn cache_dynamic_text(font: Option<&Font>, state: &GameState) {
    use crate::game::PlayerLabel;
    let cache = |s: &str, sizes: &[u16]| {
        if s.is_empty() {
            return;
        }
        debug_assert!(sizes.iter().all(|size| USED_FONT_SIZES.contains(size)));
        for &base in sizes {
            let _ = theme::measure_scaled(font, s, base);
        }
    };
    for label in &state.player_labels {
        if let PlayerLabel::Human(name) = label {
            // Score chip, center detail, ranking, and win heading.
            cache(name, &[9, 11, 14, 21]);
        }
    }
    let online = &state.online_state;
    cache(&online.name_input, &[16]);
    cache(&online.code_input, &[16]);
    if let Some(status) = &online.status_line {
        cache(status, &[13, 15]);
    }
    if let Some(room) = &online.room {
        cache(&room.code, &[28]);
        for label in &room.seat_labels {
            cache(label, &[14]);
        }
    }

    // Round results (yaku names, ranks, winner names, draw messages)
    // are external too.
    if let Some(message) = &state.result_message {
        cache(message, &[14, 20]);
    }
    if let Some(result) = state.current_win_result() {
        cache(&result.winner_name, &[21]);
        if let Some(loser) = &result.loser_name {
            cache(loser, &[21]);
        }
        cache(&result.rank_name, &[28]);
        for (name, _) in &result.yaku {
            cache(name, &[14]);
        }
    }
}

pub fn draw_game(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) -> Option<OverlayClick> {
    // The English UI uses the corner-labeled tile set.
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
            // Let fading win declarations finish even right after the
            // result screen appears.
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
