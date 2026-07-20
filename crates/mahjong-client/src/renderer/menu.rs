//! Title, mode-selection, and rule-settings screens.
//!
//! The title screen is the entry point: local CPU play, online play,
//! settings (not yet implemented), and language. Pre-game screens choose
//! the game mode and rules shared by CPU play and online room creation.

use macroquad::prelude::*;

use super::{DESIGN_W, draw_jp_text, mouse_position_design, theme};
use crate::game::{GameMode, GameState, MenuOrigin, RuleOption};
use crate::i18n::Key;
use mahjong_core::settings::{Lang, Settings};

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

    #[cfg(not(target_arch = "wasm32"))]
    theme::draw_text_centered(
        font,
        tr.get(Key::ScreenshotHint),
        DESIGN_W / 2.0,
        664.0,
        11,
        theme::TEXT_DIM,
    );
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
        RuleOption::OpenAllInside => Key::RuleOpenAllInside,
        RuleOption::SwapCalling => Key::RuleSwapCalling,
        RuleOption::DoubleYakuman => Key::RuleDoubleYakuman,
        RuleOption::NukiDora => Key::RuleNukiDora,
        RuleOption::TsumoLoss => Key::RuleTsumoLoss,
        RuleOption::FourKansDraw => Key::RuleFourKansDraw,
        RuleOption::FourWindsDraw => Key::RuleFourWindsDraw,
        RuleOption::FourRiichiDraw => Key::RuleFourRiichiDraw,
        RuleOption::NineTerminalsDraw => Key::RuleNineTerminalsDraw,
        RuleOption::TripleRonDraw => Key::RuleTripleRonDraw,
        RuleOption::MultipleRon => Key::RuleMultipleRon,
        RuleOption::YakumanPao => Key::RuleYakumanPao,
    }
}

fn rule_description_key(rule: RuleOption) -> Key {
    match rule {
        RuleOption::OpenAllInside => Key::RuleOpenAllInsideDescription,
        RuleOption::SwapCalling => Key::RuleSwapCallingDescription,
        RuleOption::DoubleYakuman => Key::RuleDoubleYakumanDescription,
        RuleOption::NukiDora => Key::RuleNukiDoraDescription,
        RuleOption::TsumoLoss => Key::RuleTsumoLossDescription,
        RuleOption::FourKansDraw => Key::RuleFourKansDrawDescription,
        RuleOption::FourWindsDraw => Key::RuleFourWindsDrawDescription,
        RuleOption::FourRiichiDraw => Key::RuleFourRiichiDrawDescription,
        RuleOption::NineTerminalsDraw => Key::RuleNineTerminalsDrawDescription,
        RuleOption::TripleRonDraw => Key::RuleTripleRonDrawDescription,
        RuleOption::MultipleRon => Key::RuleMultipleRonDescription,
        RuleOption::YakumanPao => Key::RuleYakumanPaoDescription,
    }
}

fn rule_value(state: &GameState, rule: RuleOption, enabled: bool) -> String {
    let tr = state.tr();
    let label = tr.get(rule_label_key(rule));
    let value = tr.get(if enabled { Key::RuleOn } else { Key::RuleOff });
    match state.lang {
        Lang::Ja => format!("{label}{value}"),
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

fn mode_summary_lines(state: &GameState, origin: MenuOrigin) -> Vec<String> {
    let tr = state.tr();
    let mode = selected_mode(state, origin);
    let rules = selected_rules(state, origin);
    let separator = match state.lang {
        Lang::Ja => "、",
        Lang::En => " / ",
    };
    let extension = match mode.length() {
        mahjong_server::table::GameLength::EastOnly => tr.get(Key::NoSouthExtension),
        mahjong_server::table::GameLength::Hanchan => tr.get(Key::NoWestExtension),
    };
    let join = |items: &[(RuleOption, bool)]| {
        items
            .iter()
            .map(|(rule, enabled)| rule_value(state, *rule, *enabled))
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
        (RuleOption::OpenAllInside, rules.opened_all_inside),
        (RuleOption::SwapCalling, !rules.forbid_swap_calling),
        (RuleOption::DoubleYakuman, rules.double_yakuman),
    ]));
    lines.push(join(&[
        (RuleOption::FourKansDraw, rules.four_kans_draw),
        (RuleOption::FourWindsDraw, rules.four_winds_draw),
        (RuleOption::FourRiichiDraw, rules.four_riichi_draw),
    ]));
    lines.push(join(&[
        (RuleOption::NineTerminalsDraw, rules.nine_terminals_draw),
        (RuleOption::TripleRonDraw, rules.triple_ron_draw),
        (RuleOption::MultipleRon, rules.multiple_ron),
    ]));
    let mut final_rules = vec![(RuleOption::YakumanPao, rules.yakuman_pao)];
    if mode.three_player() {
        final_rules.push((RuleOption::NukiDora, rules.nuki_dora));
        final_rules.push((RuleOption::TsumoLoss, rules.tsumo_loss));
    }
    lines.push(join(&final_rules));
    lines.push(
        [
            tr.get(Key::FixedBankruptcy),
            tr.get(Key::FixedAfterTheFactYaku),
            tr.get(Key::FixedNagashiMangan),
        ]
        .join(separator),
    );
    lines.push(
        [
            tr.get(if mode.three_player() {
                Key::FixedRedDoraThreePlayer
            } else {
                Key::FixedRedDoraFourPlayer
            }),
            tr.get(Key::FixedPinfuTsumo),
        ]
        .join(separator),
    );
    lines.push(tr.get(Key::FixedDealerContinuation).to_string());
    lines.push(tr.get(Key::FixedKiriageMangan).to_string());
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

fn rule_value_selector_rect() -> Rect2 {
    Rect2 {
        x: DESIGN_W / 2.0 - 100.0,
        y: 511.0,
        w: 200.0,
        h: 30.0,
    }
}

fn rule_value_previous_rect() -> Rect2 {
    let selector = rule_value_selector_rect();
    Rect2 {
        x: selector.x,
        y: selector.y,
        w: 46.0,
        h: selector.h,
    }
}

fn rule_value_next_rect() -> Rect2 {
    let selector = rule_value_selector_rect();
    Rect2 {
        x: selector.x + selector.w - 46.0,
        y: selector.y,
        w: 46.0,
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
        rect.x + rect.w - 30.0,
        rect.center_y() + 5.0,
        11,
        if enabled {
            theme::GOLD_LT
        } else {
            theme::TEXT_DIM
        },
    );
}

fn draw_rule_value_selector(font: Option<&Font>, value: &str, enabled: bool) {
    let selector = rule_value_selector_rect();
    let previous = rule_value_previous_rect();
    let next = rule_value_next_rect();
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

fn toggle_selected_rule(state: &mut GameState, origin: MenuOrigin) {
    state
        .selected_rule
        .toggle(selected_rules_mut(state, origin));
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
    draw_menu_panel(font, tr.get(Key::RuleSettingsTitle), 26);

    for (idx, rule) in RuleOption::ALL.into_iter().enumerate() {
        let enabled = rule.is_enabled(rules);
        let value = tr.get(if enabled { Key::RuleOn } else { Key::RuleOff });
        let mut label = tr.get(rule_label_key(rule)).to_string();
        if matches!(rule, RuleOption::NukiDora | RuleOption::TsumoLoss) {
            label.push_str(tr.get(Key::SanmaOnlyNote));
        }
        draw_rule_button(
            font,
            &rule_rect(idx),
            &label,
            value,
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
    draw_rule_value_selector(
        font,
        tr.get(if selected_enabled {
            Key::RuleOn
        } else {
            Key::RuleOff
        }),
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

    for (idx, rule) in RuleOption::ALL.into_iter().enumerate() {
        if rule_rect(idx).contains(mx, my) {
            select_rule(state, rule);
            return None;
        }
    }

    if rule_value_previous_rect().contains(mx, my) || rule_value_next_rect().contains(mx, my) {
        toggle_selected_rule(state, origin);
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
        state.setup_state.rules.forbid_swap_calling = false;

        let lines = mode_summary_lines(&state, MenuOrigin::Local);

        assert_eq!(lines[0], "四人東風、25,000点始まり、南入なし");
        assert!(lines.iter().any(|line| line.contains("喰いタンあり")));
        assert!(lines.iter().any(|line| line.contains("喰い替えあり")));
        assert!(lines.iter().any(|line| line.contains("二倍役満あり")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("飛びあり（0点は続行）"))
        );
        assert!(lines.iter().any(|line| line.contains("後付けあり")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("赤ドラ：5m・5p・5s各1枚"))
        );
        assert!(lines.iter().any(|line| line.contains("流し満貫あり")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("親の和了・聴牌、途中流局"))
        );
        assert!(lines.iter().any(|line| line.contains("平和ツモ複合あり")));
        assert!(lines.iter().any(|line| line.contains("3翻60符・4翻30符")));
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
        assert!(
            lines
                .iter()
                .any(|line| line.contains("赤ドラ：5p・5s各1枚"))
        );
        assert!(!lines.iter().any(|line| line.contains("赤ドラ：5m")));
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
    fn multiple_ron_description_explains_head_bump_priority_and_fits_panel() {
        let ja = Key::RuleMultipleRonDescription.text(Lang::Ja);
        let en = Key::RuleMultipleRonDescription.text(Lang::En);

        for phrase in ["ダブロン・トリロン", "オフの時は頭ハネ", "供託棒は頭ハネ"]
        {
            assert!(ja.contains(phrase), "missing {phrase}");
        }
        assert!(en.contains("double and triple ron"));
        assert!(en.contains("Off uses head bump"));
        assert!(en.contains("Riichi deposits use head-bump priority"));
        assert_eq!(ja.lines().count(), 3);
        assert_eq!(en.lines().count(), 3);

        let description = rule_description_rect();
        let last_baseline = description.y + 73.0 + 2.0 * 18.0;
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

        toggle_selected_rule(&mut state, MenuOrigin::Local);

        assert_ne!(rule.is_enabled(&state.setup_state.rules), before);
    }
}
