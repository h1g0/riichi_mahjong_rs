//! オンライン対戦のメニュー・ロビー画面
//!
//! 描画と入力処理。ネットワーク操作そのものは行わず、
//! メインループへアクションを返す。

use macroquad::prelude::*;

use super::{DESIGN_W, draw_jp_text, theme};
use crate::game::GameState;

/// パネルのレイアウト（設定画面と揃える）
const PANEL_X: f32 = 150.0;
const PANEL_Y: f32 = 50.0;
const PANEL_W: f32 = 980.0;
const PANEL_H: f32 = 690.0;

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

    fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
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

/// オンライン画面共通のパネルを描画する（タイトル＋英字サブタイトル）。
fn draw_online_panel(font: Option<&Font>, title: &str, subtitle: &str) {
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
    theme::draw_text_centered(font, title, cx, PANEL_Y + 54.0, 26, theme::TEXT_BR);
    theme::draw_text_centered(font, subtitle, cx, PANEL_Y + 74.0, 12, theme::TEXT_DIM);
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
    // カーソル付きで内容を表示
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

/// オンラインメニュー画面を描画する
pub fn draw_online_menu(state: &GameState, font: Option<&Font>) {
    let online = &state.online_state;

    draw_online_panel(font, "オンライン対戦", "Online Match");

    draw_jp_text(
        font,
        "名前 · Name",
        NAME_BOX.x,
        NAME_BOX.y - 9.0,
        11,
        theme::TEXT_DIM,
    );
    draw_input_box(font, &NAME_BOX, &online.name_input, !online.code_focused);

    draw_jp_text(
        font,
        "ルームコード（参加する場合） · Room Code",
        CODE_BOX.x,
        CODE_BOX.y - 9.0,
        11,
        theme::TEXT_DIM,
    );
    draw_input_box(font, &CODE_BOX, &online.code_input, online.code_focused);

    draw_button(font, &CREATE_BTN, "ルームを作成", true);
    draw_button(font, &JOIN_BTN, "ルームに参加", true);
    draw_button(font, &BACK_BTN, "戻る", false);

    draw_status_line(state, font, BACK_BTN.y + BACK_BTN.h + 30.0);
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

/// ロビー画面を描画する
pub fn draw_online_lobby(state: &GameState, font: Option<&Font>) {
    let online = &state.online_state;
    let cx = DESIGN_W / 2.0;

    draw_online_panel(font, "ロビー", "Lobby");

    let Some(room) = &online.room else {
        theme::draw_text_centered(font, "ルーム情報を取得中...", cx, 300.0, 18, theme::TEXT);
        draw_status_line(state, font, 340.0);
        return;
    };

    // ルームコード（友人に共有する）
    theme::draw_text_centered(
        font,
        &format!("ルームコード  {}", room.code),
        cx,
        210.0,
        28,
        theme::GOLD_LT,
    );
    theme::draw_text_centered(
        font,
        "このコードを参加プレイヤーに共有してください",
        cx,
        236.0,
        12,
        theme::TEXT_DIM,
    );

    // 座席一覧
    let row_x = 440.0;
    let row_w = 400.0;
    for (i, label) in room.seat_labels.iter().enumerate() {
        let y = 282.0 + i as f32 * 46.0;
        let is_me = label.contains("（あなた）");
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
        draw_button(font, &START_BTN, "対局開始", true);
        theme::draw_text_centered(
            font,
            "空席はCPUが入ります",
            cx,
            START_BTN.y - 8.0,
            12,
            theme::TEXT_DIM,
        );
    } else {
        theme::draw_text_centered(
            font,
            "ホストの開始を待っています...",
            cx,
            START_BTN.y + 34.0,
            16,
            theme::TEXT_DIM,
        );
    }
    draw_button(font, &LEAVE_BTN, "退出", false);

    draw_status_line(state, font, LEAVE_BTN.y + LEAVE_BTN.h + 28.0);
}

/// ロビーの入力を処理する
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
        let w = 440.0;
        let x = (DESIGN_W - w) / 2.0;
        // 上部バーの直下に角丸の赤いバナーを表示する
        theme::draw_rounded_rect(x, 56.0, w, 30.0, 6.0, theme::rgba(0x7a1010, 0.92));
        theme::draw_rounded_rect_lines(x, 56.0, w, 30.0, 6.0, 1.0, theme::RED);
        theme::draw_text_centered(font, line, DESIGN_W / 2.0, 76.0, 13, WHITE);
    }
}

/// 自分の手番の制限時間カウントダウンを描画する
///
/// オンラインで自分が操作待ちのときだけ表示する。
pub fn draw_turn_timer(state: &GameState, font: Option<&Font>) {
    let Some(remaining) = state.online_state.turn_remaining else {
        return;
    };
    // 自分が操作できる場面のみ表示する
    let my_turn = state.is_my_turn || !state.available_calls.is_empty();
    if !my_turn {
        return;
    }

    // 残り10秒以下は赤、それ以外はゴールド
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
        &format!("残り {remaining} 秒"),
        x + w / 2.0,
        y + h / 2.0 + 6.0,
        16,
        accent,
    );
}
