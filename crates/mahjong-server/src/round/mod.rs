//! 局の管理
//!
//! 1局分のゲーム進行を管理する。
//! ツモ → 打牌 → 鳴き判定 → 次の手番 のターンフローを制御する。

#[cfg(debug_assertions)]
mod diagnostics;
#[cfg(test)]
mod test_helpers;

use mahjong_core::hand_info::hand_analyzer;
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind};

use crate::player::Player;
use crate::protocol::{
    AvailableCall, CallType, DrawReason, MeldTiles, PlayerHandInfo, ServerEvent,
};
use crate::scoring;
use crate::wall::Wall;

/// リーチ棒1本の点数
const RIICHI_STICK_VALUE: i32 = 1000;
/// リーチ宣言に必要な最低持ち点
const RIICHI_MIN_SCORE: i32 = 1000;

/// ターンのフェーズ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnPhase {
    /// ツモフェーズ: 現在のプレイヤーがツモる
    Draw,
    /// 打牌待ち: 現在のプレイヤーの打牌を待つ
    WaitForDiscard,
    /// 鳴き待ち: 打牌後、他プレイヤーの鳴き応答を待つ
    WaitForCalls,
    /// 九種九牌待ち: プレイヤーが流局を宣言するか選択するのを待つ
    WaitForNineTerminals,
    /// 局終了
    RoundOver,
}

/// 局の結果
#[derive(Debug, Clone)]
pub enum RoundResult {
    /// ツモ和了
    Tsumo { winner: usize, winning_tile: Tile },
    /// ロン和了（1人・ダブロン・トリロン共通）
    Ron {
        /// 和了プレイヤーのインデックス（打順優先順: 下家→対面→上家）
        winners: Vec<usize>,
        loser: usize,
        winning_tile: Tile,
    },
    /// 荒牌流局（牌山切れ）
    ExhaustiveDraw {
        /// 親がテンパイしているか
        dealer_tenpai: bool,
    },
    /// 途中流局（四風連打、四家立直、九種九牌）
    SpecialDraw,
}

/// 鳴き解決後の進行先
#[derive(Debug, Clone)]
enum CallResolution {
    /// 通常の打牌後処理
    AfterDiscard,
    /// 加カンに対する搶槓判定後の処理
    AfterKakan { caller: usize, tile_type: TileType },
}

/// 鳴き待ち中の状態
#[derive(Debug, Clone)]
pub struct CallState {
    /// 捨てられた牌
    pub discarded_tile: Tile,
    /// 捨てたプレイヤー
    pub discarder: usize,
    /// 各プレイヤーが可能な鳴きのリスト（空=鳴き不可）
    pub available_calls: [Vec<AvailableCall>; 4],
    /// 各プレイヤーが応答済みか（true=応答済みまたは対象外）
    pub responded: [bool; 4],
    /// ロンを宣言したプレイヤー（複数ロン対応用）
    pub ron_declared: Vec<usize>,
    /// ポンを宣言したプレイヤーと使う手牌2枚
    pub pon_declared: Option<(usize, [Tile; 2])>,
    /// 大明カンを宣言したプレイヤー
    pub daiminkan_declared: Option<usize>,
    /// チーを宣言したプレイヤーと使う手牌2枚
    pub chi_declared: Option<(usize, [Tile; 2])>,
    /// 全員応答後の進行先
    resolution: CallResolution,
}

/// 1局分の状態
pub struct Round {
    /// 牌山
    pub wall: Wall,
    /// 4人のプレイヤー
    pub players: [Player; 4],
    /// 場風
    pub round_wind: Wind,
    /// 親のプレイヤーインデックス（0-3）
    pub dealer: usize,
    /// 現在の手番プレイヤー（0-3）
    pub current_player: usize,
    /// 本場数
    pub honba: usize,
    /// 場に出ている供託リーチ棒の本数
    pub riichi_sticks: usize,
    /// ターンフェーズ
    pub phase: TurnPhase,
    /// 局の結果（終了時にセット）
    pub result: Option<RoundResult>,
    /// 溜まったイベントキュー
    events: Vec<(usize, ServerEvent)>,
    /// 鳴き待ち中の状態
    pub call_state: Option<CallState>,
    /// 直前のツモが嶺上牌か
    pub last_draw_was_dead_wall: bool,
    /// ゲーム設定
    pub settings: Settings,
}

impl Round {
    /// 新しい局を開始する
    ///
    /// - `round_wind`: 場風（東場なら East）
    /// - `dealer`: 親のプレイヤーインデックス（0-3）
    /// - `initial_scores`: 各プレイヤーの初期点数
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        round_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        total_rounds: usize,
        settings: Settings,
    ) -> Self {
        Self::with_wall(
            Wall::new(),
            round_wind,
            dealer,
            initial_scores,
            honba,
            riichi_sticks,
            round_number,
            total_rounds,
            settings,
        )
    }

    /// 固定シードの牌山でラウンドを生成する
    ///
    /// 牌山が決定的になるため、シミュレーション・再現性のあるテストに使用する。
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_seed(
        seed: u64,
        round_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        total_rounds: usize,
        settings: Settings,
    ) -> Self {
        Self::with_wall(
            Wall::new_with_seed(seed),
            round_wind,
            dealer,
            initial_scores,
            honba,
            riichi_sticks,
            round_number,
            total_rounds,
            settings,
        )
    }

    /// 指定した牌山から局を開始する共通処理
    #[allow(clippy::too_many_arguments)]
    fn with_wall(
        mut wall: Wall,
        round_wind: Wind,
        dealer: usize,
        initial_scores: [i32; 4],
        honba: usize,
        riichi_sticks: usize,
        round_number: usize,
        total_rounds: usize,
        settings: Settings,
    ) -> Self {
        let dealt = wall.deal();

        // 座席の風を割り当て: dealer=東, 反時計回りに南西北
        let winds = [
            Wind::from_index((4 - dealer) % 4),
            Wind::from_index((1 + 4 - dealer) % 4),
            Wind::from_index((2 + 4 - dealer) % 4),
            Wind::from_index((3 + 4 - dealer) % 4),
        ];

        let players = [
            Player::new(winds[0], dealt[0].clone(), initial_scores[0]),
            Player::new(winds[1], dealt[1].clone(), initial_scores[1]),
            Player::new(winds[2], dealt[2].clone(), initial_scores[2]),
            Player::new(winds[3], dealt[3].clone(), initial_scores[3]),
        ];

        let dora_indicators = wall.dora_indicators();

        // 各プレイヤーにゲーム開始イベントを送信
        let mut events = Vec::new();
        for (i, player) in players.iter().enumerate() {
            events.push((
                i,
                ServerEvent::GameStarted {
                    seat_wind: player.seat_wind,
                    hand: player.hand.tiles().to_vec(),
                    scores: initial_scores,
                    round_wind,
                    dora_indicators: dora_indicators.clone(),
                    round_number,
                    total_rounds,
                    honba,
                    riichi_sticks,
                },
            ));
        }

        Round {
            wall,
            players,
            round_wind,
            dealer,
            current_player: dealer,
            honba,
            riichi_sticks,
            phase: TurnPhase::Draw,
            result: None,
            events,
            call_state: None,
            last_draw_was_dead_wall: false,
            settings,
        }
    }

    /// 各プレイヤーの点数を返す
    /// 全プレイヤーの手牌情報を構築する
    fn build_player_hands(&self) -> Vec<PlayerHandInfo> {
        self.players
            .iter()
            .map(|p| {
                let melds: Vec<MeldTiles> = p
                    .hand
                    .melds()
                    .iter()
                    .map(|open| {
                        let tiles: Vec<Tile> = open.expanded_tiles();
                        let call_type = match open.category {
                            mahjong_core::hand_info::meld::MeldType::Chi => CallType::Chi,
                            mahjong_core::hand_info::meld::MeldType::Pon => CallType::Pon,
                            mahjong_core::hand_info::meld::MeldType::Kan => {
                                if open.from == mahjong_core::hand_info::meld::MeldFrom::Myself {
                                    CallType::Ankan
                                } else {
                                    CallType::Daiminkan
                                }
                            }
                            mahjong_core::hand_info::meld::MeldType::Kakan => CallType::Kakan,
                        };
                        MeldTiles { call_type, tiles }
                    })
                    .collect();

                PlayerHandInfo {
                    wind: p.seat_wind,
                    hand: p.hand.tiles().to_vec(),
                    melds,
                }
            })
            .collect()
    }

    pub fn get_scores(&self) -> [i32; 4] {
        [
            self.players[0].score,
            self.players[1].score,
            self.players[2].score,
            self.players[3].score,
        ]
    }

    /// 溜まったイベントを取り出す
    /// 戻り値: (対象プレイヤーインデックス, イベント) のリスト
    pub fn drain_events(&mut self) -> Vec<(usize, ServerEvent)> {
        std::mem::take(&mut self.events)
    }

    /// ツモフェーズを実行する
    /// 山から1枚引いて現在のプレイヤーに配る
    pub fn do_draw(&mut self) -> bool {
        if self.phase != TurnPhase::Draw {
            return false;
        }

        // 同巡フリテンを解除（自分のツモ番が来たので）
        self.players[self.current_player].is_temporary_furiten = false;

        // 牌山が空なら流局
        if self.wall.is_empty() {
            self.do_exhaustive_draw();
            return true;
        }

        let Some(tile) = self.wall.draw() else {
            self.do_exhaustive_draw();
            return true;
        };
        self.players[self.current_player].draw(tile);
        self.last_draw_was_dead_wall = false;
        self.phase = TurnPhase::WaitForDiscard;

        self.push_draw_events(self.current_player, tile, "draw");

        // 九種九牌チェック: 初回ツモかつ条件を満たす場合に選択を促す
        if self.settings.nine_terminals_draw && self.check_nine_terminals() {
            self.phase = TurnPhase::WaitForNineTerminals;
            self.events
                .push((self.current_player, ServerEvent::NineTerminalsAvailable));
        }

        true
    }

    /// 打牌を実行する
    ///
    /// - `tile`: 捨てる牌（Noneならツモ切り）
    ///
    /// 打牌後、他プレイヤーの鳴き候補をチェックし、
    /// 鳴き候補があれば WaitForCalls フェーズに移行する。
    pub fn do_discard(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let Some(discarded) = self.players[self.current_player].try_discard(tile) else {
            return false;
        };

        // 一発フラグは try_discard() 内で解除済み。
        // リーチ宣言牌の打牌は do_riichi() が別途処理し、そこでフラグを復元する。
        self.announce_discard_and_check_calls(discarded, self.current_player, tile.is_none());

        true
    }

    /// 打牌の通知と鳴き候補チェックを行い、フェーズを遷移させる
    ///
    /// `do_discard` と `do_riichi` で共通の打牌後処理。
    fn announce_discard_and_check_calls(
        &mut self,
        discarded: Tile,
        discarder: usize,
        is_tsumogiri: bool,
    ) {
        // 全プレイヤーに打牌を通知
        let discarder_wind = self.players[discarder].seat_wind;
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::TileDiscarded {
                    player: discarder_wind,
                    tile: discarded,
                    is_tsumogiri,
                },
            ));
        }

        // 鳴き候補をチェック
        let call_state = self.check_available_calls(discarded, discarder);
        let has_any_calls = call_state.available_calls.iter().any(|c| !c.is_empty());

        if has_any_calls {
            // 鳴き候補がある場合、WaitForCalls フェーズへ
            self.phase = TurnPhase::WaitForCalls;

            // 各プレイヤーに鳴き可能通知を送信
            for i in 0..4 {
                if !call_state.available_calls[i].is_empty() {
                    self.events.push((
                        i,
                        ServerEvent::CallAvailable {
                            tile: discarded,
                            discarder: discarder_wind,
                            calls: call_state.available_calls[i].clone(),
                        },
                    ));
                }
            }

            self.call_state = Some(call_state);
        } else {
            // 鳴き候補がなければ次のプレイヤーへ
            self.current_player = (discarder + 1) % 4;
            self.phase = TurnPhase::Draw;

            // 特殊流局チェック（四家立直チェック含む）
            self.check_special_draws();
        }
    }

    /// 打牌後の鳴き候補を全てチェックする
    fn check_available_calls(&self, discarded_tile: Tile, discarder: usize) -> CallState {
        let is_last_tile = self.wall.is_empty();
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut responded = [true; 4]; // デフォルトは応答済み（対象外）

        for i in 0..4 {
            if i == discarder {
                continue;
            }

            let player = &self.players[i];

            // リーチ中は鳴き不可（ロンのみ可）
            // ロン判定: フリテンでなく、和了形であること
            if !player.is_furiten() {
                let win_result = scoring::check_ron_with_settings(
                    player,
                    discarded_tile,
                    self.round_wind,
                    is_last_tile,
                    &self.settings,
                );
                if win_result.is_win {
                    available_calls[i].push(AvailableCall::Ron);
                }
            }

            // リーチ中は鳴き不可
            if player.is_riichi {
                if !available_calls[i].is_empty() {
                    responded[i] = false;
                }
                continue;
            }

            // ポン判定
            let pon_opts = player.pon_options(discarded_tile);
            if !pon_opts.is_empty() {
                available_calls[i].push(AvailableCall::Pon { options: pon_opts });
            }

            // 大明カン判定（場全体で4回カン済みなら不可）
            if self.total_kan_count() < 4 && player.can_daiminkan(discarded_tile) {
                available_calls[i].push(AvailableCall::Daiminkan);
            }

            // チー判定（上家からのみ＝次のプレイヤー）
            let next_player = (discarder + 1) % 4;
            if i == next_player {
                let chi_opts = player.chi_options(discarded_tile);
                if !chi_opts.is_empty() {
                    available_calls[i].push(AvailableCall::Chi { options: chi_opts });
                }
            }

            if !available_calls[i].is_empty() {
                responded[i] = false;
            }
        }

        CallState {
            discarded_tile,
            discarder,
            available_calls,
            responded,
            ron_declared: Vec::new(),
            pon_declared: None,
            daiminkan_declared: None,
            chi_declared: None,
            resolution: CallResolution::AfterDiscard,
        }
    }

    /// 鳴き応答を処理する
    ///
    /// プレイヤーからの鳴き応答（Ron/Pon/Chi/Pass）を受け付ける。
    /// 全員の応答が揃ったら、優先度に基づいて鳴きを解決する。
    pub fn respond_to_call(&mut self, player_idx: usize, response: CallResponse) -> bool {
        if self.phase != TurnPhase::WaitForCalls {
            return false;
        }

        let call_state = match self.call_state.as_mut() {
            Some(cs) => cs,
            None => return false,
        };

        // 既に応答済みなら無視
        if call_state.responded[player_idx] {
            return false;
        }

        // 応答を記録
        match response {
            CallResponse::Ron => {
                // ロン可能か確認
                if call_state.available_calls[player_idx]
                    .iter()
                    .any(|c| matches!(c, AvailableCall::Ron))
                {
                    call_state.ron_declared.push(player_idx);
                } else {
                    return false;
                }
            }
            CallResponse::Pon { hand_tile_types } => {
                // ポンの組み合わせが有効か確認
                let valid = call_state.available_calls[player_idx].iter().any(|c| {
                    if let AvailableCall::Pon { options } = c {
                        options.contains(&hand_tile_types)
                    } else {
                        false
                    }
                });
                if valid {
                    call_state.pon_declared = Some((player_idx, hand_tile_types));
                } else {
                    return false;
                }
            }
            CallResponse::Daiminkan => {
                if call_state.available_calls[player_idx]
                    .iter()
                    .any(|c| matches!(c, AvailableCall::Daiminkan))
                {
                    call_state.daiminkan_declared = Some(player_idx);
                } else {
                    return false;
                }
            }
            CallResponse::Chi { hand_tile_types } => {
                // チーの組み合わせが有効か確認
                let valid = call_state.available_calls[player_idx].iter().any(|c| {
                    if let AvailableCall::Chi { options } = c {
                        options.contains(&hand_tile_types)
                    } else {
                        false
                    }
                });
                if valid {
                    call_state.chi_declared = Some((player_idx, hand_tile_types));
                } else {
                    return false;
                }
            }
            CallResponse::Pass => {
                // パスは何もしない
            }
        }

        call_state.responded[player_idx] = true;

        // 全員応答済みなら解決
        if call_state.responded.iter().all(|&r| r) {
            self.resolve_calls();
        }

        true
    }

    /// 鳴きを解決する（優先度: ロン > 大明カン > ポン > チー > パス）
    fn resolve_calls(&mut self) {
        let call_state = self.call_state.take().unwrap();

        // ロン見逃しによるフリテン判定
        // AvailableCall::Ron があったのにロン宣言しなかったプレイヤーにフリテンを設定
        for i in 0..4 {
            let had_ron = call_state.available_calls[i]
                .iter()
                .any(|c| matches!(c, AvailableCall::Ron));
            let declared_ron = call_state.ron_declared.contains(&i);

            if had_ron && !declared_ron {
                if self.players[i].is_riichi {
                    // リーチ中 → リーチ後フリテン（局終了まで永続）
                    self.players[i].is_riichi_furiten = true;
                } else {
                    // 非リーチ → 同巡フリテン（自分のツモ番で解除）
                    self.players[i].is_temporary_furiten = true;
                }
            }
        }

        // 1. ロン（最優先）
        if !call_state.ron_declared.is_empty() {
            let is_robbing_a_quad =
                matches!(call_state.resolution, CallResolution::AfterKakan { .. });
            let discarder = call_state.discarder;
            let winning_tile = call_state.discarded_tile;
            let ron_count = call_state.ron_declared.len();

            // 打順優先順（下家→対面→上家）でソート
            let mut sorted_winners = call_state.ron_declared.clone();
            sorted_winners.sort_by_key(|&p| (p + 4 - discarder) % 4);

            if ron_count >= 3 && self.settings.triple_ron_draw {
                // 三家和流局（最優先）
                self.declare_special_draw(DrawReason::TripleRon, None);
                return;
            }

            // 複数同時ロンが有効かつ2人以上: 全員和了
            let winners = if ron_count >= 2 && self.settings.multiple_ron {
                sorted_winners
            } else {
                // 上家取り: 最優先の1人のみ和了
                vec![sorted_winners[0]]
            };

            self.execute_ron(winners, discarder, winning_tile, is_robbing_a_quad);
            return;
        }

        if let CallResolution::AfterKakan { caller, tile_type } = call_state.resolution {
            self.execute_kakan(caller, tile_type);
            return;
        }

        // 2. 大明カン
        if let Some(caller) = call_state.daiminkan_declared {
            self.execute_daiminkan(caller, call_state.discarder, call_state.discarded_tile);
            return;
        }

        // 3. ポン
        if let Some((caller, hand_tile_types)) = call_state.pon_declared {
            self.execute_pon(
                caller,
                call_state.discarder,
                call_state.discarded_tile,
                hand_tile_types,
            );
            return;
        }

        // 4. チー
        if let Some((caller, hand_tile_types)) = call_state.chi_declared {
            self.execute_chi(
                caller,
                call_state.discarder,
                call_state.discarded_tile,
                hand_tile_types,
            );
            return;
        }

        // 5. 全員パス → 次のプレイヤーへ
        self.current_player = (call_state.discarder + 1) % 4;
        self.phase = TurnPhase::Draw;

        // 特殊流局チェック
        self.check_special_draws();
    }

    /// ロン和了を実行する（通常・ダブロン・トリロン共通）
    ///
    /// - winners: ロン和了者の打順優先順（下家→対面→上家）でソート済みのインデックスリスト
    /// - 本場ボーナスと供託棒は最初の和了者（打順最優先）のみが取得する
    fn execute_ron(
        &mut self,
        winners: Vec<usize>,
        loser: usize,
        winning_tile: Tile,
        is_robbing_a_quad: bool,
    ) {
        let is_last_tile = self.wall.is_empty();
        let dora_indicators = self.wall.dora_indicators();
        let riichi_sticks = self.riichi_sticks;
        let player_hands = self.build_player_hands();

        struct WinnerData {
            winner: usize,
            score_result: mahjong_core::scoring::score::ScoreResult,
            deltas: [i32; 4],
            uradora_indicators: Vec<Tile>,
            score_points: i32,
        }

        // 打順が最も早い和了者を rank=0 として本場・供託ボーナスの基準にする
        let mut winner_data: Vec<WinnerData> = Vec::new();

        for (rank, &winner) in winners.iter().enumerate() {
            let honba_for_this = if rank == 0 { self.honba } else { 0 };

            let win_result = scoring::check_ron_with_flags_and_settings(
                &self.players[winner],
                winning_tile,
                self.round_wind,
                is_last_tile,
                is_robbing_a_quad,
                &self.settings,
            );

            if !win_result.is_win {
                continue;
            }

            let Some(mut score_result) = win_result.score_result else {
                continue;
            };

            let uradora_indicators = if self.players[winner].is_riichi {
                self.wall.uradora_indicators()
            } else {
                vec![]
            };

            scoring::add_dora_to_score(
                &mut score_result,
                &self.players[winner].hand,
                Some(winning_tile),
                &dora_indicators,
                &uradora_indicators,
            );

            let winner_is_dealer = self.players[winner].is_dealer();
            let deltas = scoring::calculate_ron_score_deltas(
                winner,
                loser,
                &score_result,
                winner_is_dealer,
                honba_for_this,
            );

            // 供託棒は打順最優先の和了者（winner_data の先頭）のみ取得
            let riichi_bonus = if winner_data.is_empty() {
                (riichi_sticks as i32) * RIICHI_STICK_VALUE
            } else {
                0
            };
            let score_points = deltas[winner] + riichi_bonus;

            winner_data.push(WinnerData {
                winner,
                score_result,
                deltas,
                uradora_indicators,
                score_points,
            });
        }

        // 安全のため: 和了成立者が0人ならフェーズを進めて返す
        if winner_data.is_empty() {
            self.current_player = (loser + 1) % 4;
            self.phase = TurnPhase::Draw;
            return;
        }

        // 全スコアデルタを合算して適用
        for wd in &winner_data {
            for i in 0..4 {
                self.players[i].score += wd.deltas[i];
            }
        }
        // 供託棒は打順最優先の和了者に付与
        if riichi_sticks > 0 {
            self.players[winner_data[0].winner].score +=
                (riichi_sticks as i32) * RIICHI_STICK_VALUE;
            self.riichi_sticks = 0;
        }

        if !is_robbing_a_quad {
            self.mark_last_discard_as_called(loser);
        }

        let scores = self.get_scores();
        let loser_wind = self.players[loser].seat_wind;

        // 各和了者にRoundWonイベントを送信
        for (idx, wd) in winner_data.iter().enumerate() {
            let winner_wind = self.players[wd.winner].seat_wind;
            let yaku_list = wd.score_result.yaku_list.clone();
            let rank = wd.score_result.rank;
            let has_opened = wd.score_result.has_opened;
            let event_riichi_sticks = if idx == 0 { riichi_sticks } else { 0 };

            for i in 0..4 {
                self.events.push((
                    i,
                    ServerEvent::RoundWon {
                        winner: winner_wind,
                        loser: Some(loser_wind),
                        winning_tile,
                        scores,
                        yaku_list: yaku_list.clone(),
                        han: wd.score_result.han,
                        fu: wd.score_result.fu,
                        score_points: wd.score_points,
                        rank,
                        has_opened,
                        uradora_indicators: wd.uradora_indicators.clone(),
                        riichi_sticks: event_riichi_sticks,
                        player_hands: player_hands.clone(),
                    },
                ));
            }
        }

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::Ron {
            winners,
            loser,
            winning_tile,
        });
    }

    /// ポンを実行する
    fn execute_pon(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        let from = Player::meld_from_relative(caller, discarder);
        self.players[caller].do_pon(called_tile, hand_tile_types, from);

        // 捨て牌を「鳴かれた」としてマーク
        self.mark_last_discard_as_called(discarder);

        // 鳴きにより全プレイヤーの一発フラグを無効化
        self.invalidate_first_turn_flags();

        // 全プレイヤーにポン通知
        let caller_wind = self.players[caller].seat_wind;
        let tiles: Vec<Tile> = self.players[caller]
            .hand
            .melds()
            .last()
            .unwrap()
            .tiles
            .to_vec();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Pon,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        // 鳴いたプレイヤーに手牌更新を通知
        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        // 喰い替え禁止牌を設定し、ポンしたプレイヤーの打牌待ちへ
        self.apply_swap_call_restriction(caller);
        self.current_player = caller;
        self.phase = TurnPhase::WaitForDiscard;
    }

    /// 大明カンを実行する
    fn execute_daiminkan(&mut self, caller: usize, discarder: usize, called_tile: Tile) {
        let from = Player::meld_from_relative(caller, discarder);
        self.players[caller].do_daiminkan(called_tile, from);

        self.mark_last_discard_as_called(discarder);
        self.invalidate_first_turn_flags();

        let caller_wind = self.players[caller].seat_wind;
        let open = self.players[caller].hand.melds().last().unwrap();
        let tiles = open.expanded_tiles();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Daiminkan,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.current_player = caller;
        self.draw_after_kan(caller);
    }

    /// チーを実行する
    fn execute_chi(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        self.players[caller].do_chi(called_tile, hand_tile_types);

        // 捨て牌を「鳴かれた」としてマーク
        self.mark_last_discard_as_called(discarder);

        // 鳴きにより全プレイヤーの一発フラグを無効化
        self.invalidate_first_turn_flags();

        // 全プレイヤーにチー通知
        let caller_wind = self.players[caller].seat_wind;
        let tiles: Vec<Tile> = self.players[caller]
            .hand
            .melds()
            .last()
            .unwrap()
            .tiles
            .to_vec();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Chi,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        // 鳴いたプレイヤーに手牌更新を通知
        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        // 喰い替え禁止牌を設定し、チーしたプレイヤーの打牌待ちへ
        self.apply_swap_call_restriction(caller);
        self.current_player = caller;
        self.phase = TurnPhase::WaitForDiscard;
    }

    /// チー・ポン直後の喰い替え禁止牌を、設定が有効なら当該プレイヤーに設定する
    fn apply_swap_call_restriction(&mut self, caller: usize) {
        if !self.settings.forbid_swap_calling {
            return;
        }
        let forbidden = self.players[caller]
            .hand
            .melds()
            .last()
            .map(|meld| meld.forbidden_swap_tiles())
            .unwrap_or_default();
        self.players[caller].set_forbidden_discards(forbidden);
    }

    fn execute_kakan(&mut self, caller: usize, tile_type: TileType) {
        self.players[caller].do_kakan(tile_type);
        self.invalidate_first_turn_flags();

        let caller_wind = self.players[caller].seat_wind;
        let open = self.players[caller]
            .hand
            .melds()
            .iter()
            .rev()
            .find(|open| {
                open.category == mahjong_core::hand_info::meld::MeldType::Kakan
                    && open.tiles[0].get() == tile_type
            })
            .unwrap();
        let tiles = open.expanded_tiles();
        let added_tile = open.kan_fourth_tile();

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Kakan,
                    called_tile: added_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.draw_after_kan(caller);
    }

    fn check_kakan_ron_and_resolve(&mut self, caller: usize, tile_type: TileType) {
        let called_tile = self.players[caller]
            .kakan_added_tile(tile_type)
            .unwrap_or_else(|| Tile::new(tile_type));
        let is_last_tile = self.wall.is_empty();
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut responded = [true; 4];

        for i in 0..4 {
            if i == caller {
                continue;
            }

            let player = &self.players[i];
            if !player.is_furiten() {
                let win_result = scoring::check_ron_with_flags_and_settings(
                    player,
                    called_tile,
                    self.round_wind,
                    is_last_tile,
                    true,
                    &self.settings,
                );
                if win_result.is_win {
                    available_calls[i].push(AvailableCall::Ron);
                    responded[i] = false;
                }
            }
        }

        let has_any_calls = available_calls.iter().any(|calls| !calls.is_empty());
        if has_any_calls {
            self.phase = TurnPhase::WaitForCalls;
            let caller_wind = self.players[caller].seat_wind;
            for (i, calls) in available_calls.iter().enumerate() {
                if !calls.is_empty() {
                    self.events.push((
                        i,
                        ServerEvent::CallAvailable {
                            tile: called_tile,
                            discarder: caller_wind,
                            calls: calls.clone(),
                        },
                    ));
                }
            }

            self.call_state = Some(CallState {
                discarded_tile: called_tile,
                discarder: caller,
                available_calls,
                responded,
                ron_declared: Vec::new(),
                pon_declared: None,
                daiminkan_declared: None,
                chi_declared: None,
                resolution: CallResolution::AfterKakan { caller, tile_type },
            });
        } else {
            self.execute_kakan(caller, tile_type);
        }
    }

    /// 暗カン/加カンを実行する
    pub fn do_kan(&mut self, tile_type: TileType) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player_idx = self.current_player;
        if self.players[player_idx].is_riichi {
            return false;
        }

        // 場全体で4回カン済みなら追加のカン不可
        if self.total_kan_count() >= 4 {
            return false;
        }

        if self.players[player_idx]
            .ankan_options()
            .contains(&tile_type)
        {
            self.players[player_idx].do_ankan(tile_type);
        } else if self.players[player_idx]
            .kakan_options()
            .contains(&tile_type)
        {
            self.check_kakan_ron_and_resolve(player_idx, tile_type);
            return true;
        } else {
            return false;
        }
        // ankan 確定時のみこの行以降が実行される（kakan/不可の場合は early return 済み）
        self.invalidate_first_turn_flags();

        let caller_wind = self.players[player_idx].seat_wind;
        let open = self.players[player_idx].hand.melds().last().unwrap();
        let tiles = open.expanded_tiles();
        let called_tile = Tile::new(tile_type);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Ankan,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.draw_after_kan(player_idx);
        true
    }

    /// 指定プレイヤーの最後の捨て牌を「鳴かれた」としてマークする
    fn mark_last_discard_as_called(&mut self, discarder: usize) {
        if let Some(last_discard) = self.players[discarder].discards.last_mut() {
            last_discard.is_called = true;
        }
    }

    /// 鳴き・カンなどにより全プレイヤーの一発フラグと
    /// 第1巡フラグ（四風連打の判定用）を無効化する
    fn invalidate_first_turn_flags(&mut self) {
        for player in &mut self.players {
            player.is_ippatsu = false;
            player.first_turn_interrupted = true;
        }
    }

    fn reveal_new_dora_indicator(&mut self) {
        self.wall.add_dora_indicator();
        let dora_indicators = self.wall.dora_indicators();
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::DoraIndicatorsUpdated {
                    dora_indicators: dora_indicators.clone(),
                },
            ));
        }
    }

    fn draw_after_kan(&mut self, player_idx: usize) {
        // 四槓散了チェック: 4回目のカン直後に判定（設定がありの場合のみ）
        if self.settings.four_kans_draw && self.check_four_kans_draw() {
            self.declare_special_draw(DrawReason::FourKans, None);
            return;
        }

        // 同巡フリテンを解除（嶺上ツモも自分のツモ番）
        self.players[player_idx].is_temporary_furiten = false;

        let Some(tile) = self.wall.draw_rinshan() else {
            self.do_exhaustive_draw();
            return;
        };

        self.current_player = player_idx;
        self.phase = TurnPhase::WaitForDiscard;
        self.last_draw_was_dead_wall = true;
        self.players[player_idx].draw(tile);

        self.push_draw_events(player_idx, tile, "kan_draw");
    }

    /// ツモ直後の通知イベントを積む
    ///
    /// 本人には牌と可能アクションを含む `TileDrawn`、
    /// 他プレイヤーには `OtherPlayerDrew` を送る。
    fn push_draw_events(&mut self, player_idx: usize, tile: Tile, diag_label: &str) {
        let remaining = self.wall.remaining();
        let can_tsumo = self.can_tsumo();
        let can_riichi = self.can_player_riichi(player_idx);
        #[cfg(debug_assertions)]
        self.log_draw_diagnostics(player_idx, diag_label, can_tsumo, can_riichi);
        #[cfg(not(debug_assertions))]
        let _ = diag_label;

        let is_furiten = self.players[player_idx].is_furiten();
        self.events.push((
            player_idx,
            ServerEvent::TileDrawn {
                tile,
                remaining_tiles: remaining,
                can_tsumo,
                can_riichi,
                is_furiten,
            },
        ));

        let current_wind = self.players[player_idx].seat_wind;
        for i in 0..4 {
            if i != player_idx {
                self.events.push((
                    i,
                    ServerEvent::OtherPlayerDrew {
                        player: current_wind,
                        remaining_tiles: remaining,
                    },
                ));
            }
        }
    }

    fn can_player_riichi_with_discard(&self, player_idx: usize, tile: Option<Tile>) -> bool {
        let player = &self.players[player_idx];
        let mut hand = player.hand.clone();

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
                if hand.drawn().is_none() {
                    return false;
                }
                hand.set_drawn(None);
            }
        }

        hand_analyzer::calc_shanten_number(&hand).is_ready()
    }

    /// プレイヤーがリーチ宣言可能か判定する
    ///
    /// 条件:
    /// - 門前（鳴いていない）
    /// - 持ち点が1000点以上
    /// - まだリーチしていない
    /// - 山に1枚以上残っている（打牌後に少なくとも1回はツモが行われる）
    /// - 14枚の手牌から、聴牌を維持する打牌が1つ以上ある
    fn can_player_riichi(&self, player_idx: usize) -> bool {
        let player = &self.players[player_idx];

        // デバッグビルドでは人間プレイヤー(idx=0)の却下理由を診断ログに残す
        let log_reject = |detail: std::fmt::Arguments| {
            if cfg!(debug_assertions) && player_idx == 0 {
                eprintln!("[riichi-reject] {detail}");
            }
        };

        if player.is_riichi {
            log_reject(format_args!("reason=already_riichi player={player_idx}"));
            return false;
        }
        if !player.is_menzen() {
            log_reject(format_args!("reason=not_menzen player={player_idx}"));
            return false;
        }
        if player.score < RIICHI_MIN_SCORE {
            log_reject(format_args!(
                "reason=score_too_low player={player_idx} score={}",
                player.score
            ));
            return false;
        }
        if self.wall.remaining() < 1 {
            log_reject(format_args!(
                "reason=wall_empty player={player_idx} remaining={}",
                self.wall.remaining()
            ));
            return false;
        }
        if player.hand.drawn().is_none() {
            log_reject(format_args!("reason=no_drawn player={player_idx}"));
            return false;
        }

        if self.can_player_riichi_with_discard(player_idx, None) {
            return true;
        }

        player
            .hand
            .tiles()
            .iter()
            .copied()
            .any(|tile| self.can_player_riichi_with_discard(player_idx, Some(tile)))
    }

    /// リーチ宣言を実行する
    ///
    /// リーチ宣言 + 打牌を同時に行う。
    /// tile で指定した牌を捨てた後、手牌が聴牌であることを確認する。
    /// tile が None の場合はツモ切りリーチ。
    pub fn do_riichi(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player_idx = self.current_player;

        // リーチ条件チェック
        if !self.can_player_riichi(player_idx) {
            return false;
        }
        if !self.can_player_riichi_with_discard(player_idx, tile) {
            return false;
        }

        // ダブルリーチ判定（第一ツモかつ副露による中断なし）
        let is_double = self.players[player_idx].is_first_turn
            && !self.players[player_idx].first_turn_interrupted;

        // リーチ宣言
        self.players[player_idx].declare_riichi(is_double);
        self.riichi_sticks += 1;

        // リーチ宣言牌を打牌
        // （declare_riichi内でippatsu=trueが設定されるが、
        //   直後のdiscardでippatsu=falseにされてしまう。
        //   これを防ぐため、一時的にippatsuを保護する）
        let is_tsumogiri = tile.is_none();
        let Some(discarded) = self.players[player_idx].try_discard(tile) else {
            self.players[player_idx].is_riichi = false;
            self.players[player_idx].is_double_riichi = false;
            self.players[player_idx].is_ippatsu = false;
            self.players[player_idx].score += RIICHI_STICK_VALUE;
            self.riichi_sticks = self.riichi_sticks.saturating_sub(1);
            return false;
        };
        // リーチ宣言直後の打牌なのでippatsuを復元
        self.players[player_idx].is_ippatsu = true;

        // 打牌をリーチ宣言牌としてマーク
        if let Some(last_discard) = self.players[player_idx].discards.last_mut() {
            last_discard.is_riichi_declaration = true;
        }

        // 全プレイヤーにリーチ通知
        let seat_wind = self.players[player_idx].seat_wind;
        let scores = self.get_scores();
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::PlayerRiichi {
                    player: seat_wind,
                    scores,
                    riichi_sticks: self.riichi_sticks,
                },
            ));
        }

        self.announce_discard_and_check_calls(discarded, player_idx, is_tsumogiri);

        true
    }

    /// 現在のプレイヤーがツモ和了できるか判定する
    pub fn can_tsumo(&self) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }
        let player = &self.players[self.current_player];
        let is_last_tile = self.wall.is_empty();
        let result = scoring::check_win_with_settings(
            player,
            self.round_wind,
            true,
            is_last_tile,
            self.last_draw_was_dead_wall,
            &self.settings,
        );
        result.is_win
    }

    /// ツモ和了を実行する
    /// 点数移動を行い、局を終了させる
    pub fn do_tsumo(&mut self) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player = &self.players[self.current_player];
        let is_last_tile = self.wall.is_empty();
        let win_result = scoring::check_win_with_settings(
            player,
            self.round_wind,
            true,
            is_last_tile,
            self.last_draw_was_dead_wall,
            &self.settings,
        );

        if !win_result.is_win {
            return false;
        }

        let Some(mut score_result) = win_result.score_result else {
            return false;
        };
        let winner = self.current_player;
        let Some(winning_tile) = self.players[winner].hand.drawn() else {
            return false;
        };
        let winner_is_dealer = self.players[winner].is_dealer();

        // ドラ・赤ドラ・裏ドラを加算
        let dora_indicators = self.wall.dora_indicators();
        let uradora_indicators = if self.players[winner].is_riichi {
            self.wall.uradora_indicators()
        } else {
            vec![]
        };
        scoring::add_dora_to_score(
            &mut score_result,
            &self.players[winner].hand,
            None,
            &dora_indicators,
            &uradora_indicators,
        );

        // 点数移動を計算
        let deltas = scoring::calculate_tsumo_score_deltas(
            winner,
            &score_result,
            winner_is_dealer,
            self.dealer,
            self.honba,
        );
        let riichi_sticks = self.riichi_sticks;

        // 点数を適用
        for (player, &delta) in self.players.iter_mut().zip(deltas.iter()) {
            player.score += delta;
        }
        if riichi_sticks > 0 {
            self.players[winner].score += (riichi_sticks as i32) * RIICHI_STICK_VALUE;
            self.riichi_sticks = 0;
        }

        let scores = self.get_scores();
        let winner_wind = self.players[winner].seat_wind;

        // 役情報を構築
        let yaku_list = score_result.yaku_list.clone();
        let rank = score_result.rank;
        let has_opened = score_result.has_opened;
        let player_hands = self.build_player_hands();

        // 全プレイヤーに和了イベントを送信
        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundWon {
                    winner: winner_wind,
                    loser: None, // ツモなのでloserなし
                    winning_tile,
                    scores,
                    yaku_list: yaku_list.clone(),
                    han: score_result.han,
                    fu: score_result.fu,
                    score_points: deltas[winner] + (riichi_sticks as i32) * RIICHI_STICK_VALUE,
                    rank,
                    has_opened,
                    uradora_indicators: uradora_indicators.clone(),
                    riichi_sticks,
                    player_hands: player_hands.clone(),
                },
            ));
        }

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::Tsumo {
            winner,
            winning_tile,
        });

        true
    }

    /// 自動プレイヤー（CPU）のターンを進める（ツモ切り）
    /// 現在のプレイヤーがツモ → ツモ切りを1ターン分行う
    pub fn advance_auto_player(&mut self) -> bool {
        if self.phase == TurnPhase::RoundOver {
            return false;
        }

        // ツモ
        if !self.do_draw() {
            return false;
        }

        // 流局チェック
        if self.phase == TurnPhase::RoundOver {
            return true;
        }

        // ツモ切り
        self.do_discard(None)
    }

    /// 局が終了したかどうか
    pub fn is_over(&self) -> bool {
        self.phase == TurnPhase::RoundOver
    }

    /// 荒牌流局を処理する（ノーテン罰符を含む）
    fn do_exhaustive_draw(&mut self) {
        // テンパイ判定
        let mut tenpai_players = Vec::new();
        let mut noten_players = Vec::new();

        for i in 0..4 {
            if scoring::is_ready(&self.players[i]) {
                tenpai_players.push(i);
            } else {
                noten_players.push(i);
            }
        }

        // ノーテン罰符の計算（テンパイ者と非テンパイ者がいる場合のみ）
        if !tenpai_players.is_empty() && !noten_players.is_empty() {
            let total_penalty = 3000i32;
            let tenpai_count = tenpai_players.len() as i32;
            let noten_count = noten_players.len() as i32;

            let gain_each = total_penalty / tenpai_count;
            let loss_each = total_penalty / noten_count;

            for &i in &tenpai_players {
                self.players[i].score += gain_each;
            }
            for &i in &noten_players {
                self.players[i].score -= loss_each;
            }
        }

        let scores = self.get_scores();
        let tenpai_winds: Vec<Wind> = tenpai_players
            .iter()
            .map(|&i| self.players[i].seat_wind)
            .collect();

        let dealer_tenpai = tenpai_players.contains(&self.dealer);
        let player_hands = self.build_player_hands();

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::ExhaustiveDraw { dealer_tenpai });

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: DrawReason::Exhaustive,
                    tenpai: tenpai_winds.clone(),
                    riichi_sticks: self.riichi_sticks,
                    player_hands: player_hands.clone(),
                    declarer: None,
                },
            ));
        }
    }

    /// 特殊流局をチェックする（四風連打、四家立直）
    fn check_special_draws(&mut self) {
        // 四風連打チェック: 全員が1枚ずつ捨てて、全て同じ風牌
        if self.settings.four_winds_draw && self.check_four_winds_draw() {
            self.declare_special_draw(DrawReason::FourWinds, None);
            return;
        }

        // 四家立直チェック: 全員がリーチ宣言済み
        if self.settings.four_riichi_draw && self.check_four_riichi_draw() {
            self.declare_special_draw(DrawReason::FourRiichi, None);
        }
    }

    /// 四風連打を判定する
    ///
    /// 条件: 各プレイヤーがちょうど1枚ずつ捨てており、
    /// 全て同じ風牌で、鳴きが発生していない
    fn check_four_winds_draw(&self) -> bool {
        // 全プレイヤーがちょうど1枚捨てていること
        for player in &self.players {
            if player.discards.len() != 1 {
                return false;
            }
            // 鳴かれていたら不成立
            if player.discards[0].is_called {
                return false;
            }
        }

        // 全て同じ風牌であること
        let first_tile = self.players[0].discards[0].tile;
        if !first_tile.is_wind() {
            return false;
        }

        self.players
            .iter()
            .all(|p| p.discards[0].tile.get() == first_tile.get())
    }

    /// 四家立直を判定する
    ///
    /// 条件: 全4プレイヤーがリーチ宣言済み
    fn check_four_riichi_draw(&self) -> bool {
        self.players.iter().all(|p| p.is_riichi)
    }

    /// 九種九牌の宣言条件を判定する
    ///
    /// 条件: 現在のプレイヤーが一度も捨牌しておらず、
    /// 手牌＋ツモ牌に9種類以上のヤオ九牌（老頭牌・字牌）がある
    fn check_nine_terminals(&self) -> bool {
        let player = &self.players[self.current_player];
        // 初回ツモのみ（捨牌済みなら宣言不可）
        if !player.discards.is_empty() {
            return false;
        }
        let mut tile_types = std::collections::HashSet::new();
        for tile in player.hand.tiles() {
            if tile.is_1_9_honour() {
                tile_types.insert(tile.get());
            }
        }
        if let Some(tile) = player.hand.drawn()
            && tile.is_1_9_honour()
        {
            tile_types.insert(tile.get());
        }
        tile_types.len() >= 9
    }

    /// 九種九牌の宣言を処理する
    ///
    /// - `declare=true`: 流局を宣言する
    /// - `declare=false`: 続行する（通常の打牌フェーズへ移行）
    pub fn do_nine_terminals(&mut self, player_idx: usize, declare: bool) -> bool {
        if self.phase != TurnPhase::WaitForNineTerminals {
            return false;
        }
        if self.current_player != player_idx {
            return false;
        }
        if declare {
            let declarer_wind = self.players[player_idx].seat_wind;
            self.declare_special_draw(DrawReason::NineTerminals, Some(declarer_wind));
        } else {
            self.phase = TurnPhase::WaitForDiscard;

            // 続行を選んだプレイヤーに TileDrawn を再送して打牌を促す。
            // 最初の TileDrawn への応答（打牌）は WaitForNineTerminals
            // フェーズで拒否されているため、再送しないとクライアントが
            // 打牌の機会を得られず局が進行しなくなる。
            if let Some(drawn) = self.players[player_idx].hand.drawn() {
                let can_tsumo = self.can_tsumo();
                let can_riichi = self.can_player_riichi(player_idx);
                let is_furiten = self.players[player_idx].is_furiten();
                self.events.push((
                    player_idx,
                    ServerEvent::TileDrawn {
                        tile: drawn,
                        remaining_tiles: self.wall.remaining(),
                        can_tsumo,
                        can_riichi,
                        is_furiten,
                    },
                ));
            }
        }
        true
    }

    /// 場全体のカン回数を返す
    fn total_kan_count(&self) -> usize {
        self.players.iter().map(|p| p.kan_count()).sum()
    }

    /// 四槓散了を判定する
    ///
    /// 条件: 場全体で4回カンが成立し、かつ2人以上がカンしている
    /// （1人が4回カンした場合は四槓子の可能性があるため続行）
    fn check_four_kans_draw(&self) -> bool {
        if self.total_kan_count() < 4 {
            return false;
        }
        let players_with_kan = self.players.iter().filter(|p| p.kan_count() > 0).count();
        players_with_kan >= 2
    }

    /// 特殊流局を宣言する
    fn declare_special_draw(&mut self, reason: DrawReason, declarer: Option<Wind>) {
        let scores = self.get_scores();
        let player_hands = self.build_player_hands();
        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::SpecialDraw);

        for i in 0..4 {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: reason.clone(),
                    tenpai: Vec::new(),
                    riichi_sticks: self.riichi_sticks,
                    player_hands: player_hands.clone(),
                    declarer,
                },
            ));
        }
    }
}

/// 鳴き応答の種類
#[derive(Debug, Clone)]
pub enum CallResponse {
    /// ロン
    Ron,
    /// ポン（手牌から使う牌2枚。赤ドラも区別する）
    Pon { hand_tile_types: [Tile; 2] },
    /// 大明カン
    Daiminkan,
    /// チー（手牌から使う牌2枚。赤ドラも区別する）
    Chi { hand_tile_types: [Tile; 2] },
    /// パス
    Pass,
}

#[cfg(test)]
mod tests;
