//! State for the setup screen and the online UI.

use super::*;
use mahjong_core::settings::{AllLastRule, BankruptcyRule, Settings};
use mahjong_server::table::GameLength;

/// Game mode: player count x game length, as picked by the mode toggle
/// on the setup screen and online menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    /// Four-player East-only
    FourEast,
    /// Four-player hanchan
    FourHanchan,
    /// Three-player East-only
    ThreeEast,
    /// Three-player hanchan
    ThreeHanchan,
}

impl GameMode {
    /// Every mode, in toggle display order.
    pub const ALL: [GameMode; 4] = [
        GameMode::FourEast,
        GameMode::FourHanchan,
        GameMode::ThreeEast,
        GameMode::ThreeHanchan,
    ];

    /// Mode from the three-player flag and game length.
    pub fn from_parts(three_player: bool, length: GameLength) -> Self {
        match (three_player, length) {
            (false, GameLength::EastOnly) => GameMode::FourEast,
            (false, GameLength::Hanchan) => GameMode::FourHanchan,
            (true, GameLength::EastOnly) => GameMode::ThreeEast,
            (true, GameLength::Hanchan) => GameMode::ThreeHanchan,
        }
    }

    /// Whether this is a three-player mode.
    pub fn three_player(self) -> bool {
        matches!(self, GameMode::ThreeEast | GameMode::ThreeHanchan)
    }

    /// The game length.
    pub fn length(self) -> GameLength {
        match self {
            GameMode::FourEast | GameMode::ThreeEast => GameLength::EastOnly,
            GameMode::FourHanchan | GameMode::ThreeHanchan => GameLength::Hanchan,
        }
    }

    /// Label key shown on the mode toggle and in the lobby.
    pub fn label_key(self) -> Key {
        match self {
            GameMode::FourEast => Key::ModeFourEast,
            GameMode::FourHanchan => Key::ModeFourHanchan,
            GameMode::ThreeEast => Key::ModeThreeEast,
            GameMode::ThreeHanchan => Key::ModeThreeHanchan,
        }
    }
}

/// A rule exposed by the pre-game rule-settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOption {
    RedFives,
    DealerContinuation,
    RoundExtension,
    AllLast,
    Bankruptcy,
    ColdEnd,
    RonMode,
    AbortiveDrawMode,
    OpenAllInside,
    SwapCalling,
    NagashiMangan,
    KiriageMangan,
    CountedYakuman,
    DoubleYakuman,
    NukiDora,
    TsumoLoss,
    FourKansDraw,
    FourWindsDraw,
    FourRiichiDraw,
    NineTerminalsDraw,
    YakumanPao,
}

/// Category shown on the rule-settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulePage {
    Basic,
    Advanced,
}

impl RuleOption {
    /// Rules on one page, adjusted for four-player or three-player play.
    pub fn options(page: RulePage, three_player: bool) -> Vec<RuleOption> {
        match page {
            RulePage::Basic => {
                let mut rules = vec![
                    RuleOption::RedFives,
                    RuleOption::DealerContinuation,
                    RuleOption::RoundExtension,
                    RuleOption::AllLast,
                    RuleOption::Bankruptcy,
                    RuleOption::RonMode,
                    RuleOption::AbortiveDrawMode,
                ];
                if three_player {
                    rules.push(RuleOption::NukiDora);
                    rules.push(RuleOption::TsumoLoss);
                } else {
                    rules.push(RuleOption::ColdEnd);
                }
                rules
            }
            RulePage::Advanced => vec![
                RuleOption::OpenAllInside,
                RuleOption::SwapCalling,
                RuleOption::NagashiMangan,
                RuleOption::KiriageMangan,
                RuleOption::CountedYakuman,
                RuleOption::DoubleYakuman,
                RuleOption::YakumanPao,
                RuleOption::FourKansDraw,
                RuleOption::FourWindsDraw,
                RuleOption::FourRiichiDraw,
                RuleOption::NineTerminalsDraw,
            ],
        }
    }

    /// Returns the user-facing enabled state of this rule.
    pub fn is_enabled(self, rules: &Settings) -> bool {
        match self {
            RuleOption::RedFives => rules.red_fives,
            RuleOption::DealerContinuation => rules.tenpai_renchan,
            RuleOption::RoundExtension => rules.round_extension,
            RuleOption::AllLast => rules.all_last_rule != AllLastRule::Continue,
            RuleOption::Bankruptcy => rules.bankruptcy_rule != BankruptcyRule::None,
            RuleOption::ColdEnd => rules.cold_end,
            RuleOption::RonMode => rules.multiple_ron,
            RuleOption::AbortiveDrawMode => {
                rules.four_kans_draw
                    || rules.four_winds_draw
                    || rules.four_riichi_draw
                    || rules.nine_terminals_draw
            }
            RuleOption::OpenAllInside => rules.opened_all_inside,
            RuleOption::SwapCalling => !rules.forbid_swap_calling,
            RuleOption::NagashiMangan => rules.nagashi_mangan,
            RuleOption::KiriageMangan => rules.kiriage_mangan,
            RuleOption::CountedYakuman => rules.counted_yakuman,
            RuleOption::DoubleYakuman => rules.double_yakuman,
            RuleOption::NukiDora => rules.nuki_dora,
            RuleOption::TsumoLoss => rules.tsumo_loss,
            RuleOption::FourKansDraw => rules.four_kans_draw,
            RuleOption::FourWindsDraw => rules.four_winds_draw,
            RuleOption::FourRiichiDraw => rules.four_riichi_draw,
            RuleOption::NineTerminalsDraw => rules.nine_terminals_draw,
            RuleOption::YakumanPao => rules.yakuman_pao,
        }
    }

    /// Moves the selected rule value in either direction.
    pub fn cycle(self, rules: &mut Settings, forward: bool) {
        match self {
            RuleOption::RedFives => rules.red_fives = !rules.red_fives,
            RuleOption::DealerContinuation => rules.tenpai_renchan = !rules.tenpai_renchan,
            RuleOption::RoundExtension => rules.round_extension = !rules.round_extension,
            RuleOption::AllLast => {
                let value = match rules.all_last_rule {
                    AllLastRule::Continue => 0,
                    AllLastRule::Win => 1,
                    AllLastRule::WinOrTenpai => 2,
                };
                rules.all_last_rule = match cycle_three_values(value, forward) {
                    0 => AllLastRule::Continue,
                    1 => AllLastRule::Win,
                    _ => AllLastRule::WinOrTenpai,
                };
            }
            RuleOption::Bankruptcy => {
                let value = match rules.bankruptcy_rule {
                    BankruptcyRule::None => 0,
                    BankruptcyRule::Negative => 1,
                    BankruptcyRule::ZeroOrLess => 2,
                };
                rules.bankruptcy_rule = match cycle_three_values(value, forward) {
                    0 => BankruptcyRule::None,
                    1 => BankruptcyRule::Negative,
                    _ => BankruptcyRule::ZeroOrLess,
                };
            }
            RuleOption::ColdEnd => rules.cold_end = !rules.cold_end,
            RuleOption::RonMode => cycle_ron_mode(rules, forward),
            RuleOption::AbortiveDrawMode => cycle_abortive_draw_mode(rules, forward),
            RuleOption::OpenAllInside => rules.opened_all_inside = !rules.opened_all_inside,
            RuleOption::SwapCalling => rules.forbid_swap_calling = !rules.forbid_swap_calling,
            RuleOption::NagashiMangan => rules.nagashi_mangan = !rules.nagashi_mangan,
            RuleOption::KiriageMangan => rules.kiriage_mangan = !rules.kiriage_mangan,
            RuleOption::CountedYakuman => rules.counted_yakuman = !rules.counted_yakuman,
            RuleOption::DoubleYakuman => rules.double_yakuman = !rules.double_yakuman,
            RuleOption::NukiDora => rules.nuki_dora = !rules.nuki_dora,
            RuleOption::TsumoLoss => rules.tsumo_loss = !rules.tsumo_loss,
            RuleOption::FourKansDraw => rules.four_kans_draw = !rules.four_kans_draw,
            RuleOption::FourWindsDraw => rules.four_winds_draw = !rules.four_winds_draw,
            RuleOption::FourRiichiDraw => rules.four_riichi_draw = !rules.four_riichi_draw,
            RuleOption::NineTerminalsDraw => {
                rules.nine_terminals_draw = !rules.nine_terminals_draw;
            }
            RuleOption::YakumanPao => cycle_yakuman_pao(rules, forward),
        }
    }
}

fn cycle_three_values(value: usize, forward: bool) -> usize {
    (value + if forward { 1 } else { 2 }) % 3
}

fn cycle_ron_mode(rules: &mut Settings, forward: bool) {
    let value = match (rules.multiple_ron, rules.triple_ron_draw) {
        (false, _) => 0,
        (true, false) => 1,
        (true, true) => 2,
    };
    let next = cycle_three_values(value, forward);
    (rules.multiple_ron, rules.triple_ron_draw) = match next {
        0 => (false, false),
        1 => (true, false),
        _ => (true, true),
    };
}

fn cycle_abortive_draw_mode(rules: &mut Settings, forward: bool) {
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
    let value = if none {
        0
    } else if standard {
        1
    } else if all {
        2
    } else {
        3
    };
    let next = match (value, forward) {
        (0, true) | (2, false) | (3, _) => 1,
        (1, true) => 2,
        (1, false) => 0,
        (2, true) => 0,
        (0, false) => 2,
        _ => unreachable!(),
    };
    let values = match next {
        0 => (false, false, false, false),
        1 => (true, true, false, true),
        _ => (true, true, true, true),
    };
    (
        rules.four_kans_draw,
        rules.four_winds_draw,
        rules.four_riichi_draw,
        rules.nine_terminals_draw,
    ) = values;
}

fn cycle_yakuman_pao(rules: &mut Settings, forward: bool) {
    let value = if !rules.yakuman_pao {
        0
    } else if !rules.four_quads_pao {
        1
    } else {
        2
    };
    let next = cycle_three_values(value, forward);
    (rules.yakuman_pao, rules.four_quads_pao) = match next {
        0 => (false, false),
        1 => (true, false),
        _ => (true, true),
    };
}

/// State of the online UI (menu and lobby).
#[derive(Debug, Clone)]
pub struct OnlineUiState {
    /// Display-name input field
    pub name_input: String,
    /// Room-code input field
    pub code_input: String,
    /// Focus: true = room-code field, false = name field
    pub code_focused: bool,
    /// Connection status / error text
    pub status_line: Option<String>,
    /// Whether status_line is an error (shown in red)
    pub status_is_error: bool,
    /// The joined room's display info, copied from the adapter
    pub room: Option<RoomViewUi>,
    /// Seconds left on the turn timer; Some only on our own online turn
    pub turn_remaining: Option<u32>,
    /// Mode used when creating a room
    pub mode: GameMode,
    /// Rules used when creating a room
    pub rules: Settings,
}

impl OnlineUiState {
    pub fn new() -> Self {
        OnlineUiState {
            // display_name() fills the language-appropriate default on send.
            name_input: String::new(),
            code_input: String::new(),
            code_focused: false,
            status_line: None,
            status_is_error: false,
            room: None,
            turn_remaining: None,
            mode: GameMode::FourEast,
            rules: Settings::new(),
        }
    }

    /// Builds the rule settings sent when creating a room.
    ///
    pub fn build_rules(&self) -> Settings {
        let mut rules = self.rules.clone();
        rules.three_player = self.mode.three_player();
        rules
    }

    /// The game length sent when creating a room.
    pub fn length(&self) -> GameLength {
        self.mode.length()
    }
}

/// Room info shown on the lobby screen.
#[derive(Debug, Clone)]
pub struct RoomViewUi {
    /// Room code
    pub code: String,
    /// Seat captions in wind order
    pub seat_labels: [String; 4],
    /// Whether we are the host (controls the start button)
    pub is_host: bool,
    /// The room's game mode
    pub mode: GameMode,
}

/// State of the pre-game setup screen.
#[derive(Debug, Clone)]
pub struct SetupState {
    /// Game mode
    pub mode: GameMode,
    /// Rules used for local play
    pub rules: Settings,
    /// CPU levels (right, across, left)
    pub cpu_levels: [usize; 3],
    /// CPU personalities (right, across, left)
    pub cpu_personalities: [usize; 3],
}

impl SetupState {
    pub fn new() -> Self {
        SetupState {
            mode: GameMode::FourEast,
            rules: Settings::new(),
            cpu_levels: [1, 1, 1],        // all Normal
            cpu_personalities: [0, 1, 2], // Balanced, Speedy, HighValue
        }
    }

    /// Number of CPUs to configure (3 four-player, 2 three-player).
    pub fn cpu_count(&self) -> usize {
        if self.mode.three_player() { 2 } else { 3 }
    }

    /// Builds the selected rule settings.
    ///
    pub fn build_rules(&self) -> Settings {
        let mut rules = self.rules.clone();
        rules.three_player = self.mode.three_player();
        rules
    }

    /// Builds the game settings; the starting score follows the rules.
    pub fn build_game_settings(&self) -> mahjong_server::table::GameSettings {
        mahjong_server::table::GameSettings::with_rules(self.mode.length(), self.build_rules())
    }

    pub fn level_count() -> usize {
        3
    }
    pub fn personality_count() -> usize {
        4
    }

    /// Builds the CpuConfig array from the settings.
    pub fn build_configs(&self) -> [CpuConfig; 3] {
        let to_level = |idx: usize| -> CpuLevel {
            match idx {
                0 => CpuLevel::Weak,
                2 => CpuLevel::Strong,
                _ => CpuLevel::Normal,
            }
        };
        let to_personality = |idx: usize| -> CpuPersonality {
            match idx {
                1 => CpuPersonality::Speedy,
                2 => CpuPersonality::HighValue,
                3 => CpuPersonality::Defensive,
                _ => CpuPersonality::Balanced,
            }
        };
        [
            CpuConfig::new(
                to_level(self.cpu_levels[0]),
                to_personality(self.cpu_personalities[0]),
            ),
            CpuConfig::new(
                to_level(self.cpu_levels[1]),
                to_personality(self.cpu_personalities[1]),
            ),
            CpuConfig::new(
                to_level(self.cpu_levels[2]),
                to_personality(self.cpu_personalities[2]),
            ),
        ]
    }

    /// Builds the CPU specs the host sends in online play.
    pub fn build_cpu_specs(&self) -> [CpuSpec; 3] {
        self.build_configs()
            .map(|config| CpuSpec::from_config(&config))
    }
}
