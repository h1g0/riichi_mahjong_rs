//! Online menu and lobby screens: rendering and input only. Network
//! operations happen in the main loop, which receives the actions.

use macroquad::prelude::*;

use super::{DESIGN_W, draw_jp_text, theme};
use crate::game::GameState;
use crate::i18n::Key;

/// Panel layout, matching the setup screen.
const PANEL_X: f32 = 150.0;
const PANEL_Y: f32 = 50.0;
const PANEL_W: f32 = 980.0;
const PANEL_H: f32 = 690.0;

/// Button / input-field rectangle.
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
}

/// Maximum display-name length.
const NAME_MAX_CHARS: usize = 12;
/// Room-code length.
const CODE_MAX_CHARS: usize = 6;

// Layout constants for the 1280x800 window.
const NAME_BOX: Rect2 = Rect2 {
    x: 440.0,
    y: 250.0,
    w: 400.0,
    h: 44.0,
};
const CODE_BOX: Rect2 = Rect2 {
    x: 440.0,
    y: 350.0,
    w: 400.0,
    h: 44.0,
};
const CREATE_BTN: Rect2 = Rect2 {
    x: 440.0,
    y: 450.0,
    w: 400.0,
    h: 50.0,
};
const JOIN_BTN: Rect2 = Rect2 {
    x: 440.0,
    y: 520.0,
    w: 400.0,
    h: 50.0,
};
const BACK_BTN: Rect2 = Rect2 {
    x: 440.0,
    y: 610.0,
    w: 400.0,
    h: 40.0,
};
/// The lobby's CPU-setup button (host only).
const CPU_SETUP_BTN: Rect2 = Rect2 {
    x: 440.0,
    y: 500.0,
    w: 400.0,
    h: 44.0,
};
const START_BTN: Rect2 = Rect2 {
    x: 440.0,
    y: 560.0,
    w: 400.0,
    h: 56.0,
};
const LEAVE_BTN: Rect2 = Rect2 {
    x: 440.0,
    y: 650.0,
    w: 400.0,
    h: 40.0,
};

/// Online-menu actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineMenuAction {
    /// Create a room
    CreateRoom,
    /// Join by room code
    JoinRoom,
    /// Back
    Back,
}

/// Lobby actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineLobbyAction {
    /// Open the CPU setup (host only)
    OpenCpuSettings,
    /// Start the game (host only)
    StartGame,
    /// Leave
    Leave,
}

/// Draws the shared online panel and title.
fn draw_online_panel(font: Option<&Font>, title: &str) {
    super::draw_setup_background();
    theme::draw_panel(
        PANEL_X,
        PANEL_Y,
        PANEL_W,
        PANEL_H,
        12.0,
        theme::PANEL_BG,
        theme::PANEL_BORDER,
    );
    let cx = DESIGN_W / 2.0;
    theme::draw_text_centered(font, title, cx, PANEL_Y + 60.0, 26, theme::TEXT_BR);
}

fn draw_button(font: Option<&Font>, rect: &Rect2, label: &str, accent: bool) {
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
            rect.y + rect.h / 2.0 + 7.0,
            17,
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
            rect.y + rect.h / 2.0 + 6.0,
            15,
            theme::TEXT,
        );
    }
}

fn draw_input_box(font: Option<&Font>, rect: &Rect2, text: &str, focused: bool) {
    theme::draw_rounded_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        6.0,
        theme::rgba(0x000000, 0.4),
    );
    let border = if focused {
        theme::GOLD_LT
    } else {
        theme::rgba(0xffffff, 0.12)
    };
    theme::draw_rounded_rect_lines(rect.x, rect.y, rect.w, rect.h, 6.0, 1.5, border);
    // Contents drawn with a cursor.
    let shown = if focused {
        format!("{text}_")
    } else {
        text.to_string()
    };
    draw_jp_text(
        font,
        &shown,
        rect.x + 14.0,
        rect.y + rect.h / 2.0 + 7.0,
        16,
        theme::TEXT,
    );
}

fn draw_status_line(state: &GameState, font: Option<&Font>, y: f32) {
    if let Some(line) = &state.online_state.status_line {
        let color = if state.online_state.status_is_error {
            theme::RED_LT
        } else {
            theme::TEXT_DIM
        };
        theme::draw_text_centered(font, line, DESIGN_W / 2.0, y, 15, color);
    }
}

/// Draws the online menu.
pub fn draw_online_menu(state: &GameState, font: Option<&Font>) {
    let online = &state.online_state;
    let tr = state.tr();

    draw_online_panel(font, tr.get(Key::OnlinePlay));

    draw_jp_text(
        font,
        tr.get(Key::NameLabel),
        NAME_BOX.x,
        NAME_BOX.y - 9.0,
        11,
        theme::TEXT_DIM,
    );
    draw_input_box(font, &NAME_BOX, &online.name_input, !online.code_focused);

    draw_jp_text(
        font,
        tr.get(Key::RoomCodeJoinLabel),
        CODE_BOX.x,
        CODE_BOX.y - 9.0,
        11,
        theme::TEXT_DIM,
    );
    draw_input_box(font, &CODE_BOX, &online.code_input, online.code_focused);

    draw_button(font, &CREATE_BTN, tr.get(Key::CreateRoom), true);
    draw_button(font, &JOIN_BTN, tr.get(Key::JoinRoom), true);
    draw_button(font, &BACK_BTN, tr.get(Key::Back), false);

    draw_status_line(state, font, BACK_BTN.y + BACK_BTN.h + 30.0);
}

/// Handles online-menu input.
pub fn handle_online_menu_input(state: &mut GameState) -> Option<OnlineMenuAction> {
    let online = &mut state.online_state;

    // Text entry goes to the focused field.
    while let Some(c) = get_char_pressed() {
        if c.is_control() {
            continue;
        }
        if online.code_focused {
            let c = c.to_ascii_uppercase();
            // Accept only room-code characters (no confusable 0/O/1/I).
            if online.code_input.chars().count() < CODE_MAX_CHARS
                && c.is_ascii_alphanumeric()
                && !"0O1I".contains(c)
            {
                online.code_input.push(c);
            }
        } else if online.name_input.chars().count() < NAME_MAX_CHARS {
            online.name_input.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        if online.code_focused {
            online.code_input.pop();
        } else {
            online.name_input.pop();
        }
    }
    if is_key_pressed(KeyCode::Tab) {
        online.code_focused = !online.code_focused;
    }

    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let (mx, my) = super::mouse_position_design();

    if NAME_BOX.contains(mx, my) {
        online.code_focused = false;
        return None;
    }
    if CODE_BOX.contains(mx, my) {
        online.code_focused = true;
        return None;
    }

    if CREATE_BTN.contains(mx, my) {
        return Some(OnlineMenuAction::CreateRoom);
    }
    if JOIN_BTN.contains(mx, my) {
        return Some(OnlineMenuAction::JoinRoom);
    }
    if BACK_BTN.contains(mx, my) {
        return Some(OnlineMenuAction::Back);
    }

    None
}

/// Draws the lobby.
pub fn draw_online_lobby(state: &GameState, font: Option<&Font>) {
    let online = &state.online_state;
    let tr = state.tr();
    let cx = DESIGN_W / 2.0;

    draw_online_panel(font, tr.get(Key::Lobby));

    let Some(room) = &online.room else {
        theme::draw_text_centered(font, tr.get(Key::LoadingRoom), cx, 300.0, 18, theme::TEXT);
        draw_status_line(state, font, 340.0);
        return;
    };

    // The room code, for sharing.
    theme::draw_text_centered(
        font,
        &tr.room_code(&room.code),
        cx,
        210.0,
        28,
        theme::GOLD_LT,
    );
    theme::draw_text_centered(
        font,
        tr.get(Key::ShareCodeHint),
        cx,
        236.0,
        12,
        theme::TEXT_DIM,
    );
    // Spell out the room's game mode.
    theme::draw_text_centered(
        font,
        tr.get(room.mode.label_key()),
        cx,
        256.0,
        13,
        theme::GOLD_LT,
    );

    // Seat list; three-player rooms leave the unused seat blank.
    let row_x = 440.0;
    let row_w = 400.0;
    for (i, label) in room
        .seat_labels
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.is_empty())
    {
        let y = 282.0 + i as f32 * 46.0;
        let is_me = label.contains(tr.get(Key::MarkerYou));
        let (fill, border) = if is_me {
            (theme::rgba(0xc8a227, 0.07), theme::rgba(0xc8a227, 0.20))
        } else {
            (theme::rgba(0xffffff, 0.03), theme::rgba(0xffffff, 0.05))
        };
        theme::draw_rounded_rect(row_x, y, row_w, 38.0, 6.0, fill);
        theme::draw_rounded_rect_lines(row_x, y, row_w, 38.0, 6.0, 1.0, border);
        draw_jp_text(font, label, row_x + 14.0, y + 24.0, 14, theme::TEXT);
    }

    if room.is_host {
        theme::draw_text_centered(
            font,
            tr.get(Key::EmptySeatsCpu),
            cx,
            CPU_SETUP_BTN.y - 10.0,
            12,
            theme::TEXT_DIM,
        );
        draw_button(font, &CPU_SETUP_BTN, tr.get(Key::CpuSetupTitle), false);
        draw_button(font, &START_BTN, tr.get(Key::StartGame), true);
    } else {
        theme::draw_text_centered(
            font,
            tr.get(Key::WaitingHost),
            cx,
            START_BTN.y + 34.0,
            16,
            theme::TEXT_DIM,
        );
    }
    draw_button(font, &LEAVE_BTN, tr.get(Key::Leave), false);

    draw_status_line(state, font, LEAVE_BTN.y + LEAVE_BTN.h + 28.0);
}

/// Handles lobby input.
pub fn handle_online_lobby_input(state: &GameState) -> Option<OnlineLobbyAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let (mx, my) = super::mouse_position_design();

    let is_host = state
        .online_state
        .room
        .as_ref()
        .is_some_and(|room| room.is_host);
    if is_host && CPU_SETUP_BTN.contains(mx, my) {
        return Some(OnlineLobbyAction::OpenCpuSettings);
    }
    if is_host && START_BTN.contains(mx, my) {
        return Some(OnlineLobbyAction::StartGame);
    }
    if LEAVE_BTN.contains(mx, my) {
        return Some(OnlineLobbyAction::Leave);
    }

    None
}

/// Draws the in-game connection banner.
pub fn draw_connection_banner(state: &GameState, font: Option<&Font>) {
    if let Some(line) = &state.online_state.status_line {
        let w = 440.0;
        let x = (DESIGN_W - w) / 2.0;
        // A rounded red banner right under the top bar.
        theme::draw_rounded_rect(x, 56.0, w, 30.0, 6.0, theme::rgba(0x7a1010, 0.92));
        theme::draw_rounded_rect_lines(x, 56.0, w, 30.0, 6.0, 1.0, theme::RED);
        theme::draw_text_centered(font, line, DESIGN_W / 2.0, 76.0, 13, WHITE);
    }
}

/// Draws the turn-timer countdown, shown only while our action is
/// awaited online.
pub fn draw_turn_timer(state: &GameState, font: Option<&Font>) {
    let Some(remaining) = state.online_state.turn_remaining else {
        return;
    };

    let my_turn = state.is_my_turn || !state.available_calls.is_empty();
    if !my_turn {
        return;
    }

    // Red at ten seconds or less, gold otherwise.
    let (accent, border) = if remaining <= 10 {
        (theme::RED_LT, theme::RED)
    } else {
        (theme::GOLD_LT, theme::GOLD_DK)
    };
    let w = 130.0;
    let h = 34.0;
    let x = 880.0;
    let y = 632.0;
    theme::draw_rounded_rect(x, y, w, h, 6.0, theme::rgba(0x000000, 0.55));
    theme::draw_rounded_rect_lines(x, y, w, h, 6.0, 1.0, border);
    theme::draw_text_centered(
        font,
        &state.tr().seconds_left(remaining),
        x + w / 2.0,
        y + h / 2.0 + 6.0,
        16,
        accent,
    );
}
