//! Title, mode-selection, and rule-settings screens.
//!
//! The title screen is the entry point: local CPU play, online play,
//! settings (not yet implemented), and language. Pre-game screens choose
//! the game mode and rules shared by CPU play and online room creation.

use macroquad::prelude::*;

use super::{DESIGN_W, draw_jp_text, mouse_position_design, theme};
use crate::game::{GameMode, GameState, MenuOrigin, RuleOption, RulePage};
use crate::i18n::Key;
use mahjong_core::settings::{AllLastRule, BankruptcyRule, Lang, Settings};

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
    const W: f32 = 230.0;
    const H: f32 = 42.0;
    const GAP_X: f32 = 12.0;
    const GAP_Y: f32 = 10.0;
    let left = DESIGN_W / 2.0 - W - GAP_X / 2.0;
    Rect2 {
        x: left + (idx % 2) as f32 * (W + GAP_X),
        y: 210.0 + (idx / 2) as f32 * (H + GAP_Y),
        w: W,
        h: H,
    }
}

fn mode_summary_rect() -> Rect2 {
    Rect2 {
        x: panel_x() + 32.0,
        y: 318.0,
        w: PANEL_W - 64.0,
        h: 236.0,
    }
}

fn mode_rule_settings_rect() -> Rect2 {
    menu_button(563.0, 38.0)
}

/// Back-button rectangle.
fn mode_back_rect() -> Rect2 {
    Rect2 {
        x: DESIGN_W / 2.0 - 180.0,
        y: 611.0,
        w: 174.0,
        h: 42.0,
    }
}

fn mode_confirm_rect() -> Rect2 {
    Rect2 {
        x: DESIGN_W / 2.0 + 6.0,
        y: 611.0,
        w: 174.0,
        h: 42.0,
    }
}

/// Mode-screen actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeSelectAction {
    /// Confirm the selected mode and continue
    ModeChosen(GameMode),
    /// Open the rule-settings screen
    OpenRuleSettings,
    /// Back to the previous screen
    Back,
}

fn selected_mode(state: &GameState, origin: MenuOrigin) -> GameMode {
    match origin {
        MenuOrigin::Local => state.setup_state.mode,
        MenuOrigin::Online => state.online_state.mode,
    }
}

fn set_selected_mode(state: &mut GameState, origin: MenuOrigin, mode: GameMode) {
    match origin {
        MenuOrigin::Local => state.setup_state.mode = mode,
        MenuOrigin::Online => state.online_state.mode = mode,
    }
}

fn selected_rules(state: &GameState, origin: MenuOrigin) -> &Settings {
    match origin {
        MenuOrigin::Local => &state.setup_state.rules,
        MenuOrigin::Online => &state.online_state.rules,
    }
}

fn selected_rules_mut(state: &mut GameState, origin: MenuOrigin) -> &mut Settings {
    match origin {
        MenuOrigin::Local => &mut state.setup_state.rules,
        MenuOrigin::Online => &mut state.online_state.rules,
    }
}

fn rule_label_key(rule: RuleOption) -> Key {
    match rule {
        RuleOption::RedFives => Key::RuleRedFives,
        RuleOption::DealerContinuation => Key::RuleDealerContinuation,
        RuleOption::RoundExtension => Key::RuleRoundExtension,
        RuleOption::AllLast => Key::RuleAllLast,
        RuleOption::Bankruptcy => Key::RuleBankruptcy,
        RuleOption::ColdEnd => Key::RuleColdEnd,
        RuleOption::RonMode => Key::RuleRonMode,
        RuleOption::AbortiveDrawMode => Key::RuleAbortiveDrawMode,
        RuleOption::OpenAllInside => Key::RuleOpenAllInside,
        RuleOption::SwapCalling => Key::RuleSwapCalling,
        RuleOption::NagashiMangan => Key::RuleNagashiMangan,
        RuleOption::KiriageMangan => Key::RuleKiriageMangan,
        RuleOption::CountedYakuman => Key::RuleCountedYakuman,
        RuleOption::DoubleYakuman => Key::RuleDoubleYakuman,
        RuleOption::NukiDora => Key::RuleNukiDora,
        RuleOption::TsumoLoss => Key::RuleTsumoLoss,
        RuleOption::FourKansDraw => Key::RuleFourKansDraw,
        RuleOption::FourWindsDraw => Key::RuleFourWindsDraw,
        RuleOption::FourRiichiDraw => Key::RuleFourRiichiDraw,
        RuleOption::NineTerminalsDraw => Key::RuleNineTerminalsDraw,
        RuleOption::YakumanPao => Key::RuleYakumanPao,
    }
}

fn rule_description_key(rule: RuleOption) -> Key {
    match rule {
        RuleOption::RedFives => Key::RuleRedFivesDescription,
        RuleOption::DealerContinuation => Key::RuleDealerContinuationDescription,
        RuleOption::RoundExtension => Key::RuleRoundExtensionDescription,
        RuleOption::AllLast => Key::RuleAllLastDescription,
        RuleOption::Bankruptcy => Key::RuleBankruptcyDescription,
        RuleOption::ColdEnd => Key::RuleColdEndDescription,
        RuleOption::RonMode => Key::RuleRonModeDescription,
        RuleOption::AbortiveDrawMode => Key::RuleAbortiveDrawModeDescription,
        RuleOption::OpenAllInside => Key::RuleOpenAllInsideDescription,
        RuleOption::SwapCalling => Key::RuleSwapCallingDescription,
        RuleOption::NagashiMangan => Key::RuleNagashiManganDescription,
        RuleOption::KiriageMangan => Key::RuleKiriageManganDescription,
        RuleOption::CountedYakuman => Key::RuleCountedYakumanDescription,
        RuleOption::DoubleYakuman => Key::RuleDoubleYakumanDescription,
        RuleOption::NukiDora => Key::RuleNukiDoraDescription,
        RuleOption::TsumoLoss => Key::RuleTsumoLossDescription,
        RuleOption::FourKansDraw => Key::RuleFourKansDrawDescription,
        RuleOption::FourWindsDraw => Key::RuleFourWindsDrawDescription,
        RuleOption::FourRiichiDraw => Key::RuleFourRiichiDrawDescription,
        RuleOption::NineTerminalsDraw => Key::RuleNineTerminalsDrawDescription,
        RuleOption::YakumanPao => Key::RuleYakumanPaoDescription,
    }
}

fn rule_value_text(state: &GameState, rule: RuleOption, rules: &Settings) -> String {
    let boolean = |enabled| {
        state
            .tr()
            .get(if enabled { Key::RuleOn } else { Key::RuleOff })
            .to_string()
    };
    match rule {
        RuleOption::RedFives => boolean(rules.red_fives),
        RuleOption::DealerContinuation => match (state.lang, rules.tenpai_renchan) {
            (Lang::Ja, true) => "和了・聴牌".into(),
            (Lang::Ja, false) => "和了のみ".into(),
            (Lang::En, true) => "Win or tenpai".into(),
            (Lang::En, false) => "Win only".into(),
        },
        RuleOption::RoundExtension => boolean(rules.round_extension),
        RuleOption::AllLast => match (state.lang, rules.all_last_rule) {
            (Lang::Ja, AllLastRule::Continue) => "なし".into(),
            (Lang::Ja, AllLastRule::Win) => "和了".into(),
            (Lang::Ja, AllLastRule::WinOrTenpai) => "和了・聴牌".into(),
            (Lang::En, AllLastRule::Continue) => "Off".into(),
            (Lang::En, AllLastRule::Win) => "Win".into(),
            (Lang::En, AllLastRule::WinOrTenpai) => "Win or tenpai".into(),
        },
        RuleOption::Bankruptcy => match (state.lang, rules.bankruptcy_rule) {
            (Lang::Ja, BankruptcyRule::None) => "なし".into(),
            (Lang::Ja, BankruptcyRule::Negative) => "マイナス".into(),
            (Lang::Ja, BankruptcyRule::ZeroOrLess) => "0点以下".into(),
            (Lang::En, BankruptcyRule::None) => "Off".into(),
            (Lang::En, BankruptcyRule::Negative) => "Below zero".into(),
            (Lang::En, BankruptcyRule::ZeroOrLess) => "Zero or less".into(),
        },
        RuleOption::ColdEnd => boolean(rules.cold_end),
        RuleOption::RonMode => match (state.lang, rules.multiple_ron, rules.triple_ron_draw) {
            (Lang::Ja, false, _) => "頭ハネ".into(),
            (Lang::Ja, true, false) => "ダブロン・トリロン".into(),
            (Lang::Ja, true, true) => "三家和流局".into(),
            (Lang::En, false, _) => "Head bump".into(),
            (Lang::En, true, false) => "Multiple ron".into(),
            (Lang::En, true, true) => "Triple-ron draw".into(),
        },
        RuleOption::AbortiveDrawMode => {
            let standard = rules.four_kans_draw
                && rules.four_winds_draw
                && !rules.four_riichi_draw
                && rules.nine_terminals_draw;
            let all = rules.four_kans_draw
                && rules.four_winds_draw
                && rules.four_riichi_draw
                && rules.nine_terminals_draw;
            let none = !rules.four_kans_draw
                && !rules.four_winds_draw
                && !rules.four_riichi_draw
                && !rules.nine_terminals_draw;
            match (state.lang, none, standard, all) {
                (Lang::Ja, true, _, _) => "なし".into(),
                (Lang::Ja, _, true, _) => "四槓・四風・九種".into(),
                (Lang::Ja, _, _, true) => "4種すべて".into(),
                (Lang::Ja, _, _, _) => "個別設定".into(),
                (Lang::En, true, _, _) => "Off".into(),
                (Lang::En, _, true, _) => "Except four riichi".into(),
                (Lang::En, _, _, true) => "All four".into(),
                (Lang::En, _, _, _) => "Custom".into(),
            }
        }
        RuleOption::OpenAllInside => boolean(rules.opened_all_inside),
        RuleOption::SwapCalling => boolean(!rules.forbid_swap_calling),
        RuleOption::NagashiMangan => boolean(rules.nagashi_mangan),
        RuleOption::KiriageMangan => boolean(rules.kiriage_mangan),
        RuleOption::CountedYakuman => boolean(rules.counted_yakuman),
        RuleOption::DoubleYakuman => boolean(rules.double_yakuman),
        RuleOption::NukiDora => boolean(rules.nuki_dora),
        RuleOption::TsumoLoss => boolean(rules.tsumo_loss),
        RuleOption::FourKansDraw => boolean(rules.four_kans_draw),
        RuleOption::FourWindsDraw => boolean(rules.four_winds_draw),
        RuleOption::FourRiichiDraw => boolean(rules.four_riichi_draw),
        RuleOption::NineTerminalsDraw => boolean(rules.nine_terminals_draw),
        RuleOption::YakumanPao => match (state.lang, rules.yakuman_pao, rules.four_quads_pao) {
            (Lang::Ja, false, _) => "なし".into(),
            (Lang::Ja, true, false) => "大三元・大四喜".into(),
            (Lang::Ja, true, true) => "大三元・大四喜・四槓子".into(),
            (Lang::En, false, _) => "Off".into(),
            (Lang::En, true, false) => "Dragons / Winds".into(),
            (Lang::En, true, true) => "Dragons / Winds / Four Quads".into(),
        },
    }
}

fn rule_summary_value(state: &GameState, rule: RuleOption, rules: &Settings) -> String {
    let label = state.tr().get(rule_label_key(rule));
    let value = rule_value_text(state, rule, rules);
    match state.lang {
        Lang::Ja
            if matches!(
                rule,
                RuleOption::RedFives
                    | RuleOption::RoundExtension
                    | RuleOption::ColdEnd
                    | RuleOption::OpenAllInside
                    | RuleOption::SwapCalling
                    | RuleOption::NagashiMangan
                    | RuleOption::KiriageMangan
                    | RuleOption::CountedYakuman
                    | RuleOption::DoubleYakuman
                    | RuleOption::NukiDora
                    | RuleOption::TsumoLoss
                    | RuleOption::FourKansDraw
                    | RuleOption::FourWindsDraw
                    | RuleOption::FourRiichiDraw
                    | RuleOption::NineTerminalsDraw
            ) =>
        {
            format!("{label}{value}")
        }
        Lang::Ja => format!("{label}：{value}"),
        Lang::En => format!("{label}: {value}"),
    }
}

fn initial_score_label(lang: Lang, three_player: bool) -> &'static str {
    match (lang, three_player) {
        (Lang::Ja, false) => "25,000点始まり",
        (Lang::Ja, true) => "35,000点始まり",
        (Lang::En, false) => "25,000-point start",
        (Lang::En, true) => "35,000-point start",
    }
}

fn return_score_label(lang: Lang, three_player: bool) -> &'static str {
    match (lang, three_player) {
        (Lang::Ja, false) => "30,000点返し",
        (Lang::Ja, true) => "40,000点返し",
        (Lang::En, false) => "30,000-point return",
        (Lang::En, true) => "40,000-point return",
    }
}

fn mode_summary_lines(state: &GameState, origin: MenuOrigin) -> Vec<String> {
    let tr = state.tr();
    let mode = selected_mode(state, origin);
    let rules = selected_rules(state, origin);
    let separator = match state.lang {
        Lang::Ja => "、",
        Lang::En => " / ",
    };
    let extension = match (state.lang, mode.length(), rules.round_extension) {
        (Lang::Ja, mahjong_server::table::GameLength::EastOnly, true) => "南入あり",
        (Lang::Ja, mahjong_server::table::GameLength::EastOnly, false) => "南入なし",
        (Lang::Ja, mahjong_server::table::GameLength::Hanchan, true) => "西入あり",
        (Lang::Ja, mahjong_server::table::GameLength::Hanchan, false) => "西入なし",
        (Lang::En, mahjong_server::table::GameLength::EastOnly, true) => "South extension: On",
        (Lang::En, mahjong_server::table::GameLength::EastOnly, false) => "South extension: Off",
        (Lang::En, mahjong_server::table::GameLength::Hanchan, true) => "West extension: On",
        (Lang::En, mahjong_server::table::GameLength::Hanchan, false) => "West extension: Off",
    };
    let extension = if rules.round_extension {
        let return_score = return_score_label(state.lang, mode.three_player());
        match state.lang {
            Lang::Ja => format!("{extension}（{return_score}）"),
            Lang::En => format!("{extension} ({return_score})"),
        }
    } else {
        extension.into()
    };
    let join = |items: &[RuleOption]| {
        items
            .iter()
            .map(|rule| rule_summary_value(state, *rule, rules))
            .collect::<Vec<_>>()
            .join(separator)
    };
    let mut lines = vec![format!(
        "{}{}{}{}{}",
        tr.get(mode.label_key()),
        separator,
        initial_score_label(state.lang, mode.three_player()),
        separator,
        extension
    )];
    lines.push(join(&[
        RuleOption::RedFives,
        RuleOption::DealerContinuation,
        RuleOption::RonMode,
    ]));
    lines.push(join(&[
        RuleOption::Bankruptcy,
        RuleOption::AllLast,
        RuleOption::NagashiMangan,
    ]));
    let mut final_rules = vec![RuleOption::KiriageMangan, RuleOption::CountedYakuman];
    if mode.three_player() {
        final_rules.push(RuleOption::NukiDora);
        final_rules.push(RuleOption::TsumoLoss);
    } else {
        final_rules.push(RuleOption::ColdEnd);
    }
    lines.push(join(&final_rules));
    lines
}

/// Draws the mode-selection screen.
pub fn draw_mode_select(state: &GameState, font: Option<&Font>, origin: MenuOrigin) {
    let tr = state.tr();
    draw_menu_panel(font, tr.get(Key::ModeSelectTitle), 26);

    let current_mode = selected_mode(state, origin);

    for (idx, mode) in GameMode::ALL.into_iter().enumerate() {
        draw_toggle(
            font,
            &mode_rect(idx),
            tr.get(mode.label_key()),
            mode == current_mode,
            16,
        );
    }

    let summary = mode_summary_rect();
    theme::draw_panel(
        summary.x,
        summary.y,
        summary.w,
        summary.h,
        6.0,
        theme::rgba(0xffffff, 0.025),
        theme::rgba(0xc8a227, 0.16),
    );
    theme::draw_text_centered(
        font,
        tr.get(Key::CurrentRulesTitle),
        summary.center_x(),
        summary.y + 22.0,
        13,
        theme::GOLD,
    );
    for (idx, line) in mode_summary_lines(state, origin).iter().enumerate() {
        theme::draw_text_centered(
            font,
            line,
            summary.center_x(),
            summary.y + 44.0 + idx as f32 * 20.0,
            11,
            theme::TEXT_DIM,
        );
    }

    draw_menu_button(
        font,
        &mode_rule_settings_rect(),
        tr.get(Key::RuleSettingsButton),
        false,
    );
    draw_menu_button(font, &mode_back_rect(), tr.get(Key::Back), false);
    draw_menu_button(font, &mode_confirm_rect(), tr.get(Key::Confirm), true);
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
            set_selected_mode(state, origin, mode);
            return None;
        }
    }

    if mode_rule_settings_rect().contains(mx, my) {
        let three_player = selected_mode(state, origin).three_player();
        let options = RuleOption::options(state.rule_page, three_player);
        if !options.contains(&state.selected_rule) {
            state.selected_rule = options[0];
        }
        return Some(ModeSelectAction::OpenRuleSettings);
    }

    if mode_confirm_rect().contains(mx, my) {
        return Some(ModeSelectAction::ModeChosen(selected_mode(state, origin)));
    }

    if mode_back_rect().contains(mx, my) {
        return Some(ModeSelectAction::Back);
    }

    None
}

// ========== Rule-settings screen ==========

const RULE_SETTINGS_TITLE_Y: f32 = PANEL_Y + 45.0;

fn draw_rule_settings_panel(font: Option<&Font>, title: &str) {
    draw_panel_background();
    theme::draw_text_centered(
        font,
        title,
        DESIGN_W / 2.0,
        RULE_SETTINGS_TITLE_Y,
        26,
        theme::TEXT_BR,
    );
}

fn rule_page_rect(page: RulePage) -> Rect2 {
    let width = (PANEL_W - 76.0) / 2.0;
    Rect2 {
        x: panel_x()
            + 32.0
            + if page == RulePage::Advanced {
                width + 12.0
            } else {
                0.0
            },
        y: 170.0,
        w: width,
        h: 34.0,
    }
}

fn rule_rect(idx: usize) -> Rect2 {
    const W: f32 = 242.0;
    const H: f32 = 35.0;
    const GAP_X: f32 = 12.0;
    const GAP_Y: f32 = 8.0;
    let left = panel_x() + 32.0;
    Rect2 {
        x: left + (idx % 2) as f32 * (W + GAP_X),
        y: 216.0 + (idx / 2) as f32 * (H + GAP_Y),
        w: W,
        h: H,
    }
}

fn rule_description_rect() -> Rect2 {
    Rect2 {
        x: panel_x() + 32.0,
        y: 480.0,
        w: PANEL_W - 64.0,
        h: 112.0,
    }
}

const RULE_VALUE_ARROW_WIDTH: f32 = 46.0;
const RULE_VALUE_SELECTOR_MIN_WIDTH: f32 = 200.0;
const RULE_VALUE_TEXT_PADDING: f32 = 24.0;

fn rule_value_choices(lang: Lang, rule: RuleOption) -> &'static [&'static str] {
    match (lang, rule) {
        (Lang::Ja, RuleOption::DealerContinuation) => &["和了のみ", "和了・聴牌"],
        (Lang::En, RuleOption::DealerContinuation) => &["Win only", "Win or tenpai"],
        (Lang::Ja, RuleOption::AllLast) => &["なし", "和了", "和了・聴牌"],
        (Lang::En, RuleOption::AllLast) => &["Off", "Win", "Win or tenpai"],
        (Lang::Ja, RuleOption::Bankruptcy) => &["なし", "マイナス", "0点以下"],
        (Lang::En, RuleOption::Bankruptcy) => &["Off", "Below zero", "Zero or less"],
        (Lang::Ja, RuleOption::RonMode) => &["頭ハネ", "ダブロン・トリロン", "三家和流局"],
        (Lang::En, RuleOption::RonMode) => &["Head bump", "Multiple ron", "Triple-ron draw"],
        (Lang::Ja, RuleOption::AbortiveDrawMode) => {
            &["なし", "四槓・四風・九種", "4種すべて", "個別設定"]
        }
        (Lang::En, RuleOption::AbortiveDrawMode) => {
            &["Off", "Except four riichi", "All four", "Custom"]
        }
        (Lang::Ja, RuleOption::YakumanPao) => &["なし", "大三元・大四喜", "大三元・大四喜・四槓子"],
        (Lang::En, RuleOption::YakumanPao) => {
            &["Off", "Dragons / Winds", "Dragons / Winds / Four Quads"]
        }
        (Lang::Ja, _) => &["なし", "あり"],
        (Lang::En, _) => &["Off", "On"],
    }
}

fn rule_value_selector_width(lang: Lang, rule: RuleOption) -> f32 {
    let max_chars = rule_value_choices(lang, rule)
        .iter()
        .map(|value| value.chars().count())
        .max()
        .unwrap_or(0) as f32;
    let estimated_char_width = if lang == Lang::Ja { 12.0 } else { 7.0 };
    let value_width = max_chars * estimated_char_width + RULE_VALUE_TEXT_PADDING;
    (RULE_VALUE_ARROW_WIDTH * 2.0 + value_width).max(RULE_VALUE_SELECTOR_MIN_WIDTH)
}

fn rule_value_selector_rect(state: &GameState, rule: RuleOption) -> Rect2 {
    let width = rule_value_selector_width(state.lang, rule);
    Rect2 {
        x: DESIGN_W / 2.0 - width / 2.0,
        y: 511.0,
        w: width,
        h: 30.0,
    }
}

fn rule_value_previous_rect(state: &GameState, rule: RuleOption) -> Rect2 {
    let selector = rule_value_selector_rect(state, rule);
    Rect2 {
        x: selector.x,
        y: selector.y,
        w: RULE_VALUE_ARROW_WIDTH,
        h: selector.h,
    }
}

fn rule_value_next_rect(state: &GameState, rule: RuleOption) -> Rect2 {
    let selector = rule_value_selector_rect(state, rule);
    Rect2 {
        x: selector.x + selector.w - RULE_VALUE_ARROW_WIDTH,
        y: selector.y,
        w: RULE_VALUE_ARROW_WIDTH,
        h: selector.h,
    }
}

fn rule_confirm_rect() -> Rect2 {
    menu_button(600.0, 46.0)
}

fn draw_rule_button(
    font: Option<&Font>,
    rect: &Rect2,
    label: &str,
    value: &str,
    selected: bool,
    enabled: bool,
) {
    let fill = if selected {
        theme::rgba(0xc8a227, 0.13)
    } else {
        theme::rgba(0xffffff, 0.04)
    };
    let border = if selected {
        theme::rgba(0xc8a227, 0.6)
    } else {
        theme::rgba(0xffffff, 0.08)
    };
    theme::draw_rounded_rect(rect.x, rect.y, rect.w, rect.h, 4.0, fill);
    theme::draw_rounded_rect_lines(rect.x, rect.y, rect.w, rect.h, 4.0, 1.0, border);
    draw_jp_text(
        font,
        label,
        rect.x + 12.0,
        rect.center_y() + 5.0,
        12,
        if selected {
            theme::GOLD_LT
        } else {
            theme::TEXT
        },
    );
    theme::draw_text_centered(
        font,
        value,
        rect.x + rect.w - 64.0,
        rect.center_y() + 5.0,
        9,
        if enabled {
            theme::GOLD_LT
        } else {
            theme::TEXT_DIM
        },
    );
}

fn draw_rule_value_selector(
    state: &GameState,
    font: Option<&Font>,
    rule: RuleOption,
    value: &str,
    enabled: bool,
) {
    let selector = rule_value_selector_rect(state, rule);
    let previous = rule_value_previous_rect(state, rule);
    let next = rule_value_next_rect(state, rule);
    theme::draw_rounded_rect(
        selector.x,
        selector.y,
        selector.w,
        selector.h,
        4.0,
        theme::rgba(0xffffff, 0.04),
    );
    theme::draw_rounded_rect(
        previous.x,
        previous.y,
        previous.w,
        previous.h,
        4.0,
        theme::rgba(0xc8a227, 0.1),
    );
    theme::draw_rounded_rect(
        next.x,
        next.y,
        next.w,
        next.h,
        4.0,
        theme::rgba(0xc8a227, 0.1),
    );
    theme::draw_rounded_rect_lines(
        selector.x,
        selector.y,
        selector.w,
        selector.h,
        4.0,
        1.0,
        theme::rgba(0xc8a227, 0.42),
    );
    draw_line(
        previous.x + previous.w,
        selector.y + 4.0,
        previous.x + previous.w,
        selector.y + selector.h - 4.0,
        1.0,
        theme::rgba(0xc8a227, 0.25),
    );
    draw_line(
        next.x,
        selector.y + 4.0,
        next.x,
        selector.y + selector.h - 4.0,
        1.0,
        theme::rgba(0xc8a227, 0.25),
    );
    theme::draw_text_centered(
        font,
        "←",
        previous.center_x(),
        previous.center_y() + 5.0,
        12,
        theme::GOLD_LT,
    );
    theme::draw_text_centered(
        font,
        value,
        selector.center_x(),
        selector.center_y() + 5.0,
        12,
        if enabled {
            theme::GOLD_LT
        } else {
            theme::TEXT_DIM
        },
    );
    theme::draw_text_centered(
        font,
        "→",
        next.center_x(),
        next.center_y() + 5.0,
        12,
        theme::GOLD_LT,
    );
}

fn select_rule(state: &mut GameState, rule: RuleOption) {
    state.selected_rule = rule;
}

fn cycle_selected_rule(state: &mut GameState, origin: MenuOrigin, forward: bool) {
    let rule = state.selected_rule;
    rule.cycle(selected_rules_mut(state, origin), forward);
}

fn select_rule_page(state: &mut GameState, page: RulePage, origin: MenuOrigin) {
    state.rule_page = page;
    let three_player = selected_mode(state, origin).three_player();
    state.selected_rule = RuleOption::options(page, three_player)[0];
}

/// Action emitted by the rule-settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSettingsAction {
    /// Keep the changes and return to mode selection
    Confirm,
}

/// Draws every configurable game rule and the selected rule's description.
pub fn draw_rule_settings(state: &GameState, font: Option<&Font>, origin: MenuOrigin) {
    let tr = state.tr();
    let rules = selected_rules(state, origin);
    draw_rule_settings_panel(font, tr.get(Key::RuleSettingsTitle));

    for (page, key) in [
        (RulePage::Basic, Key::RuleBasicPage),
        (RulePage::Advanced, Key::RuleAdvancedPage),
    ] {
        let rect = rule_page_rect(page);
        let selected = state.rule_page == page;
        theme::draw_rounded_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            4.0,
            if selected {
                theme::rgba(0xc8a227, 0.16)
            } else {
                theme::rgba(0xffffff, 0.035)
            },
        );
        theme::draw_rounded_rect_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            4.0,
            1.0,
            if selected {
                theme::rgba(0xc8a227, 0.55)
            } else {
                theme::rgba(0xffffff, 0.08)
            },
        );
        theme::draw_text_centered(
            font,
            tr.get(key),
            rect.center_x(),
            rect.center_y() + 5.0,
            12,
            if selected {
                theme::GOLD_LT
            } else {
                theme::TEXT_DIM
            },
        );
    }

    let options = RuleOption::options(state.rule_page, selected_mode(state, origin).three_player());
    for (idx, rule) in options.into_iter().enumerate() {
        let enabled = rule.is_enabled(rules);
        let value = rule_value_text(state, rule, rules);
        let label = tr.get(rule_label_key(rule));
        draw_rule_button(
            font,
            &rule_rect(idx),
            label,
            &value,
            rule == state.selected_rule,
            enabled,
        );
    }

    let description = rule_description_rect();
    theme::draw_panel(
        description.x,
        description.y,
        description.w,
        description.h,
        6.0,
        theme::rgba(0xffffff, 0.025),
        theme::rgba(0xc8a227, 0.16),
    );
    theme::draw_text_centered(
        font,
        tr.get(rule_label_key(state.selected_rule)),
        description.center_x(),
        description.y + 23.0,
        14,
        theme::GOLD,
    );
    let selected_enabled = state.selected_rule.is_enabled(rules);
    let selected_value = rule_value_text(state, state.selected_rule, rules);
    draw_rule_value_selector(
        state,
        font,
        state.selected_rule,
        &selected_value,
        selected_enabled,
    );
    let description_text = tr.get(rule_description_key(state.selected_rule));
    let description_lines: Vec<_> = description_text.lines().collect();
    let first_baseline = if description_lines.len() == 1 {
        description.y + 82.0
    } else {
        description.y + 73.0
    };
    for (idx, line) in description_lines.into_iter().enumerate() {
        theme::draw_text_centered(
            font,
            line,
            description.center_x(),
            first_baseline + idx as f32 * 18.0,
            11,
            theme::TEXT_DIM,
        );
    }

    draw_menu_button(font, &rule_confirm_rect(), tr.get(Key::Confirm), true);
}

/// Handles rule selection, value arrows, and settings confirmation.
pub fn handle_rule_settings_input(
    state: &mut GameState,
    origin: MenuOrigin,
) -> Option<RuleSettingsAction> {
    if !is_mouse_button_pressed(MouseButton::Left) {
        return None;
    }
    let (mx, my) = mouse_position_design();

    for page in [RulePage::Basic, RulePage::Advanced] {
        if rule_page_rect(page).contains(mx, my) {
            select_rule_page(state, page, origin);
            return None;
        }
    }

    let options = RuleOption::options(state.rule_page, selected_mode(state, origin).three_player());
    for (idx, rule) in options.into_iter().enumerate() {
        if rule_rect(idx).contains(mx, my) {
            select_rule(state, rule);
            return None;
        }
    }

    if rule_value_previous_rect(state, state.selected_rule).contains(mx, my) {
        cycle_selected_rule(state, origin, false);
        return None;
    }

    if rule_value_next_rect(state, state.selected_rule).contains(mx, my) {
        cycle_selected_rule(state, origin, true);
        return None;
    }

    if rule_confirm_rect().contains(mx, my) {
        return Some(RuleSettingsAction::Confirm);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_summary_reports_mode_score_extension_and_rules() {
        let mut state = GameState::new();
        state.lang = Lang::Ja;

        let lines = mode_summary_lines(&state, MenuOrigin::Local);

        assert_eq!(lines[0], "四人東風、25,000点始まり、南入なし");
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().any(|line| line.contains("赤ドラあり")));
        assert!(lines.iter().any(|line| line.contains("連荘：和了・聴牌")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("同時ロン：ダブロン・トリロン"))
        );
        assert!(lines.iter().any(|line| line.contains("飛び終了：マイナス")));
        assert!(lines.iter().any(|line| line.contains("流し満貫あり")));
        assert!(lines.iter().any(|line| line.contains("切り上げ満貫あり")));
        assert!(lines.iter().any(|line| line.contains("数え役満あり")));
        assert!(!lines.iter().any(|line| line.contains("喰い替え")));
        assert!(!lines.iter().any(|line| line.contains("北抜き")));
    }

    #[test]
    fn mode_summary_includes_three_player_score_and_pei_dora() {
        let mut state = GameState::new();
        state.lang = Lang::Ja;
        state.setup_state.mode = GameMode::ThreeHanchan;
        state.setup_state.rules.nuki_dora = false;
        state.setup_state.rules.tsumo_loss = false;

        let lines = mode_summary_lines(&state, MenuOrigin::Local);

        assert_eq!(lines[0], "三人半荘、35,000点始まり、西入なし");
        assert!(lines.iter().any(|line| line.contains("北抜きなし")));
        assert!(lines.iter().any(|line| line.contains("ツモ損なし")));
        assert!(lines.iter().any(|line| line.contains("赤ドラあり")));
        assert!(!lines.iter().any(|line| line.contains("コールド終了")));
    }

    #[test]
    fn mode_summary_reports_return_score_only_with_round_extension() {
        let mut state = GameState::new();
        state.lang = Lang::Ja;
        state.setup_state.rules.round_extension = true;

        let four_player_lines = mode_summary_lines(&state, MenuOrigin::Local);
        assert_eq!(
            four_player_lines[0],
            "四人東風、25,000点始まり、南入あり（30,000点返し）"
        );

        state.setup_state.mode = GameMode::ThreeHanchan;
        let three_player_lines = mode_summary_lines(&state, MenuOrigin::Local);
        assert_eq!(
            three_player_lines[0],
            "三人半荘、35,000点始まり、西入あり（40,000点返し）"
        );
    }

    #[test]
    fn double_yakuman_description_lists_every_qualifying_hand() {
        let description = Key::RuleDoubleYakumanDescription.text(Lang::Ja);

        for name in [
            "国士無双十三面待ち",
            "四暗刻単騎待ち",
            "大四喜",
            "純正九蓮宝燈",
        ] {
            assert!(description.contains(name), "missing {name}");
        }
        assert_eq!(description.lines().count(), 2);
    }

    #[test]
    fn tsumo_loss_description_explains_the_disabled_payment_split() {
        let ja = Key::RuleTsumoLossDescription.text(Lang::Ja);
        let en = Key::RuleTsumoLossDescription.text(Lang::En);

        assert!(ja.contains("なしの場合は支払う2人で折半"));
        assert!(en.contains("when off, both payers split it"));
        assert_eq!(ja.lines().count(), 2);
        assert_eq!(en.lines().count(), 2);
    }

    #[test]
    fn ron_mode_description_explains_all_three_choices_and_fits_panel() {
        let ja = Key::RuleRonModeDescription.text(Lang::Ja);
        let en = Key::RuleRonModeDescription.text(Lang::En);

        for phrase in ["頭ハネ", "ダブロン・トリロン", "三家和"] {
            assert!(ja.contains(phrase), "missing {phrase}");
        }
        assert!(en.contains("head bump"));
        assert!(en.contains("double/triple ron"));
        assert!(en.contains("abortive draw"));
        assert_eq!(ja.lines().count(), 2);
        assert_eq!(en.lines().count(), 2);

        let description = rule_description_rect();
        let last_baseline = description.y + 73.0 + 18.0;
        assert!(last_baseline <= description.y + description.h);
        assert!(description.y + description.h < rule_confirm_rect().y);
    }

    #[test]
    fn selecting_a_rule_does_not_toggle_it_until_an_arrow_is_used() {
        let mut state = GameState::new();
        let rule = RuleOption::SwapCalling;
        let before = rule.is_enabled(&state.setup_state.rules);

        select_rule(&mut state, rule);

        assert_eq!(state.selected_rule, rule);
        assert_eq!(rule.is_enabled(&state.setup_state.rules), before);

        cycle_selected_rule(&mut state, MenuOrigin::Local, true);

        assert_ne!(rule.is_enabled(&state.setup_state.rules), before);
    }

    #[test]
    fn rule_settings_title_stays_above_page_tabs() {
        let basic_tab = rule_page_rect(RulePage::Basic);

        assert!(RULE_SETTINGS_TITLE_Y + 8.0 < basic_tab.y);
    }

    #[test]
    fn rule_value_selector_expands_for_the_longest_choice() {
        let mut state = GameState::new();
        state.lang = Lang::Ja;

        let boolean_width = rule_value_selector_width(state.lang, RuleOption::RedFives);
        let ron_width = rule_value_selector_width(state.lang, RuleOption::RonMode);
        let pao_width = rule_value_selector_width(state.lang, RuleOption::YakumanPao);

        assert_eq!(boolean_width, RULE_VALUE_SELECTOR_MIN_WIDTH);
        assert!(ron_width > boolean_width);
        assert!(pao_width > ron_width);

        let pao_rect = rule_value_selector_rect(&state, RuleOption::YakumanPao);
        assert_eq!(pao_rect.center_x(), DESIGN_W / 2.0);
        assert_eq!(
            rule_value_previous_rect(&state, RuleOption::YakumanPao).w,
            RULE_VALUE_ARROW_WIDTH
        );
        assert_eq!(
            rule_value_next_rect(&state, RuleOption::YakumanPao).w,
            RULE_VALUE_ARROW_WIDTH
        );
    }
}
