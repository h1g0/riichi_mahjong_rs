//! ゲーム状態管理
//!
//! サーバから受信したイベントに基づいてクライアント側の状態を管理する。

use macroquad::prelude::*;
use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::settings::Lang;
use mahjong_core::tile::{Tile, TileType, Wind};

use crate::i18n::{Key, Translator};
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::protocol::net::CpuSpec;
use mahjong_server::protocol::{
    AvailableCall, CallType, ClientAction, PlayerHandInfo, ServerEvent,
};

/// 1人分の和了結果（結果画面の1ページ分）
#[derive(Debug, Clone)]
pub struct WinResult {
    pub win_hand: Vec<Tile>,
    pub win_melds: Vec<Meld>,
    pub win_tile: Option<Tile>,
    pub win_is_tsumo: bool,
    pub uradora_indicators: Vec<Tile>,
    pub result_message: String,
    /// 和了者の表示名（例: 「東家」「あなた」）
    pub winner_name: String,
    /// 放銃者の表示名（ツモの場合は None）
    pub loser_name: Option<String>,
    /// 成立した役の一覧（役名, 翻数）
    pub yaku: Vec<(String, u32)>,
    /// 翻数
    pub han: u32,
    /// 符
    pub fu: u32,
    /// 和了点
    pub score_points: i32,
    /// 点数等級名（満貫・跳満など。通常は空）
    pub rank_name: String,
    /// この和了で受け取った供託リーチ棒の本数
    pub riichi_sticks: usize,
}

/// 捨て牌の表示情報
#[derive(Debug, Clone)]
pub struct DiscardInfo {
    pub tile: Tile,
    pub is_tsumogiri: bool,
    /// リーチ宣言牌かどうか（横向きに表示）
    pub is_riichi: bool,
    /// 他家に鳴かれた牌かどうか（薄く表示する）
    pub is_called: bool,
}

/// 他プレイヤーの手牌表示情報（相対インデックスで管理）
#[derive(Debug, Clone)]
pub struct OtherPlayerHand {
    /// 手牌（公開時のみ設定。非公開時は空）
    pub hand: Vec<Tile>,
    /// 副露（鳴き）一覧
    pub melds: Vec<Meld>,
    /// 手牌が公開されているか（和了時・テンパイ時）
    pub revealed: bool,
    /// 非公開時の手牌枚数（裏向き表示用）
    pub concealed_count: usize,
}

impl OtherPlayerHand {
    fn new() -> Self {
        OtherPlayerHand {
            hand: Vec::new(),
            melds: Vec::new(),
            revealed: false,
            concealed_count: 13,
        }
    }
}

/// 各座席のプレイヤー種別（強さ・性格の表示に使う）
#[derive(Debug, Clone)]
pub enum PlayerLabel {
    /// 自分
    Me,
    /// 他の人間プレイヤー（オンライン対戦の相手）
    Human(String),
    /// CPU（強さ・性格つき）
    Cpu { level: String, personality: String },
}

/// CPU の強さ（英語の内部名）を表示言語へ変換する。
fn localize_cpu_level(level: &str, lang: Lang) -> &'static str {
    let idx = match level {
        "Weak" => 0,
        "Strong" => 2,
        _ => 1,
    };
    Translator::new(lang).strength_label(idx)
}

/// CPU の性格（英語の内部名）を表示言語へ変換する。
fn localize_cpu_personality(personality: &str, lang: Lang) -> &'static str {
    let idx = match personality {
        "Speedy" => 1,
        "HighValue" => 2,
        "Defensive" => 3,
        _ => 0,
    };
    Translator::new(lang).personality_label(idx)
}

impl PlayerLabel {
    /// 風・得点の下に表示する補助テキスト（自分は非表示）。
    /// CPU は「CPU{n}（強さ・性格）」、人間プレイヤーは名前を返す。
    pub fn detail(&self, cpu_number: usize, lang: Lang) -> Option<String> {
        match self {
            PlayerLabel::Me => None,
            PlayerLabel::Human(name) => Some(name.clone()),
            PlayerLabel::Cpu { level, personality } => {
                Some(cpu_display(cpu_number, level, personality, lang))
            }
        }
    }

    /// 順位表などで使う表示名。CPU は「CPU{n}（強さ・性格）」。
    pub fn name(&self, cpu_number: usize, lang: Lang) -> String {
        match self {
            PlayerLabel::Me => Key::You.text(lang).to_string(),
            PlayerLabel::Human(name) => name.clone(),
            PlayerLabel::Cpu { level, personality } => {
                cpu_display(cpu_number, level, personality, lang)
            }
        }
    }
}

/// CPU の表示名（例: 日「CPU1（普通・バランス）」/ 英「CPU1 (Normal, Balanced)」）。
fn cpu_display(cpu_number: usize, level: &str, personality: &str, lang: Lang) -> String {
    let lv = localize_cpu_level(level, lang);
    let ps = localize_cpu_personality(personality, lang);
    match lang {
        Lang::Ja => format!("CPU{cpu_number}（{lv}・{ps}）"),
        Lang::En => format!("CPU{cpu_number} ({lv}, {ps})"),
    }
}

/// CPU設定から CPU 用の [`PlayerLabel`] を作る
fn cpu_label(config: &CpuConfig) -> PlayerLabel {
    PlayerLabel::Cpu {
        level: config.level.display_name().to_string(),
        personality: config.personality.display_name().to_string(),
    }
}

/// クライアント側のゲーム状態
pub struct GameState {
    /// 自分の座席の風
    pub seat_wind: Option<Wind>,
    /// 自分の手牌
    pub hand: Vec<Tile>,
    /// ツモ牌（直近にツモった牌）
    pub drawn: Option<Tile>,
    /// 各プレイヤーの捨て牌（自分=0, 下家=1, 対面=2, 上家=3）
    pub discards: [Vec<DiscardInfo>; 4],
    /// 各プレイヤーの点数
    pub scores: [i32; 4],
    /// 場風
    pub round_wind: Option<Wind>,
    /// ドラ表示牌
    pub dora_indicators: Vec<Tile>,
    /// 裏ドラ表示牌（リーチ和了時のみ公開）
    pub uradora_indicators: Vec<Tile>,
    /// 和了時の手牌情報（結果画面表示用）
    pub win_hand: Vec<Tile>,
    /// 和了時の副露
    pub win_melds: Vec<Meld>,
    /// 和了牌
    pub win_tile: Option<Tile>,
    /// ツモ和了かロン和了か（true=ツモ）
    pub win_is_tsumo: bool,
    /// 山の残り枚数
    pub remaining_tiles: usize,
    /// 選択中の牌のインデックス
    pub selected_tile: Option<usize>,
    /// ツモ牌が選択中か
    pub selected_drawn: bool,
    /// ツモ和了可能か
    pub can_tsumo: bool,
    /// リーチ宣言可能か
    pub can_riichi: bool,
    /// 自分の手番で暗カン可能な牌
    pub self_kan_options: Vec<Tile>,
    /// 自分がリーチ中か
    pub is_riichi: bool,
    /// リーチ宣言のための打牌選択中か
    pub riichi_selection_mode: bool,
    /// リーチ可能な手牌インデックス
    pub riichi_selectable_tiles: Vec<usize>,
    /// ツモ牌切りでリーチ可能か
    pub riichi_selectable_drawn: bool,
    /// 喰い替え禁止により、鳴き直後の打牌で捨てられない牌種
    /// （チー・ポン直後にのみ設定され、打牌・ツモで解除される）
    pub forbidden_discards: Vec<TileType>,
    /// 喰い替え禁止牌を選択しようとしたか（「喰い替えです！」警告の表示用）
    pub selected_forbidden_swap: bool,
    /// 局の結果メッセージ
    pub result_message: Option<String>,
    /// 和了結果一覧（ダブロン・トリロン時は複数）
    pub win_results: Vec<WinResult>,
    /// 現在表示中の和了結果インデックス
    pub win_result_index: usize,
    /// 自分の手番か
    pub is_my_turn: bool,
    /// ゲームフェーズ
    pub phase: GamePhase,
    /// 鳴き可能な選択肢
    pub available_calls: Vec<AvailableCall>,
    /// 鳴き対象の牌
    pub call_target_tile: Option<Tile>,
    /// 鳴き対象の捨てたプレイヤー
    pub call_discarder: Option<Wind>,
    /// 自分の副露（鳴き）一覧
    pub melds: Vec<Meld>,
    /// 局番号（0=東1局, 1=東2局, ...）
    pub round_number: usize,
    /// 本場数
    pub honba: usize,
    /// 場に出ている供託リーチ棒の本数
    pub riichi_sticks: usize,
    /// フリテン状態か
    pub is_furiten: bool,
    /// 選択中の牌を捨てるとフリテンになるか
    pub selected_would_cause_furiten: bool,
    /// 他プレイヤーの手牌情報（下家=0, 対面=1, 上家=2）
    pub other_players: [OtherPlayerHand; 3],
    /// リーチ宣言済みで次の打牌がリーチ宣言牌となるプレイヤーの風（一時フラグ）
    pending_riichi_player: Option<Wind>,
    /// 直前に捨て牌したプレイヤーの風（鳴き元の判定に使用）
    last_discarder: Option<Wind>,
    /// チーの組み合わせ選択UI表示中か（複数の選択肢がある場合）
    pub chi_option_selecting: bool,
    /// チー選択UIに表示する選択肢（手牌から使う2枚の牌）
    pub chi_pending_options: Vec<[Tile; 2]>,
    /// ポンの組み合わせ選択UI表示中か（赤ドラの有無で選択肢が分かれる場合）
    pub pon_option_selecting: bool,
    /// ポン選択UIに表示する選択肢（手牌から使う2枚の牌）
    pub pon_pending_options: Vec<[Tile; 2]>,
    /// 九種九牌の宣言選択中か
    pub nine_terminals_pending: bool,
    /// 対局開始前設定
    pub setup_state: SetupState,
    /// オンライン対戦UIの状態
    pub online_state: OnlineUiState,
    /// 各座席のプレイヤー種別（座席インデックス順 = scores と同じ並び）
    pub player_labels: [PlayerLabel; 4],
    /// 自分の座席インデックス（ローカルは常に0、オンラインは your_seat）
    pub my_seat: usize,
    /// 表示言語
    pub lang: Lang,
}

/// オンライン対戦UI（メニュー・ロビー）の状態
#[derive(Debug, Clone)]
pub struct OnlineUiState {
    /// 表示名の入力欄
    pub name_input: String,
    /// ルームコードの入力欄
    pub code_input: String,
    /// true ならルームコード欄、false なら名前欄にフォーカス
    pub code_focused: bool,
    /// 接続状況・エラーの表示文言
    pub status_line: Option<String>,
    /// status_line がエラーか（赤色で表示する）
    pub status_is_error: bool,
    /// 入室中のルーム表示（メインループがアダプターからコピーする）
    pub room: Option<RoomViewUi>,
    /// 手番の制限時間の残り秒数（オンラインで自分の手番のときのみ Some）
    pub turn_remaining: Option<u32>,
}

impl OnlineUiState {
    pub fn new() -> Self {
        OnlineUiState {
            // 既定の表示名は送信時に display_name() が言語に応じて補う
            name_input: String::new(),
            code_input: String::new(),
            code_focused: false,
            status_line: None,
            status_is_error: false,
            room: None,
            turn_remaining: None,
        }
    }
}

/// ロビー画面に表示するルーム情報
#[derive(Debug, Clone)]
pub struct RoomViewUi {
    /// ルームコード
    pub code: String,
    /// 各座席の表示文言（東南西北の順）
    pub seat_labels: [String; 4],
    /// 自分がホストか（対局開始ボタンの表示に使う）
    pub is_host: bool,
}

/// 対局開始前の設定画面の状態
#[derive(Debug, Clone)]
pub struct SetupState {
    /// 各CPUの強さ設定（下家, 対面, 上家）
    pub cpu_levels: [usize; 3],
    /// 各CPUの性格設定（下家, 対面, 上家）
    pub cpu_personalities: [usize; 3],
}

impl SetupState {
    pub fn new() -> Self {
        SetupState {
            cpu_levels: [1, 1, 1],        // 全員 Normal
            cpu_personalities: [0, 1, 2], // Balanced, Speedy, HighValue
        }
    }

    pub fn level_count() -> usize {
        3
    }
    pub fn personality_count() -> usize {
        4
    }

    /// 設定からCpuConfigの配列を生成する
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

    /// 設定から CPU 指定（オンライン対戦でホストが送る）を生成する
    pub fn build_cpu_specs(&self) -> [CpuSpec; 3] {
        self.build_configs()
            .map(|config| CpuSpec::from_config(&config))
    }
}

/// ゲームフェーズ
#[derive(Debug, Clone, PartialEq)]
pub enum GamePhase {
    /// 対局開始前の設定画面
    Setup,
    /// オンライン対戦メニュー（名前・ルームコード入力）
    OnlineMenu,
    /// オンラインロビー（メンバー待ち）
    OnlineLobby,
    /// ゲーム開始前
    WaitingForStart,
    /// 対局中
    Playing,
    /// 局終了（結果表示中）
    RoundResult,
    /// ゲーム終了
    GameOver,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            seat_wind: None,
            hand: Vec::new(),
            drawn: None,
            discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            scores: [25000; 4],
            round_wind: None,
            dora_indicators: Vec::new(),
            uradora_indicators: Vec::new(),
            win_hand: Vec::new(),
            win_melds: Vec::new(),
            win_tile: None,
            win_is_tsumo: false,
            remaining_tiles: 70,
            selected_tile: None,
            selected_drawn: false,
            can_tsumo: false,
            can_riichi: false,
            self_kan_options: Vec::new(),
            is_riichi: false,
            riichi_selection_mode: false,
            riichi_selectable_tiles: Vec::new(),
            riichi_selectable_drawn: false,
            forbidden_discards: Vec::new(),
            selected_forbidden_swap: false,
            result_message: None,
            win_results: Vec::new(),
            win_result_index: 0,
            is_my_turn: false,
            phase: GamePhase::Setup,
            available_calls: Vec::new(),
            call_target_tile: None,
            call_discarder: None,
            melds: Vec::new(),
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
            is_furiten: false,
            selected_would_cause_furiten: false,
            other_players: [
                OtherPlayerHand::new(),
                OtherPlayerHand::new(),
                OtherPlayerHand::new(),
            ],
            pending_riichi_player: None,
            last_discarder: None,
            chi_option_selecting: false,
            chi_pending_options: Vec::new(),
            pon_option_selecting: false,
            pon_pending_options: Vec::new(),
            nine_terminals_pending: false,
            setup_state: SetupState::new(),
            online_state: OnlineUiState::new(),
            player_labels: [
                PlayerLabel::Me,
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "Balanced".to_string(),
                },
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "Speedy".to_string(),
                },
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "HighValue".to_string(),
                },
            ],
            my_seat: 0,
            // 保存された表示言語を読み込む（未保存なら日本語）。
            // 「もう一度」などで new() が再生成されても選択を保つ。
            lang: crate::persistence::load_lang().unwrap_or(Lang::Ja),
        }
    }

    /// 現在の表示言語の [`Translator`](crate::i18n::Translator) を返す。
    pub fn tr(&self) -> crate::i18n::Translator {
        crate::i18n::Translator::new(self.lang)
    }

    /// ローカル対局のプレイヤー種別を設定する（自分=座席0, CPU=座席1〜3）
    pub fn set_local_players(&mut self, cpu_configs: &[CpuConfig; 3]) {
        self.my_seat = 0;
        self.player_labels = [
            PlayerLabel::Me,
            cpu_label(&cpu_configs[0]),
            cpu_label(&cpu_configs[1]),
            cpu_label(&cpu_configs[2]),
        ];
    }

    /// オンライン対局のプレイヤー種別を設定する
    ///
    /// `seats` は座席インデックス順、`your_seat` は自分の座席。
    pub fn set_online_players(&mut self, seats: &[PlayerLabel; 4], your_seat: usize) {
        self.my_seat = your_seat;
        self.player_labels = seats.clone();
    }

    /// サーバイベントを処理する
    pub fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                round_wind,
                dora_indicators,
                round_number,
                total_rounds: _,
                honba,
                riichi_sticks,
            } => {
                self.seat_wind = Some(seat_wind);
                self.hand = hand;
                self.hand.sort();
                self.drawn = None;
                self.scores = scores;
                self.round_wind = Some(round_wind);
                self.dora_indicators = dora_indicators;
                self.uradora_indicators = Vec::new();
                self.discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.pending_riichi_player = None;
                self.result_message = None;
                self.win_results.clear();
                self.win_result_index = 0;
                self.phase = GamePhase::Playing;
                self.available_calls.clear();
                self.chi_option_selecting = false;
                self.chi_pending_options.clear();
                self.pon_option_selecting = false;
                self.pon_pending_options.clear();
                self.nine_terminals_pending = false;
                self.call_target_tile = None;
                self.call_discarder = None;
                self.can_tsumo = false;
                self.can_riichi = false;
                self.self_kan_options.clear();
                self.is_riichi = false;
                self.clear_riichi_selection();
                self.melds.clear();
                self.round_number = round_number;
                self.honba = honba;
                self.riichi_sticks = riichi_sticks;
                self.is_furiten = false;
                self.selected_would_cause_furiten = false;
                self.other_players = [
                    OtherPlayerHand::new(),
                    OtherPlayerHand::new(),
                    OtherPlayerHand::new(),
                ];
                self.last_discarder = None;
            }

            ServerEvent::TileDrawn {
                tile,
                remaining_tiles,
                can_tsumo,
                can_riichi,
                is_furiten,
            } => {
                self.drawn = Some(tile);
                self.remaining_tiles = remaining_tiles;
                self.is_my_turn = true;
                self.can_tsumo = can_tsumo;
                self.can_riichi = can_riichi;
                self.is_furiten = is_furiten;
                self.selected_would_cause_furiten = false;
                self.clear_riichi_selection();
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();
                // ツモ後は喰い替え制限が解除される
                self.forbidden_discards.clear();
                self.selected_forbidden_swap = false;
            }

            ServerEvent::NineTerminalsAvailable => {
                self.nine_terminals_pending = true;
            }

            ServerEvent::OtherPlayerDrew {
                player,
                remaining_tiles,
            } => {
                self.remaining_tiles = remaining_tiles;
                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    self.other_players[relative_idx - 1].concealed_count += 1;
                }
            }

            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
            } => {
                self.last_discarder = Some(player);
                // 新しい打牌が出たら、過去の鳴き打診で残った call_discarder を捨てる。
                // （パスして鳴かなかった場合に古い値が残り、次の鳴き元判定を誤らせていた）
                self.call_discarder = None;
                let relative_idx = self.relative_player_index(player);
                let is_riichi = self.pending_riichi_player == Some(player);
                if is_riichi {
                    self.pending_riichi_player = None;
                }
                self.discards[relative_idx].push(DiscardInfo {
                    tile,
                    is_tsumogiri,
                    is_riichi,
                    is_called: false,
                });

                // 他プレイヤーが捨てた場合、隠し手牌の枚数を更新
                if relative_idx > 0 {
                    let other_idx = relative_idx - 1;
                    self.other_players[other_idx].concealed_count = self.other_players[other_idx]
                        .concealed_count
                        .saturating_sub(1);
                }

                // 自分が捨てた場合
                if Some(player) == self.seat_wind {
                    self.is_my_turn = false;
                    self.drawn = None;
                    self.selected_tile = None;
                    self.selected_drawn = false;
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
                    // 打牌が完了したので喰い替え制限を解除する
                    self.forbidden_discards.clear();
                    self.selected_forbidden_swap = false;
                }
            }

            ServerEvent::CallAvailable {
                tile,
                discarder,
                calls,
            } => {
                self.available_calls = calls;
                self.call_target_tile = Some(tile);
                self.call_discarder = Some(discarder);
            }

            ServerEvent::PlayerCalled {
                player,
                call_type,
                called_tile,
                tiles,
            } => {
                // 鳴き選択肢をクリア
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();

                // CallType → MeldType 変換
                let category = Self::call_type_to_meld_type(&call_type);

                // 鳴き元の判定
                let meld_from = match call_type {
                    CallType::Ankan => MeldFrom::Myself,
                    CallType::Kakan => MeldFrom::Myself,
                    _ => {
                        if let Some(discarder) = self.call_discarder.or(self.last_discarder) {
                            Self::compute_meld_direction(player, discarder)
                        } else {
                            MeldFrom::Previous
                        }
                    }
                };

                // 鳴かれた牌（ポン・チー・大明槓）を河で薄く表示するためマークする。
                // 取られた牌は鳴いた側の手番直前に捨てられた、放銃元の河の最後の該当牌。
                if matches!(
                    call_type,
                    CallType::Pon | CallType::Chi | CallType::Daiminkan
                ) && let Some(discarder) = self.call_discarder.or(self.last_discarder)
                {
                    let discarder_idx = self.relative_player_index(discarder);
                    if let Some(discard) = self.discards[discarder_idx]
                        .iter_mut()
                        .rev()
                        .find(|d| d.tile == called_tile && !d.is_called)
                    {
                        discard.is_called = true;
                    }
                }

                self.call_discarder = None;

                // 他プレイヤーが鳴いた場合、副露情報を記録
                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    let other_idx = relative_idx - 1;
                    let other = &mut self.other_players[other_idx];
                    match call_type {
                        CallType::Ron => {}
                        CallType::Kakan => {
                            if let Some(meld) = other.melds.iter_mut().find(|m| {
                                m.category == MeldType::Pon
                                    && m.tiles.first().map(|t| t.get())
                                        == tiles.first().map(|t| t.get())
                            }) {
                                meld.category = MeldType::Kakan;
                                meld.tiles = tiles.clone();
                                // from はポン時のままにする
                                other.concealed_count = other.concealed_count.saturating_sub(1);
                            } else {
                                other.melds.push(Meld {
                                    category,
                                    tiles: tiles.clone(),
                                    from: meld_from,
                                    called_tile: Some(called_tile),
                                });
                                other.concealed_count = other.concealed_count.saturating_sub(1);
                            }
                        }
                        CallType::Ankan => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: MeldFrom::Myself,
                                called_tile: None,
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(3);
                        }
                        CallType::Pon | CallType::Chi => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(2);
                        }
                        CallType::Daiminkan => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(3);
                        }
                    }
                }

                // 自分が鳴いた場合、副露情報を保存し打牌待ちへ
                if Some(player) == self.seat_wind {
                    match call_type {
                        CallType::Ron => {}
                        CallType::Pon | CallType::Chi | CallType::Daiminkan => {
                            self.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                            // チー・ポン直後の打牌では喰い替え牌を捨てられない。
                            // （大明槓は嶺上ツモになるため対象外）
                            self.forbidden_discards = match call_type {
                                CallType::Pon | CallType::Chi => self
                                    .melds
                                    .last()
                                    .map(|meld| meld.forbidden_swap_tiles())
                                    .unwrap_or_default(),
                                _ => Vec::new(),
                            };
                        }
                        CallType::Ankan => {
                            self.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: MeldFrom::Myself,
                                called_tile: None,
                            });
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                        }
                        CallType::Kakan => {
                            if let Some(meld) = self.melds.iter_mut().find(|meld| {
                                meld.category == MeldType::Pon
                                    && meld.tiles.first().map(|tile| tile.get())
                                        == tiles.first().map(|tile| tile.get())
                            }) {
                                meld.category = MeldType::Kakan;
                                meld.tiles = tiles.clone();
                            } else {
                                self.melds.push(Meld {
                                    category,
                                    tiles: tiles.clone(),
                                    from: meld_from,
                                    called_tile: Some(called_tile),
                                });
                            }
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                        }
                    }
                }
            }

            ServerEvent::DoraIndicatorsUpdated { dora_indicators } => {
                self.dora_indicators = dora_indicators;
            }

            ServerEvent::PlayerRiichi {
                player,
                scores,
                riichi_sticks,
            } => {
                self.scores = scores;
                self.riichi_sticks = riichi_sticks;

                // 次の打牌をリーチ宣言牌としてマーク
                self.pending_riichi_player = Some(player);

                // 自分がリーチした場合
                if Some(player) == self.seat_wind {
                    self.is_riichi = true;
                    self.can_riichi = false;
                    self.clear_riichi_selection();
                }
            }

            ServerEvent::HandUpdated { hand } => {
                self.hand = hand;
                self.hand.sort();
                self.refresh_self_kan_options();
            }

            ServerEvent::RoundWon {
                winner,
                loser,
                winning_tile,
                scores,
                yaku_list,
                han,
                fu,
                score_points,
                rank,
                has_opened,
                uradora_indicators,
                riichi_sticks,
                player_hands,
            } => {
                self.scores = scores;
                self.riichi_sticks = 0;

                // 手牌情報を取得
                let (win_hand, win_melds) =
                    if let Some(info) = player_hands.iter().find(|p| p.wind == winner) {
                        let hand = info.hand.clone();
                        let relative_idx = self.relative_player_index(winner);
                        let melds = if relative_idx == 0 {
                            self.melds.clone()
                        } else {
                            self.other_players[relative_idx - 1].melds.clone()
                        };
                        (hand, melds)
                    } else {
                        (Vec::new(), Vec::new())
                    };

                self.update_other_player_hands_on_win(&player_hands, winner);

                let lang = self.lang;
                let tr = Translator::new(lang);
                let winner_is_me = self.relative_player_index(winner) == 0;
                let winner_name = if winner_is_me {
                    Key::You.text(lang).to_string()
                } else {
                    self.wind_to_name(winner)
                };
                let loser_name = loser.map(|l| {
                    if self.relative_player_index(l) == 0 {
                        Key::You.text(lang).to_string()
                    } else {
                        self.wind_to_name(l)
                    }
                });
                let win_type = if loser.is_some() {
                    tr.get(Key::Ron)
                } else {
                    tr.get(Key::Tsumo)
                };
                let loser_text = if let Some(l) = loser {
                    tr.dealt_in_by(&self.wind_to_name(l))
                } else {
                    String::new()
                };

                // 構造化された役・ドラ・等級を表示言語へ解決する。
                let yaku: Vec<(String, u32)> = yaku_list
                    .iter()
                    .map(|(item, y_han)| (item.name(has_opened, lang).to_string(), *y_han))
                    .collect();
                let rank_name = rank.name(lang).to_string();

                let mut yaku_text = String::new();
                for (name, y_han) in &yaku {
                    if !yaku_text.is_empty() {
                        yaku_text.push_str("  ");
                    }
                    yaku_text.push_str(&format!("{} {}", name, tr.han(*y_han)));
                }

                let rank_display = if rank_name.is_empty() {
                    tr.han_fu(han, fu)
                } else {
                    format!("{} {}", tr.han_fu(han, fu), rank_name)
                };

                let riichi_sticks_text = if riichi_sticks == 0 {
                    String::new()
                } else {
                    format!("\n{}", tr.deposit_line(riichi_sticks))
                };

                let msg = format!(
                    "{}{}{}\n{}\n{} → {}",
                    tr.win_headline(&winner_name, win_type),
                    loser_text,
                    riichi_sticks_text,
                    yaku_text,
                    rank_display,
                    tr.points(&score_points.to_string())
                );

                self.win_results.push(WinResult {
                    win_hand,
                    win_melds,
                    win_tile: Some(winning_tile),
                    win_is_tsumo: loser.is_none(),
                    uradora_indicators,
                    result_message: msg,
                    winner_name,
                    loser_name,
                    yaku,
                    han,
                    fu,
                    score_points,
                    rank_name,
                    riichi_sticks,
                });

                // 最初のRoundWonでフェーズ遷移・表示を初期化
                if self.phase != GamePhase::RoundResult {
                    self.win_result_index = 0;
                    self.apply_current_win_result();
                    self.phase = GamePhase::RoundResult;
                    self.is_my_turn = false;
                    self.available_calls.clear();
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
                }
            }

            ServerEvent::RoundDraw {
                scores,
                reason,
                tenpai,
                riichi_sticks,
                player_hands,
                declarer,
            } => {
                self.scores = scores;
                self.riichi_sticks = riichi_sticks;
                self.update_other_player_hands_on_draw(&player_hands, &tenpai, declarer);
                let tr = Translator::new(self.lang);
                let mut msg = tr.draw_headline(reason);

                if !tenpai.is_empty() {
                    let tenpai_names: Vec<String> =
                        tenpai.iter().map(|w| self.wind_to_name(*w)).collect();
                    msg.push('\n');
                    msg.push_str(&tr.tenpai_list(&tenpai_names.join(", ")));
                }
                if riichi_sticks > 0 {
                    msg.push('\n');
                    msg.push_str(&tr.deposit_line(riichi_sticks));
                }

                self.win_hand.clear();
                self.win_tile = None;
                self.win_melds.clear();
                self.uradora_indicators.clear();
                self.result_message = Some(msg);
                self.phase = GamePhase::RoundResult;
                self.is_my_turn = false;
                self.available_calls.clear();
                self.clear_riichi_selection();
                self.self_kan_options.clear();
            }
        }
    }

    /// 現在表示中の和了結果ページを返す（流局時は None）。
    pub fn current_win_result(&self) -> Option<&WinResult> {
        self.win_results.get(self.win_result_index)
    }

    /// 現在の win_result_index が指すページを GameState の表示用フィールドに反映する
    fn apply_current_win_result(&mut self) {
        if let Some(wr) = self.win_results.get(self.win_result_index) {
            let wr = wr.clone();
            self.win_hand = wr.win_hand;
            self.win_melds = wr.win_melds;
            self.win_tile = wr.win_tile;
            self.win_is_tsumo = wr.win_is_tsumo;
            self.uradora_indicators = wr.uradora_indicators;
            self.result_message = Some(wr.result_message);
        }
    }

    /// 次の和了結果ページへ進む
    ///
    /// 次のページがある場合: 表示を更新して true を返す
    /// 最後のページだった場合: false を返す（呼び出し元が next_round() を呼ぶ）
    pub fn advance_win_result(&mut self) -> bool {
        let next = self.win_result_index + 1;
        if next < self.win_results.len() {
            self.win_result_index = next;
            self.apply_current_win_result();
            true
        } else {
            false
        }
    }

    /// 和了時に他プレイヤーの手牌を更新する（和了者の手牌を公開）
    fn update_other_player_hands_on_win(&mut self, player_hands: &[PlayerHandInfo], winner: Wind) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue; // 自分はスキップ
            }
            let other = &mut self.other_players[relative_idx - 1];
            // 副露を更新（既存の from 情報を保持）
            if other.melds.is_empty() {
                other.melds = info
                    .melds
                    .iter()
                    .map(|m| Meld {
                        category: Self::call_type_to_meld_type(&m.call_type),
                        tiles: m.tiles.clone(),
                        from: MeldFrom::Unknown, // フォールバック
                        called_tile: None,
                    })
                    .collect();
            }
            // 和了者の手牌を公開
            if info.wind == winner {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }

    /// 流局時に他プレイヤーの手牌を更新する（テンパイ者・九種九牌宣言者の手牌を公開）
    fn update_other_player_hands_on_draw(
        &mut self,
        player_hands: &[PlayerHandInfo],
        tenpai: &[Wind],
        declarer: Option<Wind>,
    ) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue; // 自分はスキップ
            }
            let other = &mut self.other_players[relative_idx - 1];
            // 副露を更新（既存の from 情報を保持）
            if other.melds.is_empty() {
                other.melds = info
                    .melds
                    .iter()
                    .map(|m| Meld {
                        category: Self::call_type_to_meld_type(&m.call_type),
                        tiles: m.tiles.clone(),
                        from: MeldFrom::Unknown, // フォールバック
                        called_tile: None,
                    })
                    .collect();
            }
            // テンパイ者または九種九牌宣言者の手牌を公開
            if tenpai.contains(&info.wind) || declarer == Some(info.wind) {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }

    fn clear_riichi_selection(&mut self) {
        self.riichi_selection_mode = false;
        self.riichi_selectable_tiles.clear();
        self.riichi_selectable_drawn = false;
        self.selected_tile = None;
        self.selected_drawn = false;
    }

    fn can_discard_for_riichi(&self, tile: Option<Tile>) -> bool {
        if self.drawn.is_none() {
            return false;
        }

        let mut hand =
            Hand::new_with_melds(self.hand.clone(), self.melds_for_analysis(), self.drawn);
        match tile {
            Some(target) => {
                let drawn = hand.drawn();
                let tiles = hand.tiles_mut();
                let Some(idx) = tiles.iter().position(|t| *t == target) else {
                    return false;
                };
                tiles.remove(idx);
                if let Some(drawn_tile) = drawn {
                    tiles.push(drawn_tile);
                    tiles.sort();
                }
                hand.set_drawn(None);
            }
            None => {
                hand.set_drawn(None);
            }
        }

        match HandAnalyzer::new(&hand) {
            Ok(analyzer) => analyzer.shanten.is_ready(),
            Err(_) => false,
        }
    }

    fn melds_for_analysis(&self) -> Vec<Meld> {
        self.melds
            .iter()
            .map(|meld| {
                let mut m = meld.clone();
                // HandAnalyzer は3枚で解析するため、カンの場合は3枚に切り詰める
                if m.category.is_kan() && m.tiles.len() > 3 {
                    m.tiles.truncate(3);
                }
                m
            })
            .collect()
    }

    fn enter_riichi_selection(&mut self) {
        self.riichi_selection_mode = true;
        self.selected_tile = None;
        self.selected_drawn = false;
        self.riichi_selectable_tiles = self
            .hand
            .iter()
            .enumerate()
            .filter_map(|(idx, &tile)| self.can_discard_for_riichi(Some(tile)).then_some(idx))
            .collect();
        self.riichi_selectable_drawn = self.can_discard_for_riichi(None);
    }

    /// 指定の牌を捨てた場合にフリテンになるかを判定する
    ///
    /// 捨てた後の手牌がテンパイで、待ち牌が自分の捨て牌に含まれていればフリテン。
    /// tile: Some(牌) = 手牌から捨てる, None = ツモ切り
    fn would_discard_cause_furiten(&self, tile: Option<Tile>) -> bool {
        let mut hand_tiles = self.hand.clone();
        match tile {
            Some(target) => {
                let Some(idx) = hand_tiles.iter().position(|t| *t == target) else {
                    return false;
                };
                hand_tiles.remove(idx);
                if let Some(drawn) = self.drawn {
                    hand_tiles.push(drawn);
                    hand_tiles.sort();
                }
            }
            None => {
                // ツモ切り: drawnを使わない
            }
        }

        // 手牌13枚でテンパイか確認
        let hand = Hand::new_with_melds(hand_tiles, self.melds_for_analysis(), None);
        let analyzer = match HandAnalyzer::new(&hand) {
            Ok(a) => a,
            Err(_) => return false,
        };
        if !analyzer.shanten.is_ready() {
            return false;
        }

        // 待ち牌を求める
        let mut waiting: Vec<TileType> = Vec::new();
        for tile_type in 0..Tile::LEN as u32 {
            let mut test_hand = hand.clone();
            test_hand.set_drawn(Some(Tile::new(tile_type)));
            if let Ok(a) = HandAnalyzer::new(&test_hand)
                && a.shanten.has_won()
            {
                waiting.push(tile_type);
            }
        }

        if waiting.is_empty() {
            return false;
        }

        // 待ち牌が自分の捨て牌に含まれていればフリテン
        let my_discards = &self.discards[0];
        for &wt in &waiting {
            if my_discards.iter().any(|d| d.tile.get() == wt) {
                return true;
            }
        }
        // 捨てようとしている牌自体も捨て牌に加わるので、それも含めて判定
        let discard_tile_type = match tile {
            Some(t) => t.get(),
            None => match self.drawn {
                Some(d) => d.get(),
                None => return false,
            },
        };
        for &wt in &waiting {
            if wt == discard_tile_type {
                return true;
            }
        }
        false
    }

    fn refresh_self_kan_options(&mut self) {
        self.self_kan_options.clear();
        if self.drawn.is_none() || self.is_riichi {
            return;
        }

        let mut counts = [0u8; Tile::LEN];
        for tile in &self.hand {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.drawn {
            counts[drawn.get() as usize] += 1;
        }

        for (idx, count) in counts.iter().enumerate() {
            if *count == 4 {
                self.self_kan_options.push(Tile::new(idx as u32));
                continue;
            }

            let has_pon = self.melds.iter().any(|meld| {
                meld.category == MeldType::Pon
                    && meld.tiles.first().map(|tile| tile.get()) == Some(idx as u32)
            });
            if has_pon && *count >= 1 {
                self.self_kan_options.push(Tile::new(idx as u32));
            }
        }
    }

    fn apply_local_discard_from_hand(&mut self, idx: usize) -> Tile {
        let discarded_tile = self.hand[idx];
        self.selected_tile = None;
        self.selected_drawn = false;
        if let Some(drawn_tile) = self.drawn.take() {
            self.hand.remove(idx);
            self.hand.push(drawn_tile);
            self.hand.sort();
        } else {
            self.hand.remove(idx);
        }
        discarded_tile
    }

    /// 入力処理: オーバーレイのクリック結果と手牌クリックを処理してアクションを返す
    pub fn handle_input(
        &mut self,
        overlay_click: Option<crate::renderer::OverlayClick>,
    ) -> Option<ClientAction> {
        use crate::renderer::OverlayClick;

        if self.phase != GamePhase::Playing {
            return None;
        }

        // リーチ中はツモ切り自動処理（マウス入力不要）
        if self.is_my_turn && self.is_riichi && self.drawn.is_some() && !self.can_tsumo {
            self.drawn.take();
            return Some(ClientAction::Discard { tile: None });
        }

        // オーバーレイのクリック判定（draw_game が返した結果を処理）
        if let Some(click) = overlay_click {
            if self.nine_terminals_pending {
                match click {
                    OverlayClick::NineTerminalsDeclare => {
                        self.nine_terminals_pending = false;
                        return Some(ClientAction::NineTerminals { declare: true });
                    }
                    OverlayClick::NineTerminalsPass => {
                        self.nine_terminals_pending = false;
                        return Some(ClientAction::NineTerminals { declare: false });
                    }
                    _ => {}
                }
                return None;
            }

            if self.chi_option_selecting {
                match click {
                    OverlayClick::Action(action) => {
                        self.chi_option_selecting = false;
                        self.chi_pending_options.clear();
                        self.available_calls.clear();
                        self.call_target_tile = None;
                        return Some(action);
                    }
                    OverlayClick::CancelMeldSelection => {
                        self.chi_option_selecting = false;
                        self.chi_pending_options.clear();
                    }
                    _ => {}
                }
                return None;
            }

            if self.pon_option_selecting {
                match click {
                    OverlayClick::Action(action) => {
                        self.pon_option_selecting = false;
                        self.pon_pending_options.clear();
                        self.available_calls.clear();
                        self.call_target_tile = None;
                        return Some(action);
                    }
                    OverlayClick::CancelMeldSelection => {
                        self.pon_option_selecting = false;
                        self.pon_pending_options.clear();
                    }
                    _ => {}
                }
                return None;
            }

            if !self.available_calls.is_empty() {
                match click {
                    OverlayClick::Action(action) => {
                        self.available_calls.clear();
                        self.call_target_tile = None;
                        return Some(action);
                    }
                    OverlayClick::ShowChiSelection { options } => {
                        self.chi_pending_options = options;
                        self.chi_option_selecting = true;
                    }
                    OverlayClick::ShowPonSelection { options } => {
                        self.pon_pending_options = options;
                        self.pon_option_selecting = true;
                    }
                    _ => {}
                }
                return None;
            }

            // 自分のターン：ツモ・リーチ・暗カン
            match click {
                OverlayClick::Action(action) => return Some(action),
                OverlayClick::ToggleRiichi => {
                    if self.riichi_selection_mode {
                        self.clear_riichi_selection();
                    } else {
                        self.enter_riichi_selection();
                    }
                    return None;
                }
                _ => {}
            }
        }

        // オーバーレイがクリックされていない場合は手牌のクリックを処理
        if !self.is_my_turn || !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }

        // 九種九牌・チー・ポン・鳴きパネル表示中は手牌クリックを無視
        if self.nine_terminals_pending
            || self.chi_option_selecting
            || self.pon_option_selecting
            || !self.available_calls.is_empty()
        {
            return None;
        }

        if self.is_riichi {
            return None;
        }

        let (mx, my) = crate::renderer::mouse_position_design();

        // 手牌クリック（描画と同じ中央寄せ基準を使う）
        let hand_len = self.hand.len();
        let hand_start_x = crate::renderer::player_hand_start_x(hand_len);
        let hand_y = crate::renderer::HAND_Y;
        let tile_w = 48.0;
        let tile_h = 68.0;

        for i in 0..hand_len {
            let x = hand_start_x + i as f32 * tile_w;
            if mx >= x && mx <= x + tile_w && my >= hand_y && my <= hand_y + tile_h {
                if self.riichi_selection_mode && !self.riichi_selectable_tiles.contains(&i) {
                    return None;
                }

                // 喰い替え禁止牌は打牌できない。選択された見た目（少し上に表示）に
                // しつつ「喰い替えです！」警告を出し、打牌アクションは発行しない。
                if self.forbidden_discards.contains(&self.hand[i].get()) {
                    self.selected_tile = Some(i);
                    self.selected_drawn = false;
                    self.selected_would_cause_furiten = false;
                    self.selected_forbidden_swap = true;
                    return None;
                }

                if self.selected_tile == Some(i) {
                    let discarded_tile = self.apply_local_discard_from_hand(i);
                    if self.riichi_selection_mode {
                        self.clear_riichi_selection();
                        return Some(ClientAction::Riichi {
                            tile: Some(discarded_tile),
                        });
                    }
                    return Some(ClientAction::Discard {
                        tile: Some(discarded_tile),
                    });
                }

                self.selected_tile = Some(i);
                self.selected_drawn = false;
                self.selected_forbidden_swap = false;
                self.selected_would_cause_furiten =
                    self.would_discard_cause_furiten(Some(self.hand[i]));
                return None;
            }
        }

        if self.drawn.is_some() {
            let drawn_x = hand_start_x + hand_len as f32 * tile_w + crate::renderer::DRAWN_GAP;
            if mx >= drawn_x && mx <= drawn_x + tile_w && my >= hand_y && my <= hand_y + tile_h {
                if self.riichi_selection_mode && !self.riichi_selectable_drawn {
                    return None;
                }

                if self.selected_drawn {
                    self.selected_drawn = false;
                    self.drawn.take();
                    if self.riichi_selection_mode {
                        self.clear_riichi_selection();
                        return Some(ClientAction::Riichi { tile: None });
                    }
                    return Some(ClientAction::Discard { tile: None });
                }

                self.selected_drawn = true;
                self.selected_tile = None;
                self.selected_forbidden_swap = false;
                self.selected_would_cause_furiten = self.would_discard_cause_furiten(None);
                return None;
            }
        }

        None
    }

    fn relative_player_index(&self, wind: Wind) -> usize {
        let my_idx = self.seat_wind.map(|w| w.to_index()).unwrap_or(0);
        let their_idx = wind.to_index();
        (their_idx + 4 - my_idx) % 4
    }

    /// CallType → MeldType 変換
    fn call_type_to_meld_type(call_type: &CallType) -> MeldType {
        match call_type {
            CallType::Chi => MeldType::Chi,
            CallType::Pon => MeldType::Pon,
            CallType::Ankan | CallType::Daiminkan => MeldType::Kan,
            CallType::Kakan => MeldType::Kakan,
            CallType::Ron => MeldType::Pon, // フォールバック（使われない）
        }
    }

    /// 鳴いたプレイヤー(caller)から見て、鳴き元(discarder)がどの位置かを返す
    fn compute_meld_direction(caller: Wind, discarder: Wind) -> MeldFrom {
        let caller_idx = caller.to_index();
        let discarder_idx = discarder.to_index();
        let rel = (discarder_idx + 4 - caller_idx) % 4;
        match rel {
            3 => MeldFrom::Previous,  // 上家
            2 => MeldFrom::Opposite,  // 対面
            1 => MeldFrom::Following, // 下家
            _ => MeldFrom::Myself,    // 自家（通常ここには来ない）
        }
    }

    /// 和了者・放銃者などに使う席名（日本語は「東家」、英語は「East」）。
    fn wind_to_name(&self, wind: Wind) -> String {
        match self.lang {
            Lang::Ja => format!("{}家", wind.name(Lang::Ja)),
            Lang::En => wind.name(Lang::En).to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_local_players_assigns_cpu_labels_to_seats_1_to_3() {
        let mut state = GameState::new();
        let configs = [
            CpuConfig::new(CpuLevel::Weak, CpuPersonality::Defensive),
            CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue),
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Speedy),
        ];
        state.set_local_players(&configs);

        assert_eq!(state.my_seat, 0);
        assert!(matches!(state.player_labels[0], PlayerLabel::Me));
        assert_eq!(state.player_labels[0].detail(0, Lang::Ja), None);
        assert_eq!(
            state.player_labels[1].detail(1, Lang::Ja),
            Some("CPU1（弱い・守備的）".to_string())
        );
        assert_eq!(
            state.player_labels[2].name(2, Lang::Ja),
            "CPU2（強い・高得点）".to_string()
        );
        assert_eq!(
            state.player_labels[2].name(2, Lang::En),
            "CPU2 (Strong, High Value)".to_string()
        );
    }

    #[test]
    fn test_set_online_players_keeps_seat_order_and_self() {
        let mut state = GameState::new();
        let labels = [
            PlayerLabel::Human("ホスト".to_string()),
            PlayerLabel::Me,
            PlayerLabel::Cpu {
                level: "Normal".to_string(),
                personality: "Speedy".to_string(),
            },
            PlayerLabel::Cpu {
                level: "Normal".to_string(),
                personality: "HighValue".to_string(),
            },
        ];
        state.set_online_players(&labels, 1);

        assert_eq!(state.my_seat, 1);
        assert!(matches!(state.player_labels[1], PlayerLabel::Me));
        assert_eq!(
            state.player_labels[0].detail(3, Lang::Ja),
            Some("ホスト".to_string())
        );
        assert_eq!(
            state.player_labels[2].detail(1, Lang::Ja),
            Some("CPU1（普通・スピード）".to_string())
        );
    }

    #[test]
    fn test_called_tile_is_marked_in_river() {
        let mut state = GameState::new();
        state.seat_wind = Some(Wind::East);
        let tile = Tile::new(Tile::P3);

        // 南家が捨てる（河に積まれ、まだ鳴かれていない）
        state.handle_event(ServerEvent::TileDiscarded {
            player: Wind::South,
            tile,
            is_tsumogiri: false,
        });
        assert_eq!(state.discards[1].len(), 1);
        assert!(!state.discards[1][0].is_called);

        // 西家がポン → 南家の河の該当牌が鳴かれた扱いになる
        state.handle_event(ServerEvent::PlayerCalled {
            player: Wind::West,
            call_type: CallType::Pon,
            called_tile: tile,
            tiles: vec![tile, tile],
        });
        assert!(state.discards[1][0].is_called);
    }

    #[test]
    fn test_called_tile_marked_despite_stale_call_offer() {
        let mut state = GameState::new();
        state.seat_wind = Some(Wind::East);
        // 過去の鳴き打診で call_discarder が残っている状況（パスした後など）
        state.call_discarder = Some(Wind::North);

        let tile = Tile::new(Tile::S5);
        // 下家（南）が捨てる
        state.handle_event(ServerEvent::TileDiscarded {
            player: Wind::South,
            tile,
            is_tsumogiri: false,
        });
        // 対面（西）がチー → 古い call_discarder ではなく直前の打牌者を鳴き元とする
        state.handle_event(ServerEvent::PlayerCalled {
            player: Wind::West,
            call_type: CallType::Chi,
            called_tile: tile,
            tiles: vec![Tile::new(Tile::S6), Tile::new(Tile::S7)],
        });
        assert!(state.discards[1][0].is_called);
    }

    #[test]
    fn test_self_chi_sets_forbidden_swap_discards_and_clears_on_discard() {
        let mut state = GameState::new();
        state.seat_wind = Some(Wind::East);
        state.last_discarder = Some(Wind::North);

        // 上家が捨てた 3m を 4m,5m でチー（順子 3-4-5）
        state.handle_event(ServerEvent::PlayerCalled {
            player: Wind::East,
            call_type: CallType::Chi,
            called_tile: Tile::new(Tile::M3),
            tiles: vec![
                Tile::new(Tile::M3),
                Tile::new(Tile::M4),
                Tile::new(Tile::M5),
            ],
        });

        // 現物(3m)とスジ(6m)が打牌禁止になる
        assert!(state.forbidden_discards.contains(&Tile::M3));
        assert!(state.forbidden_discards.contains(&Tile::M6));

        // 打牌が完了すると制限が解除される
        state.handle_event(ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::P1),
            is_tsumogiri: false,
        });
        assert!(state.forbidden_discards.is_empty());
    }

    #[test]
    fn test_self_pon_forbids_only_called_tile() {
        let mut state = GameState::new();
        state.seat_wind = Some(Wind::East);
        state.last_discarder = Some(Wind::North);

        state.handle_event(ServerEvent::PlayerCalled {
            player: Wind::East,
            call_type: CallType::Pon,
            called_tile: Tile::new(Tile::S1),
            tiles: vec![Tile::new(Tile::S1); 3],
        });

        assert_eq!(state.forbidden_discards, vec![Tile::S1]);
    }

    #[test]
    fn test_self_daiminkan_has_no_forbidden_discards() {
        let mut state = GameState::new();
        state.seat_wind = Some(Wind::East);
        state.last_discarder = Some(Wind::North);

        state.handle_event(ServerEvent::PlayerCalled {
            player: Wind::East,
            call_type: CallType::Daiminkan,
            called_tile: Tile::new(Tile::S1),
            tiles: vec![Tile::new(Tile::S1); 4],
        });

        assert!(state.forbidden_discards.is_empty());
    }

    #[test]
    fn test_enter_riichi_selection_marks_only_tenpai_discards() {
        let mut state = GameState::new();
        let hand = Hand::from("123m123p123s45z67m 8m");
        state.hand = hand.tiles().to_vec();
        state.hand.sort();
        state.drawn = hand.drawn();
        state.enter_riichi_selection();

        assert_eq!(state.riichi_selectable_tiles.len(), 2);
        assert_eq!(
            state.hand[state.riichi_selectable_tiles[0]],
            Tile::new(Tile::Z4)
        );
        assert_eq!(
            state.hand[state.riichi_selectable_tiles[1]],
            Tile::new(Tile::Z5)
        );
        assert!(!state.riichi_selectable_drawn);
    }

    #[test]
    fn test_can_discard_for_riichi_rejects_non_tenpai_discard() {
        let mut state = GameState::new();
        let hand = Hand::from("123m123p123s45z67m 8m");
        state.hand = hand.tiles().to_vec();
        state.hand.sort();
        state.drawn = hand.drawn();

        assert!(!state.can_discard_for_riichi(None));
        assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::Z4))));
        assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::Z5))));
    }

    #[test]
    fn test_can_discard_for_riichi_after_ankan_uses_opened_melds() {
        let mut state = GameState::new();
        let hand = Hand::from("1m1m5m5m7m7m9m1s2s3s 3m3m3m3m 8m");
        state.hand = hand.tiles().to_vec();
        state.hand.sort();
        state.drawn = hand.drawn();
        state.melds.push(Meld {
            category: MeldType::Kan,
            tiles: vec![
                Tile::new(Tile::M3),
                Tile::new(Tile::M3),
                Tile::new(Tile::M3),
                Tile::new(Tile::M3),
            ],
            from: MeldFrom::Myself,
            called_tile: None,
        });

        assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::M5))));
        assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::M7))));
    }
}
