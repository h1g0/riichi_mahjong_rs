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

    /// Continuance-points caption shown under the score
    /// (JA 「＋{n}本場　{s}点」 / EN "+{n} honba: {s} pts"); `s` is
    /// pre-formatted.
    pub fn honba_points(&self, n: usize, s: &str) -> String {
        match self.lang {
            Lang::Ja => format!("＋{n}本場　{s}点"),
            Lang::En => format!("+{n} honba: {s} pts"),
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
    /// Open the pre-game rule-settings screen
    RuleSettingsButton,
    /// Rule-settings screen title
    RuleSettingsTitle,
    /// Basic rule-settings category
    RuleBasicPage,
    /// Advanced rule-settings category
    RuleAdvancedPage,
    /// Heading above the compact rule summary
    CurrentRulesTitle,
    /// Enabled value shown beside a rule
    RuleOn,
    /// Disabled value shown beside a rule
    RuleOff,
    /// Red-five rule label
    RuleRedFives,
    /// Dealer-continuation rule label
    RuleDealerContinuation,
    /// South/West extension rule label
    RuleRoundExtension,
    /// Final-dealer stopping rule label
    RuleAllLast,
    /// Bankruptcy rule label
    RuleBankruptcy,
    /// 55,000-point cold-end rule label
    RuleColdEnd,
    /// Simultaneous-ron handling label
    RuleRonMode,
    /// Abortive-draw handling label
    RuleAbortiveDrawMode,
    /// Nagashi Mangan rule label
    RuleNagashiMangan,
    /// Kiriage Mangan rule label
    RuleKiriageMangan,
    /// Counted-yakuman rule label
    RuleCountedYakuman,
    /// Open All Inside rule label
    RuleOpenAllInside,
    /// Swap-calling rule label
    RuleSwapCalling,
    /// Double-yakuman rule label
    RuleDoubleYakuman,
    /// Pei dora rule label
    RuleNukiDora,
    /// Tsumo-loss rule label
    RuleTsumoLoss,
    /// Four-quads abortive-draw rule label
    RuleFourKansDraw,
    /// Four-winds abortive-draw rule label
    RuleFourWindsDraw,
    /// Four-riichi abortive-draw rule label
    RuleFourRiichiDraw,
    /// Nine-terminals abortive-draw rule label
    RuleNineTerminalsDraw,
    /// Yakuman liability-payment rule label
    RuleYakumanPao,
    /// Red-five rule description
    RuleRedFivesDescription,
    /// Dealer-continuation rule description
    RuleDealerContinuationDescription,
    /// South/West extension rule description
    RuleRoundExtensionDescription,
    /// Final-dealer stopping rule description
    RuleAllLastDescription,
    /// Bankruptcy rule description
    RuleBankruptcyDescription,
    /// Cold-end rule description
    RuleColdEndDescription,
    /// Simultaneous-ron rule description
    RuleRonModeDescription,
    /// Abortive-draw mode description
    RuleAbortiveDrawModeDescription,
    /// Nagashi Mangan rule description
    RuleNagashiManganDescription,
    /// Kiriage Mangan rule description
    RuleKiriageManganDescription,
    /// Counted-yakuman rule description
    RuleCountedYakumanDescription,
    /// Open All Inside rule description
    RuleOpenAllInsideDescription,
    /// Swap-calling rule description
    RuleSwapCallingDescription,
    /// Double-yakuman rule description
    RuleDoubleYakumanDescription,
    /// Pei dora rule description
    RuleNukiDoraDescription,
    /// Tsumo-loss rule description
    RuleTsumoLossDescription,
    /// Four-quads abortive-draw rule description
    RuleFourKansDrawDescription,
    /// Four-winds abortive-draw rule description
    RuleFourWindsDrawDescription,
    /// Four-riichi abortive-draw rule description
    RuleFourRiichiDrawDescription,
    /// Nine-terminals abortive-draw rule description
    RuleNineTerminalsDrawDescription,
    /// Yakuman liability-payment rule description
    RuleYakumanPaoDescription,
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
            Key::RuleSettingsButton => match lang {
                Lang::Ja => "ルール設定",
                Lang::En => "Rule Settings",
            },
            Key::RuleSettingsTitle => match lang {
                Lang::Ja => "対戦ルール設定",
                Lang::En => "Game Rules",
            },
            Key::RuleBasicPage => match lang {
                Lang::Ja => "基本設定",
                Lang::En => "Basic",
            },
            Key::RuleAdvancedPage => match lang {
                Lang::Ja => "詳細設定",
                Lang::En => "Advanced",
            },
            Key::CurrentRulesTitle => match lang {
                Lang::Ja => "現在の対戦ルール",
                Lang::En => "Current Rules",
            },
            Key::RuleOn => match lang {
                Lang::Ja => "あり",
                Lang::En => "On",
            },
            Key::RuleOff => match lang {
                Lang::Ja => "なし",
                Lang::En => "Off",
            },
            Key::RuleRedFives => match lang {
                Lang::Ja => "赤ドラ",
                Lang::En => "Red fives",
            },
            Key::RuleDealerContinuation => match lang {
                Lang::Ja => "連荘",
                Lang::En => "Dealer continuation",
            },
            Key::RuleRoundExtension => match lang {
                Lang::Ja => "南入・西入",
                Lang::En => "Round extension",
            },
            Key::RuleAllLast => match lang {
                Lang::Ja => "和了止め・聴牌止め",
                Lang::En => "Final dealer stop",
            },
            Key::RuleBankruptcy => match lang {
                Lang::Ja => "飛び終了",
                Lang::En => "Bankruptcy",
            },
            Key::RuleColdEnd => match lang {
                Lang::Ja => "コールド終了",
                Lang::En => "Cold end",
            },
            Key::RuleRonMode => match lang {
                Lang::Ja => "同時ロン",
                Lang::En => "Simultaneous ron",
            },
            Key::RuleAbortiveDrawMode => match lang {
                Lang::Ja => "途中流局",
                Lang::En => "Abortive draws",
            },
            Key::RuleNagashiMangan => match lang {
                Lang::Ja => "流し満貫",
                Lang::En => "Nagashi Mangan",
            },
            Key::RuleKiriageMangan => match lang {
                Lang::Ja => "切り上げ満貫",
                Lang::En => "Kiriage Mangan",
            },
            Key::RuleCountedYakuman => match lang {
                Lang::Ja => "数え役満",
                Lang::En => "Counted yakuman",
            },
            Key::RuleOpenAllInside => match lang {
                Lang::Ja => "喰いタン",
                Lang::En => "Open All Inside",
            },
            Key::RuleSwapCalling => match lang {
                Lang::Ja => "喰い替え",
                Lang::En => "Swap-calling",
            },
            Key::RuleDoubleYakuman => match lang {
                Lang::Ja => "二倍役満",
                Lang::En => "Double yakuman",
            },
            Key::RuleNukiDora => match lang {
                Lang::Ja => "北抜き",
                Lang::En => "Pei dora",
            },
            Key::RuleTsumoLoss => match lang {
                Lang::Ja => "ツモ損",
                Lang::En => "Tsumo loss",
            },
            Key::RuleFourKansDraw => match lang {
                Lang::Ja => "四槓散了",
                Lang::En => "Four-quads draw",
            },
            Key::RuleFourWindsDraw => match lang {
                Lang::Ja => "四風連打",
                Lang::En => "Four-winds draw",
            },
            Key::RuleFourRiichiDraw => match lang {
                Lang::Ja => "四家立直",
                Lang::En => "Four-riichi draw",
            },
            Key::RuleNineTerminalsDraw => match lang {
                Lang::Ja => "九種九牌",
                Lang::En => "Nine Terminals",
            },
            Key::RuleYakumanPao => match lang {
                Lang::Ja => "責任払い",
                Lang::En => "Liability payment",
            },
            Key::RuleRedFivesDescription => match lang {
                Lang::Ja => "四麻は5萬・5筒・5索、三麻は5筒・5索に\n赤牌を各1枚入れます。",
                Lang::En => "Adds one red five per available suit.\nSanma has red 5p and 5s.",
            },
            Key::RuleDealerContinuationDescription => match lang {
                Lang::Ja => {
                    "親の和了のみ、または親の和了・流局時の聴牌で連荘します。\n途中流局はどちらでも連荘します。"
                }
                Lang::En => {
                    "Continue after dealer wins only, or after dealer wins/tenpai.\nAbortive draws always continue the hand."
                }
            },
            Key::RuleRoundExtensionDescription => match lang {
                Lang::Ja => {
                    "規定局終了時に全員が返し点未満なら、東風戦は南場、\n半荘戦は西場へ入ります。延長は1場までです。"
                }
                Lang::En => {
                    "If nobody reaches the target at the scheduled end, extend\nEast-only into South or hanchan into West, for at most one wind."
                }
            },
            Key::RuleAllLastDescription => match lang {
                Lang::Ja => {
                    "オーラスの親が首位のとき、親の和了または聴牌流局後に、\n連荘せず対局を自動終了する条件を設定します。"
                }
                Lang::En => {
                    "Automatically ends without dealer continuation when the leading\nfinal dealer wins, or is tenpai after an exhaustive draw."
                }
            },
            Key::RuleBankruptcyDescription => match lang {
                Lang::Ja => {
                    "持ち点がマイナス、または0点以下になった時点で\n対局を終了するか設定します。"
                }
                Lang::En => {
                    "Ends the game when a score drops below zero, at zero or below,\nor never ends early for bankruptcy."
                }
            },
            Key::RuleColdEndDescription => match lang {
                Lang::Ja => "四麻で誰かが55,000点以上になった時点で対局を終了します。",
                Lang::En => "Ends a four-player game when anyone reaches 55,000 points.",
            },
            Key::RuleRonModeDescription => match lang {
                Lang::Ja => {
                    "頭ハネ、ダブロン・トリロンあり、または三家和を\n途中流局として扱う方式から選びます。"
                }
                Lang::En => {
                    "Choose head bump, all double/triple ron, or multiple ron\nwith triple ron treated as an abortive draw."
                }
            },
            Key::RuleAbortiveDrawModeDescription => match lang {
                Lang::Ja => {
                    "四槓散了・四風連打・四家立直・九種九牌をまとめて設定します。\n詳細設定では各項目を個別に変更できます。"
                }
                Lang::En => {
                    "Sets the four-quads, four-winds, four-riichi and nine-terminals\ndraws together. Advanced settings can override each one."
                }
            },
            Key::RuleNagashiManganDescription => match lang {
                Lang::Ja => {
                    "流局時、鳴かれていない捨て牌がすべて么九牌なら\n流し満貫として精算します。"
                }
                Lang::En => {
                    "At an exhaustive draw, awards Mangan when every unclaimed\ndiscard is a terminal or honour."
                }
            },
            Key::RuleKiriageManganDescription => match lang {
                Lang::Ja => "3翻60符と4翻30符を満貫として扱います。",
                Lang::En => "Rounds 3 han 60 fu and 4 han 30 fu up to Mangan.",
            },
            Key::RuleCountedYakumanDescription => match lang {
                Lang::Ja => {
                    "役満役を含まない13翻以上の手を役満として扱います。\nなしの場合は三倍満です。"
                }
                Lang::En => {
                    "Scores non-yakuman hands with 13+ han as yakuman.\nWhen off, they are capped at Sanbaiman."
                }
            },
            Key::RuleOpenAllInsideDescription => match lang {
                Lang::Ja => "副露した断么九を1翻役として認めます。",
                Lang::En => "Allows All Inside (Tan'yao) on an open hand.",
            },
            Key::RuleSwapCallingDescription => match lang {
                Lang::Ja => "鳴いた直後に、鳴いた牌や同等牌を捨てることを認めます。",
                Lang::En => "Allows an immediate discard of the called or an equivalent tile.",
            },
            Key::RuleDoubleYakumanDescription => match lang {
                Lang::Ja => {
                    "国士無双十三面待ち、四暗刻単騎待ち、大四喜、\n純正九蓮宝燈を二倍役満にします。"
                }
                Lang::En => {
                    "Thirteen Orphans (13-sided wait), Four Concealed Triplets (pair wait),\nBig Winds, and Pure Nine Gates are worth two yakuman."
                }
            },
            Key::RuleNukiDoraDescription => match lang {
                Lang::Ja => "三人麻雀で北を抜き、1翻のドラとして扱います。",
                Lang::En => "In three-player games, North may be extracted as one dora.",
            },
            Key::RuleTsumoLossDescription => match lang {
                Lang::Ja => {
                    "ありの場合は不在の北家分を受け取らず、\nなしの場合は支払う2人で折半します。"
                }
                Lang::En => {
                    "When on, the absent North player's share is not received;\nwhen off, both payers split it."
                }
            },
            Key::RuleFourKansDrawDescription => match lang {
                Lang::Ja => "複数人による4回目のカン成立時に流局します。",
                Lang::En => "Ends the hand when multiple players complete four quads in total.",
            },
            Key::RuleFourWindsDrawDescription => match lang {
                Lang::Ja => "全員が最初の打牌で同じ風牌を捨てると流局します。",
                Lang::En => "Ends the hand when every first discard is the same wind tile.",
            },
            Key::RuleFourRiichiDrawDescription => match lang {
                Lang::Ja => "全員がリーチを宣言すると流局します。",
                Lang::En => "Ends the hand when every player declares riichi.",
            },
            Key::RuleNineTerminalsDrawDescription => match lang {
                Lang::Ja => "配牌で么九牌が9種以上なら流局を宣言できます。",
                Lang::En => "Allows a draw declaration with nine kinds of terminals and honours.",
            },
            Key::RuleYakumanPaoDescription => match lang {
                Lang::Ja => "大三元・大四喜・四槓子を確定させた打牌に責任払いを適用します。",
                Lang::En => "Applies liability payment when a discard completes certain yakuman.",
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
    fn all_last_rule_text_explains_the_automatic_stop() {
        assert_eq!(Key::RuleAllLast.text(Lang::Ja), "和了止め・聴牌止め");
        assert_eq!(
            Key::RuleAllLastDescription.text(Lang::Ja),
            "オーラスの親が首位のとき、親の和了または聴牌流局後に、\n連荘せず対局を自動終了する条件を設定します。"
        );
        assert!(
            Key::RuleAllLastDescription
                .text(Lang::En)
                .starts_with("Automatically ends")
        );
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

    #[test]
    fn bonus_point_labels() {
        let ja = Translator::new(Lang::Ja);
        let en = Translator::new(Lang::En);
        assert_eq!(ja.deposit_points(1, "1,000"), "＋供託1本　1,000点");
        assert_eq!(ja.honba_points(2, "600"), "＋2本場　600点");
        assert_eq!(en.deposit_points(1, "1,000"), "+deposit 1: 1,000 pts");
        assert_eq!(en.honba_points(2, "600"), "+2 honba: 600 pts");
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
