//! 鳴き（ロン・ポン・カン・チー）の判定と解決

use mahjong_core::tile::{Tile, TileType};

use crate::player::Player;
use crate::protocol::{AvailableCall, CallType, DrawReason, ServerEvent};
use crate::scoring;

use super::{
    CallResolution, CallResponse, CallState, RIICHI_STICK_VALUE, Round, RoundResult, TurnPhase,
};

impl Round {
    /// 打牌の通知と鳴き候補チェックを行い、フェーズを遷移させる
    ///
    /// `do_discard` と `do_riichi` で共通の打牌後処理。
    pub(super) fn announce_discard_and_check_calls(
        &mut self,
        discarded: Tile,
        discarder: usize,
        is_tsumogiri: bool,
    ) {
        // 全プレイヤーに打牌を通知
        let discarder_wind = self.players[discarder].seat_wind;
        for i in 0..self.player_count {
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
            for i in 0..self.player_count {
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
            self.current_player = self.next_seat(discarder);
            self.phase = TurnPhase::Draw;

            // 特殊流局チェック（四家立直チェック含む）
            self.check_special_draws();
        }
    }

    /// 打牌後の鳴き候補を全てチェックする
    pub(super) fn check_available_calls(
        &self,
        discarded_tile: Tile,
        discarder: usize,
    ) -> CallState {
        let is_last_tile = self.wall.is_empty();
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut responded = [true; 4]; // デフォルトは応答済み（対象外・ダミー席含む）

        for i in 0..self.player_count {
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

            // チー判定（上家からのみ＝次のプレイヤー。三麻ではチーなし）
            let next_player = self.next_seat(discarder);
            if !self.settings.three_player && i == next_player {
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
        for i in 0..self.player_count {
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
            sorted_winners
                .sort_by_key(|&p| (p + self.player_count - discarder) % self.player_count);

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
        self.current_player = self.next_seat(call_state.discarder);
        self.phase = TurnPhase::Draw;

        // 特殊流局チェック
        self.check_special_draws();
    }

    /// ロン和了を実行する（通常・ダブロン・トリロン共通）
    ///
    /// - winners: ロン和了者の打順優先順（下家→対面→上家）でソート済みのインデックスリスト
    /// - 本場ボーナスと供託棒は最初の和了者（打順最優先）のみが取得する
    pub(super) fn execute_ron(
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
                &self.players[winner].pei_tiles,
                self.settings.three_player,
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
            self.current_player = self.next_seat(loser);
            self.phase = TurnPhase::Draw;
            return;
        }

        // 全スコアデルタを合算して適用
        for wd in &winner_data {
            for i in 0..self.player_count {
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

            for i in 0..self.player_count {
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
    pub(super) fn execute_pon(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        let from = Player::meld_from_relative(caller, discarder, self.player_count);
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

        for i in 0..self.player_count {
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
    pub(super) fn execute_daiminkan(&mut self, caller: usize, discarder: usize, called_tile: Tile) {
        let from = Player::meld_from_relative(caller, discarder, self.player_count);
        self.players[caller].do_daiminkan(called_tile, from);

        self.mark_last_discard_as_called(discarder);
        self.invalidate_first_turn_flags();

        let caller_wind = self.players[caller].seat_wind;
        let open = self.players[caller].hand.melds().last().unwrap();
        let tiles = open.expanded_tiles();

        for i in 0..self.player_count {
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
    pub(super) fn execute_chi(
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

        for i in 0..self.player_count {
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

        for i in 0..self.player_count {
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

    pub(super) fn check_kakan_ron_and_resolve(&mut self, caller: usize, tile_type: TileType) {
        let called_tile = self.players[caller]
            .kakan_added_tile(tile_type)
            .unwrap_or_else(|| Tile::new(tile_type));
        let is_last_tile = self.wall.is_empty();
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut responded = [true; 4];

        for i in 0..self.player_count {
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
}
