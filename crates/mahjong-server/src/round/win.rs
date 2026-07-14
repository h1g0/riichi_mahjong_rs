//! Riichi declaration and tsumo wins.

use mahjong_core::hand_info::hand_analyzer;
use mahjong_core::tile::Tile;

use crate::protocol::ServerEvent;
use crate::scoring;

use super::{
    RIICHI_MIN_SCORE, RIICHI_STICK_VALUE, Round, RoundResult, TurnPhase, diagnostics_enabled,
};

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

    /// Whether the player may declare riichi.
    ///
    /// Requirements:
    /// - closed hand
    /// - at least 1000 points (for the deposit)
    /// - has not already declared riichi
    /// - at least one tile left in the wall, so at least one more draw
    ///   happens after the declaration discard
    /// - at least one discard from the 14 tiles keeps the hand tenpai
    pub(super) fn can_player_riichi(&self, player_idx: usize) -> bool {
        let player = &self.players[player_idx];

        // Verbose rejection logging is opt-in because this check runs for
        // every discard candidate and otherwise dominates debug simulations.
        let log_reject = |detail: std::fmt::Arguments| {
            if player_idx == 0 && diagnostics_enabled() {
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

    /// Declares riichi.
    ///
    /// Declaration and discard happen together: the hand must be tenpai
    /// after discarding `tile`. `None` discards the drawn tile.
    pub fn do_riichi(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player_idx = self.current_player;

        if !self.can_player_riichi(player_idx) {
            return false;
        }
        if !self.can_player_riichi_with_discard(player_idx, tile) {
            return false;
        }

        // Double riichi requires the very first draw with no interrupting call.
        let is_double = self.players[player_idx].is_first_turn
            && !self.players[player_idx].first_turn_interrupted;

        self.players[player_idx].declare_riichi(is_double);
        self.riichi_sticks += 1;

        let is_tsumogiri = tile.is_none();
        let hand_index = self.discard_hand_index(player_idx, tile);
        let Some(discarded) = self.players[player_idx].try_discard(tile) else {
            self.players[player_idx].is_riichi = false;
            self.players[player_idx].is_double_riichi = false;
            self.players[player_idx].is_ippatsu = false;
            self.players[player_idx].score += RIICHI_STICK_VALUE;
            self.riichi_sticks = self.riichi_sticks.saturating_sub(1);
            return false;
        };
        // try_discard() clears the ippatsu flag on every discard, but the
        // riichi declaration discard must keep it; restore it here.
        self.players[player_idx].is_ippatsu = true;

        if let Some(last_discard) = self.players[player_idx].discards.last_mut() {
            last_discard.is_riichi_declaration = true;
        }

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

    /// Whether the current player can win by tsumo.
    pub fn can_tsumo(&self) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }
        let player = &self.players[self.current_player];
        let is_last_tile = self.wall.is_empty() && !self.last_draw_was_dead_wall;
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

    /// Executes a tsumo win: applies the payments and ends the hand.
    pub fn do_tsumo(&mut self) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player = &self.players[self.current_player];
        let is_last_tile = self.wall.is_empty() && !self.last_draw_was_dead_wall;
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

        // Three-player games use tsumo loss: the absent player's share
        // is simply not received.
        let pao_players = self.pao_players_for_win(winner, &score_result.yaku_list);
        let deltas = scoring::calculate_tsumo_score_deltas_with_pao(
            winner,
            &score_result,
            winner_is_dealer,
            self.dealer,
            self.honba,
            self.player_count,
            &pao_players,
        );
        let riichi_sticks = self.riichi_sticks;

        for (player, &delta) in self.players.iter_mut().zip(deltas.iter()) {
            player.score += delta;
        }
        if riichi_sticks > 0 {
            self.players[winner].score += (riichi_sticks as i32) * RIICHI_STICK_VALUE;
            self.riichi_sticks = 0;
        }

        let scores = self.get_scores();
        let winner_wind = self.players[winner].seat_wind;

        let yaku_list = score_result.yaku_list.clone();
        let rank = score_result.rank;
        let has_opened = score_result.has_opened;
        let player_hands = self.build_player_hands();

        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::RoundWon {
                    winner: winner_wind,
                    loser: None,
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
