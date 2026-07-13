//! CPU-setup screen rendering and input.
//!
//! Local play confirms with "start game"; online (opened by the host
//! from the lobby) with "confirm". The game mode is chosen on the mode
//! screen (menu.rs).

use super::*;
use crate::game::MenuOrigin;

// ========== CPU-setup screen ==========

/// Button areas of the setup screen.
pub(super) struct SetupButton {
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

// Layout constants shared by rendering and hit testing.
pub(super) const SETUP_PANEL_W: f32 = 980.0;
pub(super) const SETUP_PANEL_Y: f32 = 56.0;
pub(super) const SETUP_PANEL_H: f32 = 612.0;
pub(super) const SETUP_CARD_PAD: f32 = 40.0;
pub(super) const SETUP_CARD_GAP: f32 = 20.0;
pub(super) const SETUP_CARD_Y: f32 = 142.0;
pub(super) const SETUP_CARD_H: f32 = 348.0;
pub(super) const SETUP_OPT_H: f32 = 28.0;
pub(super) const SETUP_OPT_STEP: f32 = 32.0;

pub(super) fn setup_panel_x() -> f32 {
    (DESIGN_W - SETUP_PANEL_W) / 2.0
}

pub(super) fn setup_card_w() -> f32 {
    (SETUP_PANEL_W - 2.0 * SETUP_CARD_PAD - 2.0 * SETUP_CARD_GAP) / 3.0
}

pub(super) fn setup_card_x(i: usize) -> f32 {
    setup_panel_x() + SETUP_CARD_PAD + i as f32 * (setup_card_w() + SETUP_CARD_GAP)
}

pub(super) fn setup_opt_rect(cpu_idx: usize, base_offset: f32, opt_idx: usize) -> SetupButton {
    let card_x = setup_card_x(cpu_idx);
    SetupButton {
        x: card_x + 14.0,
        y: SETUP_CARD_Y + base_offset + opt_idx as f32 * SETUP_OPT_STEP,
        w: setup_card_w() - 28.0,
        h: SETUP_OPT_H,
    }
}

pub(super) const SETUP_STR_OFFSET: f32 = 84.0;
pub(super) const SETUP_PERS_OFFSET: f32 = 210.0;

pub(super) fn setup_start_rect() -> SetupButton {
    let y = SETUP_CARD_Y + SETUP_CARD_H + 18.0;
    SetupButton {
        x: DESIGN_W / 2.0 - 120.0,
        y,
        w: 240.0,
        h: 56.0,
    }
}

pub(super) fn setup_back_rect() -> SetupButton {
    let s = setup_start_rect();
    SetupButton {
        x: DESIGN_W / 2.0 - 110.0,
        y: s.y + s.h + 12.0,
        w: 220.0,
        h: 38.0,
    }
}

/// Draws one option button.
pub(super) fn draw_setup_option(
    font: Option<&Font>,
    btn: &SetupButton,
    label: &str,
    selected: bool,
) {
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

/// Draws the CPU-setup screen.
///
/// `origin` picks the confirm label: "start game" locally,
/// "confirm" online.
pub(super) fn draw_setup(state: &GameState, font: Option<&Font>, origin: MenuOrigin) {
    draw_setup_background();
    let setup = &state.setup_state;
    let tr = state.tr();
    let panel_x = setup_panel_x();

    theme::draw_panel(
        panel_x,
        SETUP_PANEL_Y,
        SETUP_PANEL_W,
        SETUP_PANEL_H,
        12.0,
        theme::PANEL_BG,
        theme::PANEL_BORDER,
    );

    // Title, with the selected mode appended.
    let cx = DESIGN_W / 2.0;
    theme::draw_text_centered(
        font,
        tr.get(Key::CpuSetupTitle),
        cx,
        SETUP_PANEL_Y + 52.0,
        26,
        theme::TEXT_BR,
    );
    theme::draw_text_centered(
        font,
        tr.get(setup.mode.label_key()),
        cx,
        SETUP_PANEL_Y + 78.0,
        13,
        theme::GOLD_LT,
    );

    // CPU cards (two in three-player games).
    let card_w = setup_card_w();
    for cpu_idx in 0..setup.cpu_count() {
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

        // Header: numbered ring plus name. Seats are randomized at game
        // start, so CPUs show numbers rather than winds/positions.
        let ring_cx = card_x + 16.0 + 18.0;
        let ring_cy = SETUP_CARD_Y + 16.0 + 18.0;
        draw_circle(ring_cx, ring_cy, 18.0, theme::rgba(0x9a7a1a, 0.30));
        draw_circle_lines(ring_cx, ring_cy, 18.0, 1.5, theme::GOLD_DK);
        theme::draw_text_centered(
            font,
            &format!("{}", cpu_idx + 1),
            ring_cx,
            ring_cy + 6.0,
            16,
            theme::GOLD_LT,
        );
        draw_jp_text(
            font,
            &tr.cpu_slot(cpu_idx),
            card_x + 56.0,
            SETUP_CARD_Y + 39.0,
            15,
            theme::TEXT,
        );

        draw_jp_text(
            font,
            tr.get(Key::CpuStrengthLabel),
            card_x + 14.0,
            SETUP_CARD_Y + 76.0,
            10,
            theme::TEXT_DIM,
        );
        for level_idx in 0..SetupState::level_count() {
            let btn = setup_opt_rect(cpu_idx, SETUP_STR_OFFSET, level_idx);
            draw_setup_option(
                font,
                &btn,
                tr.strength_label(level_idx),
                setup.cpu_levels[cpu_idx] == level_idx,
            );
        }

        draw_jp_text(
            font,
            tr.get(Key::CpuPersonalityLabel),
            card_x + 14.0,
            SETUP_CARD_Y + 202.0,
            10,
            theme::TEXT_DIM,
        );
        for pers_idx in 0..SetupState::personality_count() {
            let btn = setup_opt_rect(cpu_idx, SETUP_PERS_OFFSET, pers_idx);
            draw_setup_option(
                font,
                &btn,
                tr.personality_label(pers_idx),
                setup.cpu_personalities[cpu_idx] == pers_idx,
            );
        }
    }

    let confirm_key = match origin {
        MenuOrigin::Local => Key::StartGame,
        MenuOrigin::Online => Key::Confirm,
    };
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
    theme::draw_text_centered(
        font,
        tr.get(confirm_key),
        cx,
        s.y + 34.0,
        20,
        theme::GOLD_LT,
    );

    let b = setup_back_rect();
    theme::draw_rounded_rect(b.x, b.y, b.w, b.h, 6.0, theme::rgba(0xffffff, 0.05));
    theme::draw_rounded_rect_lines(b.x, b.y, b.w, b.h, 6.0, 1.0, theme::rgba(0xc8a227, 0.3));
    theme::draw_text_centered(font, tr.get(Key::Back), cx, b.y + 24.0, 14, theme::TEXT);
}

/// CPU-setup screen actions.
pub enum SetupAction {
    /// Start a local game
    StartLocal([CpuConfig; 3]),
    /// Confirm the CPU setup and return to the lobby (online host)
    ApplyOnline,
    /// Back (mode selection locally, lobby online)
    Back,
}

/// Handles setup-screen input, returning any pressed action.
pub fn handle_setup_input(
    state: &mut GameState,
    _font: Option<&Font>,
    origin: MenuOrigin,
) -> Option<SetupAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }

    let (mx, my) = mouse_position_design();
    let setup = &mut state.setup_state;

    for cpu_idx in 0..setup.cpu_count() {
        for level_idx in 0..SetupState::level_count() {
            if setup_opt_rect(cpu_idx, SETUP_STR_OFFSET, level_idx).contains(mx, my) {
                setup.cpu_levels[cpu_idx] = level_idx;
                return None;
            }
        }
        for pers_idx in 0..SetupState::personality_count() {
            if setup_opt_rect(cpu_idx, SETUP_PERS_OFFSET, pers_idx).contains(mx, my) {
                setup.cpu_personalities[cpu_idx] = pers_idx;
                return None;
            }
        }
    }

    if setup_start_rect().contains(mx, my) {
        return match origin {
            MenuOrigin::Local => {
                let configs = setup.build_configs();
                state.phase = GamePhase::WaitingForStart;
                Some(SetupAction::StartLocal(configs))
            }
            MenuOrigin::Online => Some(SetupAction::ApplyOnline),
        };
    }

    if setup_back_rect().contains(mx, my) {
        return Some(SetupAction::Back);
    }

    None
}
