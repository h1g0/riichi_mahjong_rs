//! Client UI internationalization.
//!
//! The display language reuses [`mahjong_core::settings::Lang`]. Fixed
//! strings live in the [`Key`] enum with every language defined per key
//! (so missing translations cannot slip through); parameterized strings
//! (with numbers, tiles, ...) are built by [`Translator`] methods to
//! absorb word-order differences between languages.
//!
//! Game terms - yaku names, score ranks, winds, dora - are canonically
//! localized in `mahjong-core`; this module covers UI-only strings.

use mahjong_core::settings::Lang;
use mahjong_core::tile::Wind;
use mahjong_server::cpu::client::{CpuLevel, CpuPersonality};
use mahjong_server::protocol::DrawReason;

/// Lightweight handle holding the current language and resolving strings.
///
/// Owned by [`GameState`](crate::game::GameState) and passed to render
/// functions via `&GameState`; `Copy`, so duplicate freely.
#[derive(Debug, Clone, Copy)]
pub struct Translator {
    lang: Lang,
}

impl Translator {
    /// Creates a [`Translator`] for the language.
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// Resolves a fixed string.
    pub fn get(&self, key: Key) -> &'static str {
        key.text(self.lang)
    }

    /// The current display language.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// CPU level label (0 = weak, 1 = normal, 2 = strong).
    pub fn strength_label(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["弱い", "普通", "強い"],
            Lang::En => ["Weak", "Normal", "Strong"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// CPU personality label (0 = balanced, 1 = speedy, 2 = high value,
    /// 3 = defensive).
    pub fn personality_label(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["バランス", "スピード", "高得点", "守備的"],
            Lang::En => ["Balanced", "Speedy", "High Value", "Defensive"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// CPU card name on the setup screen (e.g. "CPU 1"); numbered rather
    /// than positional because seats are randomized at game start.
    pub fn cpu_slot(&self, idx: usize) -> String {
        format!("CPU {}", idx + 1)
    }

    /// Hand label (JA 「東1局」 / EN "East 1"); `round_number` is 0-based
    /// and `rounds_per_wind` equals the player count.
    pub fn round_label(&self, round_number: usize, rounds_per_wind: usize) -> String {
        let wind = Wind::from_index(round_number / rounds_per_wind).name(self.lang);
        let num = (round_number % rounds_per_wind) + 1;
        match self.lang {
            Lang::Ja => format!("{wind}{num}局"),
            Lang::En => format!("{wind} {num}"),
        }
    }

    /// Honba caption (JA 「{n}本場」 / EN "{n} honba") on the top bar.
    pub fn honba_suffix(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("{n}本場"),
            Lang::En => format!("{n} honba"),
        }
    }

    /// Remaining-tiles caption (JA 「残{n}枚」 / EN "{n} left").
    pub fn wall_remaining(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("残{n}枚"),
            Lang::En => format!("{n} left"),
        }
    }

    /// Han caption (JA 「{n}飜」 / EN "{n} han").
    pub fn han(&self, n: u32) -> String {
        match self.lang {
            Lang::Ja => format!("{n}飜"),
            Lang::En => format!("{n} han"),
        }
    }

    /// Combined han/fu caption (JA 「{han}飜 {fu}符」 / EN "{han} han {fu} fu").
    pub fn han_fu(&self, han: u32, fu: u32) -> String {
        match self.lang {
            Lang::Ja => format!("{han}飜 {fu}符"),
            Lang::En => format!("{han} han {fu} fu"),
        }
    }

    /// Points caption (JA 「{s}点」 / EN "{s} pts"); `s` is pre-formatted.
    pub fn points(&self, s: &str) -> String {
        match self.lang {
            Lang::Ja => format!("{s}点"),
            Lang::En => format!("{s} pts"),
        }
    }

    /// Deposit-points caption shown under the score (JA 「＋供託{n}本　{s}点」 /
    /// EN "+deposit {n}: {s} pts"); `s` is pre-formatted.
    pub fn deposit_points(&self, n: usize, s: &str) -> String {
        match self.lang {
            Lang::Ja => format!("＋供託{n}本　{s}点"),
            Lang::En => format!("+deposit {n}: {s} pts"),
        }
    }

    /// Lobby room-code heading.
    pub fn room_code(&self, code: &str) -> String {
        match self.lang {
            Lang::Ja => format!("ルームコード  {code}"),
            Lang::En => format!("Room code  {code}"),
        }
    }

    /// CPU level name from `CpuLevel`.
    pub fn cpu_level_name(&self, level: CpuLevel) -> &'static str {
        let idx = match level {
            CpuLevel::Weak => 0,
            CpuLevel::Normal => 1,
            CpuLevel::Strong => 2,
        };
        self.strength_label(idx)
    }

    /// CPU personality name from `CpuPersonality`.
    pub fn cpu_personality_name(&self, personality: CpuPersonality) -> &'static str {
        let idx = match personality {
            CpuPersonality::Balanced => 0,
            CpuPersonality::Speedy => 1,
            CpuPersonality::HighValue => 2,
            CpuPersonality::Defensive => 3,
        };
        self.personality_label(idx)
    }

    /// Lobby CPU seat label (JA 「CPU (普通・バランス)」 /
    /// EN "CPU (Normal, Balanced)").
    pub fn cpu_seat_label(&self, level: CpuLevel, personality: CpuPersonality) -> String {
        let lv = self.cpu_level_name(level);
        let ps = self.cpu_personality_name(personality);
        match self.lang {
            Lang::Ja => format!("CPU ({lv}・{ps})"),
            Lang::En => format!("CPU ({lv}, {ps})"),
        }
    }

    /// Lobby empty-seat row, noting which CPU would fill it
    /// (JA 「空席（CPU: 普通・バランス）」 / EN "Empty (CPU: ...)").
    pub fn empty_seat_cpu_label(&self, level: CpuLevel, personality: CpuPersonality) -> String {
        let lv = self.cpu_level_name(level);
        let ps = self.cpu_personality_name(personality);
        match self.lang {
            Lang::Ja => format!("空席（CPU: {lv}・{ps}）"),
            Lang::En => format!("Empty (CPU: {lv}, {ps})"),
        }
    }

    /// Lobby member row ("1: {who}{marks}"); numbered by join order
    /// (1-based) rather than wind, since seats are randomized at start.
    pub fn seat_row(&self, number: usize, who: &str, marks: &str) -> String {
        format!("{number}: {who}{marks}")
    }

    /// Disconnected-player seat caption (JA 「{name}（切断中）」 /
    /// EN "{name} (offline)").
    pub fn disconnected_name(&self, name: &str) -> String {
        format!("{name}{}", self.get(Key::MarkerDisconnected))
    }

    /// Names of the draw reasons.
    pub fn draw_reason(&self, reason: DrawReason) -> &'static str {
        match self.lang {
            Lang::Ja => match reason {
                DrawReason::Exhaustive => "荒牌流局",
                DrawReason::FourWinds => "四風連打",
                DrawReason::FourRiichi => "四家立直",
                DrawReason::NineTerminals => "九種九牌",
                DrawReason::FourKans => "四槓散了",
                DrawReason::TripleRon => "三家和",
            },
            Lang::En => match reason {
                DrawReason::Exhaustive => "Exhaustive draw",
                DrawReason::FourWinds => "Four winds",
                DrawReason::FourRiichi => "Four riichi",
                DrawReason::NineTerminals => "Nine terminals",
                DrawReason::FourKans => "Four quads",
                DrawReason::TripleRon => "Triple ron",
            },
        }
    }

    /// Draw heading (JA 「流局（{reason}）」 / EN "Draw ({reason})").
    pub fn draw_headline(&self, reason: DrawReason) -> String {
        let reason = self.draw_reason(reason);
        match self.lang {
            Lang::Ja => format!("流局（{reason}）"),
            Lang::En => format!("Draw ({reason})"),
        }
    }

    /// Tenpai-players line (JA 「テンパイ: {names}」 / EN "Tenpai: {names}").
    pub fn tenpai_list(&self, names: &str) -> String {
        match self.lang {
            Lang::Ja => format!("テンパイ: {names}"),
            Lang::En => format!("Tenpai: {names}"),
        }
    }

    /// Deposits line (JA 「供託: {n}本」 / EN "Deposits: {n}").
    pub fn deposit_line(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("供託: {n}本"),
            Lang::En => format!("Deposits: {n}"),
        }
    }

    /// Deal-in note (JA 「（{name}が放銃）」 / EN " (dealt in by {name})").
    pub fn dealt_in_by(&self, name: &str) -> String {
        match self.lang {
            Lang::Ja => format!("（{name}が放銃）"),
            Lang::En => format!(" (dealt in by {name})"),
        }
    }

    /// Win heading (JA 「{winner}が{type}和了！」 /
    /// EN "{winner} wins by {type}!").
    pub fn win_headline(&self, winner: &str, win_type: &str) -> String {
        match self.lang {
            Lang::Ja => format!("{winner}が{win_type}和了！"),
            Lang::En => format!("{winner} wins by {win_type}!"),
        }
    }

    /// Turn-timer caption (JA 「残り {n} 秒」 / EN "{n}s left").
    pub fn seconds_left(&self, n: u32) -> String {
        match self.lang {
            Lang::Ja => format!("残り {n} 秒"),
            Lang::En => format!("{n}s left"),
        }
    }

    /// Rank suffix (JA always 「位」, EN ordinal suffixes); `rank` is 0-based.
    pub fn place_suffix(&self, rank: usize) -> &'static str {
        match self.lang {
            Lang::Ja => "位",
            Lang::En => match rank {
                0 => "st",
                1 => "nd",
                2 => "rd",
                _ => "th",
            },
        }
    }
}

/// Keys for fixed, parameterless UI strings.
///
/// Every variant's translations live together in [`Key::text`]. Adding a
/// language means extending `Lang` and each `match` arm; exhaustiveness
/// is compiler-checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Title: local CPU game button
    CpuBattle,
    /// Title: rules-settings button (not yet implemented)
    SettingsMenu,
    /// Not-yet-implemented note
    ComingSoon,
    /// Title: language heading
    LanguageLabel,
    /// Mode-selection screen title
    ModeSelectTitle,
    /// Note that pei dora applies to three-player games only
    SanmaOnlyNote,
    /// CPU-setup screen title / lobby CPU-setup button
    CpuSetupTitle,
    /// Confirm button (online CPU setup)
    Confirm,
    /// Mode: four-player East-only
    ModeFourEast,
    /// Mode: four-player hanchan
    ModeFourHanchan,
    /// Mode: three-player East-only
    ModeThreeEast,
    /// Mode: three-player hanchan
    ModeThreeHanchan,
    /// Pei dora toggle
    NukiDoraToggle,
    /// Double yakuman variants toggle
    DoubleYakumanToggle,
    /// CPU level heading
    CpuStrengthLabel,
    /// CPU personality heading
    CpuPersonalityLabel,
    /// Start-local-game button
    StartGame,
    /// Go-online button
    OnlinePlay,
    /// Waiting-for-start caption
    GameStarting,
    /// Our own name on score chips etc.
    You,
    /// Furiten badge
    Furiten,
    /// Riichi badge
    RiichiActive,
    /// Badge prompting the riichi discard choice
    SelectDiscard,
    /// Warning badge: the selected tile causes furiten
    WillBeFuriten,
    /// Swap-calling warning badge
    IsSwapCalling,
    /// Tsumo (win/tile label)
    Tsumo,
    /// Ron
    Ron,
    /// Next winner (multiple ron)
    NextWin,
    /// Next
    Next,
    /// Draw
    RoundDraw,
    /// Game over
    GameOver,
    /// Play again
    PlayAgain,
    /// Win button
    Win,
    /// Waiting-for-others hint
    WaitingOtherPlayer,
    /// Riichi discard-choice hint
    RiichiSelectHint,
    /// Auto-tsumogiri-under-riichi hint
    RiichiAutoDiscard,
    /// Normal discard hint
    NormalPlayHint,
    /// Riichi button
    Riichi,
    /// Pon
    Pon,
    /// Kan
    Kan,
    /// Chii
    Chi,
    /// Pei (North extraction button)
    Pei,
    /// Pass
    Pass,
    /// Cancel
    Cancel,
    /// Chii option-picker heading
    ChiSelectTitle,
    /// Pon option-picker heading
    PonSelectTitle,
    /// Nine-terminals heading
    NineTerminals,
    /// Nine-terminals confirmation prompt
    DeclareDrawPrompt,
    /// Declare-the-draw button
    DeclareDraw,
    /// Continue (decline nine terminals)
    Continue,
    /// Name-field heading
    NameLabel,
    /// Room-code-field heading
    RoomCodeJoinLabel,
    /// Create-room button
    CreateRoom,
    /// Join-room button
    JoinRoom,
    /// Back button
    Back,
    /// Lobby heading
    Lobby,
    /// Fetching-room-info caption
    LoadingRoom,
    /// Share-the-room-code hint
    ShareCodeHint,
    /// Note that CPUs fill empty seats
    EmptySeatsCpu,
    /// Waiting-for-host caption
    WaitingHost,
    /// Leave button
    Leave,
    /// "You" seat marker
    MarkerYou,
    /// "Host" seat marker
    MarkerHost,
    /// "Offline" seat marker
    MarkerDisconnected,
    /// Connecting caption
    Connecting,
    /// Disconnected caption
    Disconnected,
    /// Empty seat
    EmptySeat,
    /// Default display name
    DefaultPlayerName,
    /// Room-code length error
    RoomCodeLengthError,
    /// Reconnecting caption
    Reconnecting,
    /// Opponent-disconnected / CPU-substitution caption
    PeerDisconnected,
    /// Generic transport-error caption (details go to the log)
    NetworkError,
}

impl Key {
    /// The string in the given language.
    pub fn text(self, lang: Lang) -> &'static str {
        match self {
            Key::CpuBattle => match lang {
                Lang::Ja => "CPU対戦",
                Lang::En => "VS CPU",
            },
            Key::SettingsMenu => match lang {
                Lang::Ja => "設定",
                Lang::En => "Settings",
            },
            Key::ComingSoon => match lang {
                Lang::Ja => "（準備中）",
                Lang::En => " (coming soon)",
            },
            Key::LanguageLabel => match lang {
                Lang::Ja => "言語設定",
                Lang::En => "Language",
            },
            Key::ModeSelectTitle => match lang {
                Lang::Ja => "モード選択",
                Lang::En => "Select Mode",
            },
            Key::SanmaOnlyNote => match lang {
                Lang::Ja => "（三麻のみ）",
                Lang::En => " (3-player only)",
            },
            Key::CpuSetupTitle => match lang {
                Lang::Ja => "CPU設定",
                Lang::En => "CPU Settings",
            },
            Key::Confirm => match lang {
                Lang::Ja => "決定",
                Lang::En => "Confirm",
            },
            Key::ModeFourEast => match lang {
                Lang::Ja => "四人東風",
                Lang::En => "4P East",
            },
            Key::ModeFourHanchan => match lang {
                Lang::Ja => "四人半荘",
                Lang::En => "4P Hanchan",
            },
            Key::ModeThreeEast => match lang {
                Lang::Ja => "三人東風",
                Lang::En => "3P East",
            },
            Key::ModeThreeHanchan => match lang {
                Lang::Ja => "三人半荘",
                Lang::En => "3P Hanchan",
            },
            Key::NukiDoraToggle => match lang {
                Lang::Ja => "北抜きドラ",
                Lang::En => "Pei dora",
            },
            Key::DoubleYakumanToggle => match lang {
                Lang::Ja => "二倍役満",
                Lang::En => "Double yakuman",
            },
            Key::CpuStrengthLabel => match lang {
                Lang::Ja => "強さ",
                Lang::En => "Strength",
            },
            Key::CpuPersonalityLabel => match lang {
                Lang::Ja => "性格",
                Lang::En => "Style",
            },
            Key::StartGame => match lang {
                Lang::Ja => "対局開始",
                Lang::En => "Start Game",
            },
            Key::OnlinePlay => match lang {
                Lang::Ja => "オンライン対戦",
                Lang::En => "Online Play",
            },
            Key::GameStarting => match lang {
                Lang::Ja => "ゲーム開始中...",
                Lang::En => "Starting game...",
            },
            Key::You => match lang {
                Lang::Ja => "あなた",
                Lang::En => "You",
            },
            Key::Furiten => match lang {
                Lang::Ja => "振聴",
                Lang::En => "Furiten",
            },
            Key::RiichiActive => match lang {
                Lang::Ja => "リーチ中",
                Lang::En => "Riichi",
            },
            Key::SelectDiscard => match lang {
                Lang::Ja => "打牌を選択",
                Lang::En => "Select a discard",
            },
            Key::WillBeFuriten => match lang {
                Lang::Ja => "振聴になります！",
                Lang::En => "Will cause furiten!",
            },
            Key::IsSwapCalling => match lang {
                Lang::Ja => "喰い替えです！",
                Lang::En => "That's swap-calling!",
            },
            Key::Tsumo => match lang {
                Lang::Ja => "ツモ",
                Lang::En => "Tsumo",
            },
            Key::Ron => match lang {
                Lang::Ja => "ロン",
                Lang::En => "Ron",
            },
            Key::NextWin => match lang {
                Lang::Ja => "次の和了へ →",
                Lang::En => "Next win →",
            },
            Key::Next => match lang {
                Lang::Ja => "次へ →",
                Lang::En => "Next →",
            },
            Key::RoundDraw => match lang {
                Lang::Ja => "流局",
                Lang::En => "Draw",
            },
            Key::GameOver => match lang {
                Lang::Ja => "ゲーム終了",
                Lang::En => "Game Over",
            },
            Key::PlayAgain => match lang {
                Lang::Ja => "もう一度",
                Lang::En => "Play Again",
            },
            Key::Win => match lang {
                Lang::Ja => "和了",
                Lang::En => "Win",
            },
            Key::WaitingOtherPlayer => match lang {
                Lang::Ja => "他のプレイヤーの手番です...",
                Lang::En => "Waiting for other players...",
            },
            Key::RiichiSelectHint => match lang {
                Lang::Ja => "【リーチ】聴牌になる牌を選んで打牌",
                Lang::En => "[Riichi] Discard a tile that keeps you tenpai",
            },
            Key::RiichiAutoDiscard => match lang {
                Lang::Ja => "【リーチ中】自動ツモ切り",
                Lang::En => "[Riichi] Auto-discarding draws",
            },
            Key::NormalPlayHint => match lang {
                Lang::Ja => "牌をクリックで選択、もう一度クリックで打牌",
                Lang::En => "Click a tile to select, click again to discard",
            },
            Key::Riichi => match lang {
                Lang::Ja => "リーチ",
                Lang::En => "Riichi",
            },
            Key::Pon => match lang {
                Lang::Ja => "ポン",
                Lang::En => "Pon",
            },
            Key::Kan => match lang {
                Lang::Ja => "カン",
                // "kan" per the glossary (docs/glossary.md).
                Lang::En => "Kan",
            },
            Key::Chi => match lang {
                Lang::Ja => "チー",
                // "chii" per the glossary (docs/glossary.md).
                Lang::En => "Chii",
            },
            Key::Pei => match lang {
                Lang::Ja => "北抜き",
                Lang::En => "Pei",
            },
            Key::Pass => match lang {
                Lang::Ja => "パス",
                Lang::En => "Pass",
            },
            Key::Cancel => match lang {
                Lang::Ja => "キャンセル",
                Lang::En => "Cancel",
            },
            Key::ChiSelectTitle => match lang {
                Lang::Ja => "チーの組み合わせを選択",
                Lang::En => "Choose a chii combination",
            },
            Key::PonSelectTitle => match lang {
                Lang::Ja => "ポンの組み合わせを選択",
                Lang::En => "Choose a pon combination",
            },
            Key::NineTerminals => match lang {
                Lang::Ja => "九種九牌",
                Lang::En => "Nine Terminals",
            },
            Key::DeclareDrawPrompt => match lang {
                Lang::Ja => "流局しますか？",
                Lang::En => "Declare a draw?",
            },
            Key::DeclareDraw => match lang {
                Lang::Ja => "流局する",
                Lang::En => "Declare draw",
            },
            Key::Continue => match lang {
                Lang::Ja => "続ける",
                Lang::En => "Continue",
            },
            Key::NameLabel => match lang {
                Lang::Ja => "名前",
                Lang::En => "Name",
            },
            Key::RoomCodeJoinLabel => match lang {
                Lang::Ja => "ルームコード（参加する場合）",
                Lang::En => "Room code (to join)",
            },
            Key::CreateRoom => match lang {
                Lang::Ja => "ルームを作成",
                Lang::En => "Create Room",
            },
            Key::JoinRoom => match lang {
                Lang::Ja => "ルームに参加",
                Lang::En => "Join Room",
            },
            Key::Back => match lang {
                Lang::Ja => "戻る",
                Lang::En => "Back",
            },
            Key::Lobby => match lang {
                Lang::Ja => "ロビー",
                Lang::En => "Lobby",
            },
            Key::LoadingRoom => match lang {
                Lang::Ja => "ルーム情報を取得中...",
                Lang::En => "Loading room...",
            },
            Key::ShareCodeHint => match lang {
                Lang::Ja => "このコードを参加プレイヤーに共有してください",
                Lang::En => "Share this code with the players joining",
            },
            Key::EmptySeatsCpu => match lang {
                Lang::Ja => "空席はCPUが入ります",
                Lang::En => "Empty seats are filled by CPUs",
            },
            Key::WaitingHost => match lang {
                Lang::Ja => "ホストの開始を待っています...",
                Lang::En => "Waiting for the host to start...",
            },
            Key::Leave => match lang {
                Lang::Ja => "退出",
                Lang::En => "Leave",
            },
            Key::MarkerYou => match lang {
                Lang::Ja => "（あなた）",
                Lang::En => " (You)",
            },
            Key::MarkerHost => match lang {
                Lang::Ja => "（ホスト）",
                Lang::En => " (Host)",
            },
            Key::MarkerDisconnected => match lang {
                Lang::Ja => "（切断中）",
                Lang::En => " (offline)",
            },
            Key::Connecting => match lang {
                Lang::Ja => "サーバに接続中...",
                Lang::En => "Connecting to server...",
            },
            Key::Disconnected => match lang {
                Lang::Ja => "サーバとの接続が切れました",
                Lang::En => "Disconnected from server",
            },
            Key::EmptySeat => match lang {
                Lang::Ja => "空席",
                Lang::En => "Empty",
            },
            Key::DefaultPlayerName => match lang {
                Lang::Ja => "プレイヤー",
                Lang::En => "Player",
            },
            Key::RoomCodeLengthError => match lang {
                Lang::Ja => "ルームコードを6文字で入力してください",
                Lang::En => "Enter a 6-character room code",
            },
            Key::Reconnecting => match lang {
                Lang::Ja => "再接続中...",
                Lang::En => "Reconnecting...",
            },
            Key::PeerDisconnected => match lang {
                Lang::Ja => "他のプレイヤーが切断中（CPUが代打ち）",
                Lang::En => "A player disconnected (a CPU is filling in)",
            },
            Key::NetworkError => match lang {
                Lang::Ja => "通信エラーが発生しました",
                Lang::En => "A network error occurred",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_resolves_both_languages() {
        assert_eq!(Key::StartGame.text(Lang::Ja), "対局開始");
        assert_eq!(Key::StartGame.text(Lang::En), "Start Game");
    }

    #[test]
    fn translator_indexed_labels() {
        let ja = Translator::new(Lang::Ja);
        let en = Translator::new(Lang::En);
        assert_eq!(ja.strength_label(0), "弱い");
        assert_eq!(en.strength_label(2), "Strong");
        assert_eq!(ja.personality_label(3), "守備的");
        assert_eq!(en.personality_label(0), "Balanced");
        assert_eq!(ja.cpu_slot(1), "CPU 2");
        assert_eq!(en.cpu_slot(2), "CPU 3");
    }

    /// The empty-seat row must localize its CPU config note (#245).
    #[test]
    fn empty_seat_cpu_label_localizes() {
        let ja = Translator::new(Lang::Ja);
        let en = Translator::new(Lang::En);
        assert_eq!(
            ja.empty_seat_cpu_label(CpuLevel::Normal, CpuPersonality::Balanced),
            "空席（CPU: 普通・バランス）"
        );
        assert_eq!(
            en.empty_seat_cpu_label(CpuLevel::Strong, CpuPersonality::Defensive),
            "Empty (CPU: Strong, Defensive)"
        );
    }

    #[test]
    fn out_of_range_index_is_empty() {
        let t = Translator::new(Lang::En);
        assert_eq!(t.strength_label(9), "");
    }

    /// The hand label must respect hands-per-wind (4 or 3) (#271).
    #[test]
    fn round_label_respects_rounds_per_wind() {
        let ja = Translator::new(Lang::Ja);
        let en = Translator::new(Lang::En);
        // Four-player: South 1 follows East 4.
        assert_eq!(ja.round_label(3, 4), "東4局");
        assert_eq!(ja.round_label(4, 4), "南1局");
        // Three-player: South 1 follows East 3.
        assert_eq!(ja.round_label(2, 3), "東3局");
        assert_eq!(ja.round_label(3, 3), "南1局");
        assert_eq!(en.round_label(3, 3), "South 1");
    }
}
