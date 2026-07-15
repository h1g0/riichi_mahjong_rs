//! State for the setup screen and the online UI.

use super::*;
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
    /// Whether pei dora is on when creating a room (three-player only)
    pub nuki_dora: bool,
    /// Whether the four special variants are worth double yakuman
    pub double_yakuman: bool,
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
            nuki_dora: true,
            double_yakuman: true,
        }
    }

    /// Builds the rule settings sent when creating a room.
    ///
    /// Rules without UI stay at their defaults; new rule pickers only
    /// need to be reflected here to reach the server.
    pub fn build_rules(&self) -> mahjong_core::settings::Settings {
        mahjong_core::settings::Settings {
            three_player: self.mode.three_player(),
            nuki_dora: self.nuki_dora,
            double_yakuman: self.double_yakuman,
            ..mahjong_core::settings::Settings::new()
        }
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
    /// Whether pei dora is on (three-player only)
    pub nuki_dora: bool,
    /// Whether the four special variants are worth double yakuman
    pub double_yakuman: bool,
    /// CPU levels (right, across, left)
    pub cpu_levels: [usize; 3],
    /// CPU personalities (right, across, left)
    pub cpu_personalities: [usize; 3],
}

impl SetupState {
    pub fn new() -> Self {
        SetupState {
            mode: GameMode::FourEast,
            nuki_dora: true,
            double_yakuman: true,
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
    /// Rules without UI stay at their defaults; new rule pickers only
    /// need to be reflected here to reach both local and online play.
    pub fn build_rules(&self) -> mahjong_core::settings::Settings {
        mahjong_core::settings::Settings {
            three_player: self.mode.three_player(),
            nuki_dora: self.nuki_dora,
            double_yakuman: self.double_yakuman,
            ..mahjong_core::settings::Settings::new()
        }
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
