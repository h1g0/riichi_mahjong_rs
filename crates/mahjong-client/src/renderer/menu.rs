//! Title and mode-selection screens.
//!
//! The title screen is the entry point: local CPU play, online play,
//! settings (not yet implemented), and language. The mode screen picks
//! the game mode (four-player East-only through three-player hanchan)
//! and pei dora, shared by CPU play and online room creation.

use macroquad::prelude::*;

use super::{DESIGN_W, draw_jp_text, mouse_position_design, theme};
use crate::game::{GameMode, GameState, MenuOrigin};
use crate::i18n::Key;
use mahjong_core::settings::Lang;

/// A button rectangle.
struct Rect2 {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect2 {
    fn contains(&self, mx: f32, my: f32) -> bool {
        mx >= self.x && mx < self.x + self.w && my >= self.y && my < self.y + self.h
    }

    fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
    }

    fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

// Shared panel frame for the title and mode screens.
const PANEL_W: f32 = 560.0;
const PANEL_Y: f32 = 110.0;
const PANEL_H: f32 = 580.0;

fn panel_x() -> f32 {
    (DESIGN_W - PANEL_W) / 2.0
}

/// Vertically stacked menu buttons: shared width/X, varying Y.
fn menu_button(y: f32, h: f32) -> Rect2 {
    Rect2 {
        x: DESIGN_W / 2.0 - 180.0,
        y,
        w: 360.0,
        h,
    }
}

// ========== Title screen ==========

fn top_cpu_rect() -> Rect2 {
    menu_button(300.0, 56.0)
}

fn top_online_rect() -> Rect2 {
    menu_button(376.0, 56.0)
}

fn top_settings_rect() -> Rect2 {
    menu_button(452.0, 56.0)
}

/// Language-toggle rectangles; idx 0 = Japanese, 1 = English.
fn top_lang_rect(idx: usize) -> Rect2 {
    const W: f32 = 140.0;
    const H: f32 = 40.0;
    const GAP: f32 = 12.0;
    let left = DESIGN_W / 2.0 - W - GAP / 2.0;
    Rect2 {
        x: left + idx as f32 * (W + GAP),
        y: 588.0,
        w: W,
        h: H,
    }
}

/// Language names shown on the toggle, each in its own language.
const LANG_LABELS: [&str; 2] = ["日本語", "English"];

/// Title-screen actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopMenuAction {
    /// Local CPU play (to mode selection)
    CpuBattle,
    /// Online play (to the online menu)
    Online,
}

/// Draws the shared menu panel and title.
fn draw_panel_background() {
    super::draw_setup_background();
    theme::draw_panel(
        panel_x(),
        PANEL_Y,
        PANEL_W,
        PANEL_H,
        12.0,
        theme::PANEL_BG,
        theme::PANEL_BORDER,
    );
}

fn draw_menu_panel(font: Option<&Font>, title: &str, title_size: u16) {
    draw_panel_background();
    theme::draw_text_centered(
        font,
        title,
        DESIGN_W / 2.0,
        PANEL_Y + 80.0,
        title_size,
        theme::TEXT_BR,
    );
}

/// Draws the title logo (panel background + image).
fn draw_menu_logo(logo: &Texture2D) {
    draw_panel_background();
    let logo_w = PANEL_W - 80.0;
    let logo_h = logo_w * logo.height() / logo.width();
    draw_texture_ex(
        logo,
        DESIGN_W / 2.0 - logo_w / 2.0,
        PANEL_Y + 80.0 - logo_h / 2.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(logo_w, logo_h)),
            ..Default::default()
        },
    );
}

/// Draws a large menu button.
fn draw_menu_button(font: Option<&Font>, rect: &Rect2, label: &str, accent: bool) {
    if accent {
        theme::draw_gradient_button(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            8.0,
            theme::rgb_pub(0x9a7a1a),
            theme::rgb_pub(0x6a5210),
            theme::GOLD,
            2.0,
        );
        theme::draw_text_centered(
            font,
            label,
            rect.center_x(),
            rect.center_y() + 7.0,
            18,
            theme::GOLD_LT,
        );
    } else {
        theme::draw_rounded_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            6.0,
            theme::rgba(0xffffff, 0.05),
        );
        theme::draw_rounded_rect_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            6.0,
            1.0,
            theme::rgba(0xc8a227, 0.3),
        );
        theme::draw_text_centered(
            font,
            label,
            rect.center_x(),
            rect.center_y() + 6.0,
            15,
            theme::TEXT,
        );
    }
}

/// Draws a disabled menu button (unimplemented feature).
fn draw_disabled_button(font: Option<&Font>, rect: &Rect2, label: &str) {
    theme::draw_rounded_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        6.0,
        theme::rgba(0xffffff, 0.02),
    );
    theme::draw_rounded_rect_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        6.0,
        1.0,
        theme::rgba(0xffffff, 0.06),
    );
    theme::draw_text_centered(
        font,
        label,
        rect.center_x(),
        rect.center_y() + 6.0,
        15,
        theme::rgba(0xffffff, 0.25),
    );
}

/// Draws a small toggle button; gold marks the selection.
fn draw_toggle(font: Option<&Font>, rect: &Rect2, label: &str, selected: bool, size: u16) {
    let (fill, border, text_color) = if selected {
        (
            theme::rgba(0xc8a227, 0.13),
            theme::rgba(0xc8a227, 0.45),
            theme::GOLD_LT,
        )
    } else {
        (
            Color::new(1.0, 1.0, 1.0, 0.04),
            Color::new(1.0, 1.0, 1.0, 0.07),
            theme::TEXT_DIM,
        )
    };
    theme::draw_rounded_rect(rect.x, rect.y, rect.w, rect.h, 4.0, fill);
    theme::draw_rounded_rect_lines(rect.x, rect.y, rect.w, rect.h, 4.0, 1.0, border);
    theme::draw_text_centered(
        font,
        label,
        rect.center_x(),
        rect.center_y() + 5.0,
        size,
        text_color,
    );
}

/// Draws the title screen.
pub fn draw_top_menu(state: &GameState, font: Option<&Font>, tile_textures: &super::TileTextures) {
    let tr = state.tr();
    draw_menu_logo(tile_textures.logo());

    draw_menu_button(font, &top_cpu_rect(), tr.get(Key::CpuBattle), true);
    draw_menu_button(font, &top_online_rect(), tr.get(Key::OnlinePlay), true);

    // Rules settings are unimplemented, so the button is disabled.
    let settings_label = format!("{}{}", tr.get(Key::SettingsMenu), tr.get(Key::ComingSoon));
    draw_disabled_button(font, &top_settings_rect(), &settings_label);

    let lang_rect = top_lang_rect(0);
    draw_jp_text(
        font,
        tr.get(Key::LanguageLabel),
        lang_rect.x,
        lang_rect.y - 12.0,
        12,
        theme::TEXT_DIM,
    );
    let active_lang = match state.lang {
        Lang::Ja => 0,
        Lang::En => 1,
    };
    for (idx, &label) in LANG_LABELS.iter().enumerate() {
        draw_toggle(font, &top_lang_rect(idx), label, idx == active_lang, 14);
    }
}

/// Handles title-screen input, returning any pressed action.
pub fn handle_top_menu_input(state: &mut GameState) -> Option<TopMenuAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let (mx, my) = mouse_position_design();

    for (idx, lang) in [Lang::Ja, Lang::En].into_iter().enumerate() {
        if top_lang_rect(idx).contains(mx, my) {
            state.lang = lang;
            crate::persistence::save_lang(lang);
            return None;
        }
    }

    if top_cpu_rect().contains(mx, my) {
        return Some(TopMenuAction::CpuBattle);
    }
    if top_online_rect().contains(mx, my) {
        return Some(TopMenuAction::Online);
    }

    None
}

// ========== Mode-selection screen ==========

/// Mode-button rectangles; idx follows GameMode::ALL order.
fn mode_rect(idx: usize) -> Rect2 {
    menu_button(220.0 + idx as f32 * 66.0, 52.0)
}

/// Pei dora toggle rectangle, under the mode buttons.
fn nuki_rect() -> Rect2 {
    menu_button(504.0, 40.0)
}

/// Back-button rectangle.
fn mode_back_rect() -> Rect2 {
    menu_button(596.0, 40.0)
}

/// Mode-screen actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeSelectAction {
    /// Pick a mode and continue (CPU setup locally, room creation online)
    ModeChosen(GameMode),
    /// Back to the previous screen
    Back,
}

/// Current mode and pei dora values for the given origin.
fn mode_and_nuki(state: &GameState, origin: MenuOrigin) -> (GameMode, bool) {
    match origin {
        MenuOrigin::Local => (state.setup_state.mode, state.setup_state.nuki_dora),
        MenuOrigin::Online => (state.online_state.mode, state.online_state.nuki_dora),
    }
}

/// Draws the mode-selection screen.
pub fn draw_mode_select(state: &GameState, font: Option<&Font>, origin: MenuOrigin) {
    let tr = state.tr();
    draw_menu_panel(font, tr.get(Key::ModeSelectTitle), 26);

    let (current_mode, nuki_dora) = mode_and_nuki(state, origin);

    for (idx, mode) in GameMode::ALL.into_iter().enumerate() {
        draw_toggle(
            font,
            &mode_rect(idx),
            tr.get(mode.label_key()),
            mode == current_mode,
            16,
        );
    }

    // Pei dora toggle; only takes effect for three-player modes.
    let nuki_label = format!(
        "{}{}",
        tr.get(Key::NukiDoraToggle),
        tr.get(Key::SanmaOnlyNote)
    );
    draw_toggle(font, &nuki_rect(), &nuki_label, nuki_dora, 13);

    draw_menu_button(font, &mode_back_rect(), tr.get(Key::Back), false);
}

/// Handles mode-screen input, returning any pressed action.
pub fn handle_mode_select_input(
    state: &mut GameState,
    origin: MenuOrigin,
) -> Option<ModeSelectAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let (mx, my) = mouse_position_design();

    for (idx, mode) in GameMode::ALL.into_iter().enumerate() {
        if mode_rect(idx).contains(mx, my) {
            match origin {
                MenuOrigin::Local => state.setup_state.mode = mode,
                MenuOrigin::Online => state.online_state.mode = mode,
            }
            return Some(ModeSelectAction::ModeChosen(mode));
        }
    }

    if nuki_rect().contains(mx, my) {
        match origin {
            MenuOrigin::Local => {
                state.setup_state.nuki_dora = !state.setup_state.nuki_dora;
            }
            MenuOrigin::Online => {
                state.online_state.nuki_dora = !state.online_state.nuki_dora;
            }
        }
        return None;
    }

    if mode_back_rect().contains(mx, my) {
        return Some(ModeSelectAction::Back);
    }

    None
}
