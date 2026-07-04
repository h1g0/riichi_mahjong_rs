//! 設定画面の描画と入力処理

use super::*;

// ========== 設定画面 ==========

/// 設定画面のボタン領域
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

// 設定画面のレイアウト定数（描画と入力判定で共有する）
pub(super) const SETUP_PANEL_W: f32 = 980.0;
pub(super) const SETUP_PANEL_Y: f32 = 56.0;
pub(super) const SETUP_PANEL_H: f32 = 612.0;
pub(super) const SETUP_CARD_PAD: f32 = 40.0;
pub(super) const SETUP_CARD_GAP: f32 = 20.0;
pub(super) const SETUP_CARD_Y: f32 = 142.0;
pub(super) const SETUP_CARD_H: f32 = 348.0;
pub(super) const SETUP_OPT_H: f32 = 28.0;
pub(super) const SETUP_OPT_STEP: f32 = 32.0;

/// 言語切替トグルのボタン矩形（パネル右上）。idx 0=日本語, 1=English。
pub(super) fn setup_lang_button_rect(idx: usize) -> SetupButton {
    const W: f32 = 84.0;
    const H: f32 = 28.0;
    const GAP: f32 = 6.0;
    let right = setup_panel_x() + SETUP_PANEL_W - SETUP_CARD_PAD;
    let y = SETUP_PANEL_Y + 24.0;
    // 右詰めで [日本語][English] を並べる
    let x = right - (2.0 - idx as f32) * W - (1.0 - idx as f32) * GAP;
    SetupButton { x, y, w: W, h: H }
}

/// 言語切替トグルに表示する固有名（言語非依存の自称表記）。
pub(super) const SETUP_LANG_LABELS: [&str; 2] = ["日本語", "English"];

/// 対局モードトグルのボタン矩形（パネル左上）。
/// idx 0=四人東風, 1=四人半荘, 2=三人東風, 3=三人半荘。
pub(super) fn setup_mode_button_rect(idx: usize) -> SetupButton {
    const W: f32 = 96.0;
    const H: f32 = 28.0;
    const GAP: f32 = 6.0;
    let left = setup_panel_x() + SETUP_CARD_PAD;
    let y = SETUP_PANEL_Y + 24.0;
    SetupButton {
        x: left + idx as f32 * (W + GAP),
        y,
        w: W,
        h: H,
    }
}

/// 北抜きドラトグルのボタン矩形（三麻選択時のみ表示）。
pub(super) fn setup_nuki_button_rect() -> SetupButton {
    let m = setup_mode_button_rect(MODE_COUNT - 1);
    SetupButton {
        x: m.x + m.w + 18.0,
        y: m.y,
        w: 110.0,
        h: m.h,
    }
}

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

pub(super) fn setup_online_rect() -> SetupButton {
    let s = setup_start_rect();
    SetupButton {
        x: DESIGN_W / 2.0 - 110.0,
        y: s.y + s.h + 12.0,
        w: 220.0,
        h: 38.0,
    }
}

/// 設定画面のオプションボタンを 1 個描画する。
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

/// 設定画面を描画する
pub(super) fn draw_setup(state: &GameState, font: Option<&Font>) {
    draw_setup_background();
    let setup = &state.setup_state;
    let tr = state.tr();
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
        tr.get(Key::SetupTitle),
        cx,
        SETUP_PANEL_Y + 52.0,
        26,
        theme::TEXT_BR,
    );

    // 言語切替トグル（日本語 / English）
    let active_lang = match state.lang {
        Lang::Ja => 0,
        Lang::En => 1,
    };
    for (idx, &label) in SETUP_LANG_LABELS.iter().enumerate() {
        let btn = setup_lang_button_rect(idx);
        draw_setup_option(font, &btn, label, idx == active_lang);
    }

    // 対局モードトグル（四人東風 / 四人半荘 / 三人東風 / 三人半荘）
    let mode_labels = [
        Key::ModeFourEast,
        Key::ModeFourHanchan,
        Key::ModeThreeEast,
        Key::ModeThreeHanchan,
    ];
    for (idx, key) in mode_labels.into_iter().enumerate() {
        let btn = setup_mode_button_rect(idx);
        draw_setup_option(font, &btn, tr.get(key), idx == setup.mode_index());
    }
    // 北抜きドラトグル（三麻のみ）
    if setup.three_player {
        let btn = setup_nuki_button_rect();
        draw_setup_option(font, &btn, tr.get(Key::NukiDoraToggle), setup.nuki_dora);
    }

    // CPU カード（三麻はCPU2人）
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

        // ヘッダー：番号リング＋名前（席順は対局開始時にランダムで決まるため、
        // 風・相対位置ではなく CPU 番号で表示する）
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

        // 強さ
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

        // 性格
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
    theme::draw_text_centered(
        font,
        tr.get(Key::StartGame),
        cx,
        s.y + 34.0,
        20,
        theme::GOLD_LT,
    );

    // オンライン対戦ボタン
    let o = setup_online_rect();
    theme::draw_rounded_rect(o.x, o.y, o.w, o.h, 6.0, theme::rgba(0xffffff, 0.05));
    theme::draw_rounded_rect_lines(o.x, o.y, o.w, o.h, 6.0, 1.0, theme::rgba(0xc8a227, 0.3));
    theme::draw_text_centered(
        font,
        tr.get(Key::OnlinePlay),
        cx,
        o.y + 24.0,
        14,
        theme::TEXT,
    );
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

    // 言語切替トグル（日本語 / English）
    for (idx, lang) in [Lang::Ja, Lang::En].into_iter().enumerate() {
        if setup_lang_button_rect(idx).contains(mx, my) {
            state.lang = lang;
            crate::persistence::save_lang(lang);
            return None;
        }
    }

    let setup = &mut state.setup_state;

    // 対局モードトグル（四人東風 / 四人半荘 / 三人東風 / 三人半荘）
    for idx in 0..MODE_COUNT {
        if setup_mode_button_rect(idx).contains(mx, my) {
            setup.set_mode_index(idx);
            return None;
        }
    }
    // 北抜きドラトグル（三麻のみ）
    if setup.three_player && setup_nuki_button_rect().contains(mx, my) {
        setup.nuki_dora = !setup.nuki_dora;
        return None;
    }

    for cpu_idx in 0..setup.cpu_count() {
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
