//! State for the setup screen and the online UI.

use super::*;
use mahjong_core::settings::Settings;
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
    OpenAllInside,
    SwapCalling,
    DoubleYakuman,
    NukiDora,
    TsumoLoss,
    FourKansDraw,
    FourWindsDraw,
    FourRiichiDraw,
    NineTerminalsDraw,
    TripleRonDraw,
    MultipleRon,
    YakumanPao,
}

impl RuleOption {
    /// Every configurable rule in display order.
    pub const ALL: [RuleOption; 12] = [
        RuleOption::OpenAllInside,
        RuleOption::SwapCalling,
        RuleOption::DoubleYakuman,
        RuleOption::NukiDora,
        RuleOption::TsumoLoss,
        RuleOption::FourKansDraw,
        RuleOption::FourWindsDraw,
        RuleOption::FourRiichiDraw,
        RuleOption::NineTerminalsDraw,
        RuleOption::TripleRonDraw,
        RuleOption::MultipleRon,
        RuleOption::YakumanPao,
    ];

    /// Returns the user-facing enabled state of this rule.
    pub fn is_enabled(self, rules: &Settings) -> bool {
        match self {
            RuleOption::OpenAllInside => rules.opened_all_inside,
            RuleOption::SwapCalling => !rules.forbid_swap_calling,
            RuleOption::DoubleYakuman => rules.double_yakuman,
            RuleOption::NukiDora => rules.nuki_dora,
            RuleOption::TsumoLoss => rules.tsumo_loss,
            RuleOption::FourKansDraw => rules.four_kans_draw,
            RuleOption::FourWindsDraw => rules.four_winds_draw,
            RuleOption::FourRiichiDraw => rules.four_riichi_draw,
            RuleOption::NineTerminalsDraw => rules.nine_terminals_draw,
            RuleOption::TripleRonDraw => rules.triple_ron_draw,
            RuleOption::MultipleRon => rules.multiple_ron,
            RuleOption::YakumanPao => rules.yakuman_pao,
        }
    }

    /// Changes the rule to its opposite user-facing state.
    pub fn toggle(self, rules: &mut Settings) {
        match self {
            RuleOption::OpenAllInside => rules.opened_all_inside = !rules.opened_all_inside,
            RuleOption::SwapCalling => rules.forbid_swap_calling = !rules.forbid_swap_calling,
            RuleOption::DoubleYakuman => rules.double_yakuman = !rules.double_yakuman,
            RuleOption::NukiDora => rules.nuki_dora = !rules.nuki_dora,
            RuleOption::TsumoLoss => rules.tsumo_loss = !rules.tsumo_loss,
            RuleOption::FourKansDraw => rules.four_kans_draw = !rules.four_kans_draw,
            RuleOption::FourWindsDraw => rules.four_winds_draw = !rules.four_winds_draw,
            RuleOption::FourRiichiDraw => rules.four_riichi_draw = !rules.four_riichi_draw,
            RuleOption::NineTerminalsDraw => {
                rules.nine_terminals_draw = !rules.nine_terminals_draw;
            }
            RuleOption::TripleRonDraw => rules.triple_ron_draw = !rules.triple_ron_draw,
            RuleOption::MultipleRon => rules.multiple_ron = !rules.multiple_ron,
            RuleOption::YakumanPao => rules.yakuman_pao = !rules.yakuman_pao,
        }
    }
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
