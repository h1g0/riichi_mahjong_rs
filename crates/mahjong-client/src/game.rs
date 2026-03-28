//! ゲーム状態管理
//!
//! サーバから受信したイベントに基づいてクライアント側の状態を管理する。

use macroquad::prelude::*;
use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType, Wind};
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::protocol::{AvailableCall, CallType, ClientAction, DrawReason, PlayerHandInfo, ServerEvent};

/// 捨て牌の表示情報
#[derive(Debug, Clone)]
pub struct DiscardInfo {
    pub tile: Tile,
    pub is_tsumogiri: bool,
    /// リーチ宣言牌かどうか（横向きに表示）
    pub is_riichi: bool,
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
    pub prevailing_wind: Option<Wind>,
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
    /// 局の結果メッセージ
    pub result_message: Option<String>,
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
    /// 対局開始前設定
    pub setup_state: SetupState,
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
            cpu_levels: [1, 1, 1],         // 全員 Normal
            cpu_personalities: [0, 1, 2],  // Balanced, Speedy, HighValue
        }
    }

    pub fn level_name(idx: usize) -> &'static str {
        match idx {
            0 => "Weak",
            1 => "Normal",
            2 => "Strong",
            _ => "Normal",
        }
    }

    pub fn personality_name(idx: usize) -> &'static str {
        match idx {
            0 => "Balanced",
            1 => "Speedy",
            2 => "HighValue",
            3 => "Defensive",
            _ => "Balanced",
        }
    }

    pub fn level_count() -> usize { 3 }
    pub fn personality_count() -> usize { 4 }

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
            CpuConfig::new(to_level(self.cpu_levels[0]), to_personality(self.cpu_personalities[0])),
            CpuConfig::new(to_level(self.cpu_levels[1]), to_personality(self.cpu_personalities[1])),
            CpuConfig::new(to_level(self.cpu_levels[2]), to_personality(self.cpu_personalities[2])),
        ]
    }
}

/// ゲームフェーズ
#[derive(Debug, Clone, PartialEq)]
pub enum GamePhase {
    /// 対局開始前の設定画面
    Setup,
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
            prevailing_wind: None,
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
            result_message: None,
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
            other_players: [OtherPlayerHand::new(), OtherPlayerHand::new(), OtherPlayerHand::new()],
            pending_riichi_player: None,
            last_discarder: None,
            setup_state: SetupState::new(),
        }
    }

    /// サーバイベントを処理する
    pub fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                prevailing_wind,
                dora_indicators,
                round_number,
                honba,
                riichi_sticks,
            } => {
                self.seat_wind = Some(seat_wind);
                self.hand = hand;
                self.hand.sort();
                self.drawn = None;
                self.scores = scores;
                self.prevailing_wind = Some(prevailing_wind);
                self.dora_indicators = dora_indicators;
                self.uradora_indicators = Vec::new();
                self.discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.pending_riichi_player = None;
                self.result_message = None;
                self.phase = GamePhase::Playing;
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();
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
                self.other_players = [OtherPlayerHand::new(), OtherPlayerHand::new(), OtherPlayerHand::new()];
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
                let relative_idx = self.relative_player_index(player);
                let is_riichi = self.pending_riichi_player == Some(player);
                if is_riichi {
                    self.pending_riichi_player = None;
                }
                self.discards[relative_idx].push(DiscardInfo {
                    tile,
                    is_tsumogiri,
                    is_riichi,
                });

                // 他プレイヤーが捨てた場合、隠し手牌の枚数を更新
                if relative_idx > 0 {
                    let other_idx = relative_idx - 1;
                    self.other_players[other_idx].concealed_count =
                        self.other_players[other_idx].concealed_count.saturating_sub(1);
                }

                // 自分が捨てた場合
                if Some(player) == self.seat_wind {
                    self.is_my_turn = false;
                    self.drawn = None;
                    self.selected_tile = None;
                    self.selected_drawn = false;
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
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
                                    && m.tiles.first().map(|t| t.get()) == tiles.first().map(|t| t.get())
                            }) {
                                meld.category = MeldType::Kakan;
                                meld.tiles = tiles.clone();
                                // from はポン時のままにする
                                other.concealed_count = other.concealed_count.saturating_sub(1);
                            } else {
                                other.melds.push(Meld {
                                    category, tiles: tiles.clone(),
                                    from: meld_from, called_tile: Some(called_tile),
                                });
                                other.concealed_count = other.concealed_count.saturating_sub(1);
                            }
                        }
                        CallType::Ankan => {
                            other.melds.push(Meld {
                                category, tiles: tiles.clone(),
                                from: MeldFrom::Myself, called_tile: None,
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(3);
                        }
                        CallType::Pon | CallType::Chi => {
                            other.melds.push(Meld {
                                category, tiles: tiles.clone(),
                                from: meld_from, called_tile: Some(called_tile),
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(2);
                        }
                        CallType::Daiminkan => {
                            other.melds.push(Meld {
                                category, tiles: tiles.clone(),
                                from: meld_from, called_tile: Some(called_tile),
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
                                category, tiles: tiles.clone(),
                                from: meld_from, called_tile: Some(called_tile),
                            });
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                        }
                        CallType::Ankan => {
                            self.melds.push(Meld {
                                category, tiles: tiles.clone(),
                                from: MeldFrom::Myself, called_tile: None,
                            });
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                        }
                        CallType::Kakan => {
                            if let Some(meld) = self.melds.iter_mut().find(|meld| {
                                meld.category == MeldType::Pon
                                    && meld.tiles.first().map(|tile| tile.get()) == tiles.first().map(|tile| tile.get())
                            }) {
                                meld.category = MeldType::Kakan;
                                meld.tiles = tiles.clone();
                            } else {
                                self.melds.push(Meld {
                                    category, tiles: tiles.clone(),
                                    from: meld_from, called_tile: Some(called_tile),
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
                rank_name,
                uradora_indicators,
                riichi_sticks,
                player_hands,
            } => {
                self.scores = scores;
                self.riichi_sticks = 0;
                self.uradora_indicators = uradora_indicators.clone();
                self.win_tile = Some(winning_tile);
                self.win_is_tsumo = loser.is_none();

                // 和了者の手牌情報を保存
                if let Some(_info) = player_hands.iter().find(|p| p.wind == winner) {
                    self.win_hand = _info.hand.clone();
                    // 既存の Meld（from 情報付き）を使用
                    let relative_idx = self.relative_player_index(winner);
                    if relative_idx == 0 {
                        // 自分が和了者
                        self.win_melds = self.melds.clone();
                    } else {
                        self.win_melds = self.other_players[relative_idx - 1].melds.clone();
                    }
                } else {
                    self.win_hand.clear();
                    self.win_melds.clear();
                }

                self.update_other_player_hands_on_win(&player_hands, winner);
                let winner_name = self.wind_to_name(winner);
                let win_type = if loser.is_some() { "ロン" } else { "ツモ" };
                let loser_text = if let Some(l) = loser {
                    format!("（{}が放銃）", self.wind_to_name(l))
                } else {
                    String::new()
                };

                // 役一覧を構築
                let mut yaku_text = String::new();
                for (name, y_han) in &yaku_list {
                    if !yaku_text.is_empty() {
                        yaku_text.push_str("  ");
                    }
                    yaku_text.push_str(&format!("{} {}翻", name, y_han));
                }

                // 点数表示
                let rank_display = if rank_name.is_empty() {
                    format!("{}符{}翻", fu, han)
                } else {
                    format!("{}符{}翻 {}", fu, han, rank_name)
                };

                let riichi_sticks_text = if riichi_sticks == 0 {
                    String::new()
                } else {
                    format!("\n供託: {}本", riichi_sticks)
                };

                let msg = format!(
                    "{}が{}和了！{}{}\n{}\n{} → {}点",
                    winner_name,
                    win_type,
                    loser_text,
                    riichi_sticks_text,
                    yaku_text,
                    rank_display,
                    score_points
                );
                self.result_message = Some(msg);
                self.phase = GamePhase::RoundResult;
                self.is_my_turn = false;
                self.available_calls.clear();
                self.clear_riichi_selection();
                self.self_kan_options.clear();
            }

            ServerEvent::RoundDraw {
                scores,
                reason,
                tenpai,
                riichi_sticks,
                player_hands,
            } => {
                self.scores = scores;
                self.riichi_sticks = riichi_sticks;
                self.update_other_player_hands_on_draw(&player_hands, &tenpai);
                let reason_text = match reason {
                    DrawReason::Exhaustive => "荒牌流局",
                    DrawReason::FourWinds => "四風連打",
                    DrawReason::FourRiichi => "四家立直",
                    DrawReason::NineTerminals => "九種九牌",
                    DrawReason::FourKans => "四槓散了",
                };
                let mut msg = format!("流局（{}）", reason_text);

                if !tenpai.is_empty() {
                    let tenpai_names: Vec<&str> = tenpai
                        .iter()
                        .map(|w| self.wind_to_name(*w))
                        .collect();
                    msg.push_str(&format!("\nテンパイ: {}", tenpai_names.join(", ")));
                }
                if riichi_sticks > 0 {
                    msg.push_str(&format!("\n供託: {}本", riichi_sticks));
                }

                self.result_message = Some(msg);
                self.phase = GamePhase::RoundResult;
                self.is_my_turn = false;
                self.available_calls.clear();
                self.clear_riichi_selection();
                self.self_kan_options.clear();
            }
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
            // 副露を更新
            // 副露を更新（既存の from 情報を保持）
            if other.melds.is_empty() {
                other.melds = info.melds.iter().map(|m| Meld {
                    category: Self::call_type_to_meld_type(&m.call_type),
                    tiles: m.tiles.clone(),
                    from: MeldFrom::Unknown, // フォールバック
                    called_tile: None,
                }).collect();
            }
            // 和了者の手牌を公開
            if info.wind == winner {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }

    /// 流局時に他プレイヤーの手牌を更新する（テンパイ者の手牌を公開）
    fn update_other_player_hands_on_draw(&mut self, player_hands: &[PlayerHandInfo], tenpai: &[Wind]) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue; // 自分はスキップ
            }
            let other = &mut self.other_players[relative_idx - 1];
            // 副露を更新
            // 副露を更新（既存の from 情報を保持）
            if other.melds.is_empty() {
                other.melds = info.melds.iter().map(|m| Meld {
                    category: Self::call_type_to_meld_type(&m.call_type),
                    tiles: m.tiles.clone(),
                    from: MeldFrom::Unknown, // フォールバック
                    called_tile: None,
                }).collect();
            }
            // テンパイ者の手牌を公開
            if tenpai.contains(&info.wind) {
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

        let mut hand = Hand::new_with_melds(self.hand.clone(), self.melds_for_analysis(), self.drawn);
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
            if let Ok(a) = HandAnalyzer::new(&test_hand) {
                if a.shanten.has_won() {
                    waiting.push(tile_type);
                }
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

        let mut counts = [0u8; Tile::LEN as usize];
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

    /// 入力処理: クリックで牌を選択し、アクションを返す
    pub fn handle_input(&mut self) -> Option<ClientAction> {
        if self.phase != GamePhase::Playing {
            return None;
        }

        if !self.available_calls.is_empty() {
            return self.handle_call_input();
        }

        if !self.is_my_turn {
            return None;
        }

        if self.is_riichi && self.drawn.is_some() && !self.can_tsumo {
            self.drawn.take();
            return Some(ClientAction::Discard { tile: None });
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();

            if self.can_tsumo {
                // 和了ボタン（手牌上部の大きなボタン）
                if mx >= crate::renderer::AGARI_BTN_X
                    && mx <= crate::renderer::AGARI_BTN_X + crate::renderer::AGARI_BTN_W
                    && my >= crate::renderer::AGARI_BTN_Y
                    && my <= crate::renderer::AGARI_BTN_Y + crate::renderer::AGARI_BTN_H
                {
                    return Some(ClientAction::Tsumo);
                }
            }

            if self.can_riichi {
                let riichi_x = 1000.0;
                let riichi_y = 720.0;
                let btn_w = 80.0;
                let btn_h = 40.0;
                if mx >= riichi_x
                    && mx <= riichi_x + btn_w
                    && my >= riichi_y
                    && my <= riichi_y + btn_h
                {
                    if self.riichi_selection_mode {
                        self.clear_riichi_selection();
                    } else {
                        self.enter_riichi_selection();
                    }
                    return None;
                }
            }

            for (idx, tile) in self.self_kan_options.iter().enumerate() {
                let x = 720.0 + idx as f32 * 110.0;
                let y = 670.0;
                let btn_w = 100.0;
                let btn_h = 40.0;
                if mx >= x && mx <= x + btn_w && my >= y && my <= y + btn_h {
                    return Some(ClientAction::Kan {
                        tile_index: tile.get() as usize,
                    });
                }
            }

            if self.is_riichi {
                return None;
            }

            let hand_start_x = 100.0;
            let hand_y = 680.0;
            let tile_w = 48.0;
            let tile_h = 68.0;
            let hand_len = self.hand.len();

            for i in 0..hand_len {
                let x = hand_start_x + i as f32 * tile_w;
                if mx >= x && mx <= x + tile_w && my >= hand_y && my <= hand_y + tile_h {
                    if self.riichi_selection_mode && !self.riichi_selectable_tiles.contains(&i) {
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
                    self.selected_would_cause_furiten =
                        self.would_discard_cause_furiten(Some(self.hand[i]));
                    return None;
                }
            }

            if self.drawn.is_some() {
                let drawn_x = hand_start_x + hand_len as f32 * tile_w + 20.0;
                if mx >= drawn_x
                    && mx <= drawn_x + tile_w
                    && my >= hand_y
                    && my <= hand_y + tile_h
                {
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
                    self.selected_would_cause_furiten =
                        self.would_discard_cause_furiten(None);
                    return None;
                }
            }
        }

        None
    }

    /// 鳴きボタンの入力処理
    fn handle_call_input(&mut self) -> Option<ClientAction> {
        if !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }

        let (mx, my) = mouse_position();

        // 和了ボタン（ロン）の判定 — 手牌上部の大きなボタン
        let has_ron = self
            .available_calls
            .iter()
            .any(|c| matches!(c, AvailableCall::Ron));
        if has_ron
            && mx >= crate::renderer::AGARI_BTN_X
            && mx <= crate::renderer::AGARI_BTN_X + crate::renderer::AGARI_BTN_W
            && my >= crate::renderer::AGARI_BTN_Y
            && my <= crate::renderer::AGARI_BTN_Y + crate::renderer::AGARI_BTN_H
        {
            self.available_calls.clear();
            return Some(ClientAction::Ron);
        }

        // 鳴きボタンの配置 — ロンがある場合は和了ボタンの右側に配置
        let base_x = if has_ron {
            crate::renderer::AGARI_BTN_X + crate::renderer::AGARI_BTN_W + 20.0
        } else {
            400.0
        };
        let base_y = if has_ron {
            crate::renderer::AGARI_BTN_Y + 10.0
        } else {
            620.0
        };
        let btn_w = 100.0;
        let btn_h = 40.0;
        let btn_spacing = 10.0;

        let mut btn_idx = 0;

        for call in &self.available_calls {
            if matches!(call, AvailableCall::Ron) {
                continue;
            }
            let x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
            if mx >= x && mx <= x + btn_w && my >= base_y && my <= base_y + btn_h {
                match call {
                    AvailableCall::Ron => unreachable!(),
                    AvailableCall::Pon => {
                        self.available_calls.clear();
                        return Some(ClientAction::Pon);
                    }
                    AvailableCall::Daiminkan => {
                        let tile = self.call_target_tile?;
                        self.available_calls.clear();
                        return Some(ClientAction::Kan {
                            tile_index: tile.get() as usize,
                        });
                    }
                    AvailableCall::Chi { options } => {
                        // 最初の選択肢を使う（複数ある場合はMVPでは最初を選択）
                        if let Some(&tiles) = options.first() {
                            self.available_calls.clear();
                            return Some(ClientAction::Chi { tiles });
                        }
                    }
                }
            }
            btn_idx += 1;
        }

        // パスボタン（最後に配置）
        let pass_x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
        if mx >= pass_x && mx <= pass_x + btn_w && my >= base_y && my <= base_y + btn_h {
            self.available_calls.clear();
            self.call_target_tile = None;
            return Some(ClientAction::Pass);
        }

        None
    }

    /// 風牌を相対位置（自分=0, 下家=1, 対面=2, 上家=3）に変換
    fn relative_player_index(&self, wind: Wind) -> usize {
        let my_idx = self
            .seat_wind
            .map(|w| w.to_index())
            .unwrap_or(0);
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
            2 => MeldFrom::Opposite,   // 対面
            1 => MeldFrom::Following, // 下家
            _ => MeldFrom::Myself,   // 自家（通常ここには来ない）
        }
    }

    /// 風牌を日本語の名前に変換
    fn wind_to_name(&self, wind: Wind) -> &'static str {
        match wind {
            Wind::East => "東家",
            Wind::South => "南家",
            Wind::West => "西家",
            Wind::North => "北家",
        }
    }
}

/// 牌を文字列に変換
pub fn tile_to_string(tile: Tile) -> String {
    let tile_type = tile.get();
    let names = [
        "一萬", "二萬", "三萬", "四萬", "五萬", "六萬", "七萬", "八萬", "九萬",
        "一筒", "二筒", "三筒", "四筒", "五筒", "六筒", "七筒", "八筒", "九筒",
        "一索", "二索", "三索", "四索", "五索", "六索", "七索", "八索", "九索",
        "東", "南", "西", "北", "白", "發", "中",
    ];
    if (tile_type as usize) < names.len() {
        let name = names[tile_type as usize].to_string();
        if tile.is_red_dora() {
            format!("{}(赤)", name)
        } else {
            name
        }
    } else {
        "?".to_string()
    }
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_riichi_selection_marks_only_tenpai_discards() {
        let mut state = GameState::new();
        let hand = Hand::from("123m123p123s45z67m 8m");
        state.hand = hand.tiles().to_vec();
        state.hand.sort();
        state.drawn = hand.drawn();
        state.enter_riichi_selection();

        assert_eq!(state.riichi_selectable_tiles.len(), 2);
        assert_eq!(state.hand[state.riichi_selectable_tiles[0]], Tile::new(Tile::Z4));
        assert_eq!(state.hand[state.riichi_selectable_tiles[1]], Tile::new(Tile::Z5));
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

