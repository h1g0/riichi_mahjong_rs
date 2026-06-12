//! オンライン対戦のメニュー・ロビー画面
//!
//! 描画と入力処理。ネットワーク操作そのものは行わず、
//! メインループへアクションを返す。

use macroquad::prelude::*;

use super::draw_jp_text;
use crate::game::GameState;

/// ボタン・入力欄の矩形
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
}

/// 表示名の最大文字数
const NAME_MAX_CHARS: usize = 12;
/// ルームコードの文字数
const CODE_MAX_CHARS: usize = 6;

// レイアウト定数（ウィンドウ 1280x800）
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

/// オンラインメニューでの操作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineMenuAction {
    /// ルームを作成する
    CreateRoom,
    /// ルームコードで参加する
    JoinRoom,
    /// 設定画面に戻る
    Back,
}

/// ロビーでの操作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineLobbyAction {
    /// 対局を開始する（ホストのみ）
    StartGame,
    /// 退出する
    Leave,
}

fn draw_button(font: Option<&Font>, rect: &Rect2, label: &str, accent: bool) {
    let bg = if accent {
        Color::new(0.6, 0.15, 0.15, 1.0)
    } else {
        Color::new(0.25, 0.25, 0.25, 1.0)
    };
    let border = if accent {
        Color::new(0.9, 0.3, 0.3, 1.0)
    } else {
        Color::new(0.5, 0.5, 0.5, 1.0)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);
    // ボタン内でフォント(24px)を垂直中央に: y + (h+24)/2
    let text_w = label.chars().count() as f32 * 24.0;
    draw_jp_text(
        font,
        label,
        rect.x + (rect.w - text_w) / 2.0,
        rect.y + (rect.h + 24.0) / 2.0,
        24,
        WHITE,
    );
}

fn draw_input_box(font: Option<&Font>, rect: &Rect2, text: &str, focused: bool) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::new(0.1, 0.1, 0.1, 1.0),
    );
    let border = if focused {
        Color::new(1.0, 0.9, 0.3, 1.0)
    } else {
        Color::new(0.5, 0.5, 0.5, 1.0)
    };
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, border);
    // カーソル付きで内容を表示
    let shown = if focused {
        format!("{text}_")
    } else {
        text.to_string()
    };
    draw_jp_text(font, &shown, rect.x + 10.0, rect.y + 31.0, 24, WHITE);
}

fn draw_status_line(state: &GameState, font: Option<&Font>, y: f32) {
    if let Some(line) = &state.online_state.status_line {
        let color = if state.online_state.status_is_error {
            Color::new(1.0, 0.4, 0.4, 1.0)
        } else {
            Color::new(0.8, 0.8, 0.8, 1.0)
        };
        draw_jp_text(font, line, 440.0, y, 22, color);
    }
}

/// オンラインメニュー画面を描画する
pub fn draw_online_menu(state: &GameState, font: Option<&Font>) {
    let online = &state.online_state;

    draw_rectangle(190.0, 80.0, 900.0, 640.0, Color::new(0.0, 0.0, 0.0, 0.85));
    draw_rectangle_lines(
        190.0,
        80.0,
        900.0,
        640.0,
        2.0,
        Color::new(0.5, 0.5, 0.5, 1.0),
    );

    draw_jp_text(font, "オンライン対戦", 500.0, 140.0, 36, WHITE);

    draw_jp_text(
        font,
        "名前:",
        NAME_BOX.x,
        NAME_BOX.y - 10.0,
        22,
        Color::new(0.8, 0.8, 0.8, 1.0),
    );
    draw_input_box(font, &NAME_BOX, &online.name_input, !online.code_focused);

    draw_jp_text(
        font,
        "ルームコード（参加する場合）:",
        CODE_BOX.x,
        CODE_BOX.y - 10.0,
        22,
        Color::new(0.8, 0.8, 0.8, 1.0),
    );
    draw_input_box(font, &CODE_BOX, &online.code_input, online.code_focused);

    draw_button(font, &CREATE_BTN, "ルームを作成", true);
    draw_button(font, &JOIN_BTN, "ルームに参加", true);
    draw_button(font, &BACK_BTN, "戻る", false);

    draw_status_line(state, font, BACK_BTN.y + BACK_BTN.h + 40.0);
}

/// オンラインメニューの入力を処理する
pub fn handle_online_menu_input(state: &mut GameState) -> Option<OnlineMenuAction> {
    let online = &mut state.online_state;

    // 文字入力（フォーカス中の欄へ）
    while let Some(c) = get_char_pressed() {
        if c.is_control() {
            continue;
        }
        if online.code_focused {
            let c = c.to_ascii_uppercase();
            // ルームコードの文字種（紛らわしい 0/O/1/I は無い）のみ受け付ける
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
    let (mx, my) = mouse_position();

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

/// ロビー画面を描画する
pub fn draw_online_lobby(state: &GameState, font: Option<&Font>) {
    let online = &state.online_state;

    draw_rectangle(190.0, 80.0, 900.0, 640.0, Color::new(0.0, 0.0, 0.0, 0.85));
    draw_rectangle_lines(
        190.0,
        80.0,
        900.0,
        640.0,
        2.0,
        Color::new(0.5, 0.5, 0.5, 1.0),
    );

    draw_jp_text(font, "ロビー", 580.0, 140.0, 36, WHITE);

    let Some(room) = &online.room else {
        draw_jp_text(font, "ルーム情報を取得中...", 500.0, 300.0, 26, WHITE);
        draw_status_line(state, font, 380.0);
        return;
    };

    // ルームコード（友人に共有する）
    draw_jp_text(
        font,
        &format!("ルームコード: {}", room.code),
        440.0,
        220.0,
        32,
        Color::new(1.0, 0.9, 0.3, 1.0),
    );
    draw_jp_text(
        font,
        "このコードを友人に伝えて参加してもらいましょう",
        440.0,
        252.0,
        18,
        Color::new(0.7, 0.7, 0.7, 1.0),
    );

    // 座席一覧
    for (i, label) in room.seat_labels.iter().enumerate() {
        let y = 310.0 + i as f32 * 50.0;
        draw_rectangle(
            440.0,
            y - 28.0,
            400.0,
            40.0,
            Color::new(0.15, 0.15, 0.15, 1.0),
        );
        draw_jp_text(font, label, 452.0, y, 24, WHITE);
    }

    if room.is_host {
        draw_button(font, &START_BTN, "対局開始", true);
        draw_jp_text(
            font,
            "空席はCPUが入ります",
            START_BTN.x + 110.0,
            START_BTN.y - 8.0,
            18,
            Color::new(0.7, 0.7, 0.7, 1.0),
        );
    } else {
        draw_jp_text(
            font,
            "ホストの開始を待っています...",
            480.0,
            START_BTN.y + 34.0,
            24,
            Color::new(0.8, 0.8, 0.8, 1.0),
        );
    }
    draw_button(font, &LEAVE_BTN, "退出", false);

    draw_status_line(state, font, LEAVE_BTN.y + LEAVE_BTN.h + 36.0);
}

/// ロビーの入力を処理する
pub fn handle_online_lobby_input(state: &GameState) -> Option<OnlineLobbyAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let (mx, my) = mouse_position();

    let is_host = state
        .online_state
        .room
        .as_ref()
        .is_some_and(|room| room.is_host);
    if is_host && START_BTN.contains(mx, my) {
        return Some(OnlineLobbyAction::StartGame);
    }
    if LEAVE_BTN.contains(mx, my) {
        return Some(OnlineLobbyAction::Leave);
    }

    None
}

/// 対局中の接続状態バナーを描画する
pub fn draw_connection_banner(state: &GameState, font: Option<&Font>) {
    if let Some(line) = &state.online_state.status_line {
        let w = 420.0;
        let x = (1280.0 - w) / 2.0;
        draw_rectangle(x, 4.0, w, 32.0, Color::new(0.5, 0.1, 0.1, 0.9));
        draw_jp_text(font, line, x + 16.0, 27.0, 20, WHITE);
    }
}
