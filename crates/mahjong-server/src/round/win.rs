//! リーチ宣言とツモ和了

use mahjong_core::hand_info::hand_analyzer;
use mahjong_core::tile::Tile;

use crate::protocol::ServerEvent;
use crate::scoring;

use super::{RIICHI_MIN_SCORE, RIICHI_STICK_VALUE, Round, RoundResult, TurnPhase};

impl Round {
    pub(super) fn can_player_riichi_with_discard(
        &self,
        player_idx: usize,
        tile: Option<Tile>,
    ) -> bool {
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
    pub(super) fn can_player_riichi(&self, player_idx: usize) -> bool {
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
        // 手出しなら打牌前のソート済み手牌内での位置を控える（他家の手牌演出用）
        let hand_index = self.discard_hand_index(player_idx, tile);
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
        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::PlayerRiichi {
                    player: seat_wind,
                    scores,
                    riichi_sticks: self.riichi_sticks,
                },
            ));
        }

        self.announce_discard_and_check_calls(discarded, player_idx, is_tsumogiri, hand_index);

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
            &self.players[winner].pei_tiles,
            self.settings.three_player,
        );

        // 点数移動を計算（三麻はツモ損: いない北家分は貰えない）
        let mut deltas = scoring::calculate_tsumo_score_deltas(
            winner,
            &score_result,
            winner_is_dealer,
            self.dealer,
            self.honba,
            self.player_count,
        );
        // 包（責任払い）: 対象役満が成立していれば包のプレイヤーが全額を支払う
        if let Some(pao_player) = self.pao_player_for_win(winner, &score_result.yaku_list) {
            scoring::apply_pao_to_tsumo_deltas(&mut deltas, winner, pao_player);
        }
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
        for i in 0..self.player_count {
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
}
