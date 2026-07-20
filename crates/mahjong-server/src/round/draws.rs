//! Draw handling: exhaustive draws and abortive draws.

use mahjong_core::tile::Wind;

use crate::protocol::{DrawReason, NagashiManganWinner, ServerEvent};
use crate::scoring;

use super::{RIICHI_STICK_VALUE, Round, RoundResult, TurnPhase};

impl Round {
    /// Handles an exhaustive draw, including the noten penalty.
    pub(super) fn do_exhaustive_draw(&mut self) {
        if self.settings.nagashi_mangan {
            let nagashi_winners = self.nagashi_mangan_players();
            if !nagashi_winners.is_empty() && self.do_nagashi_mangan(nagashi_winners) {
                return;
            }
        }

        let mut tenpai_players = Vec::new();
        let mut noten_players = Vec::new();

        for i in 0..self.player_count {
            if scoring::is_ready(&self.players[i]) {
                tenpai_players.push(i);
            } else {
                noten_players.push(i);
            }
        }

        // The 3000-point noten penalty applies only when both tenpai and
        // noten players exist.
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

        for i in 0..self.player_count {
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

    /// Players whose entire non-empty discard pool consists of unclaimed
    /// terminals and honours. Calling another player's tile is allowed; the
    /// defining condition concerns only the player's own discard pool.
    pub(super) fn nagashi_mangan_players(&self) -> Vec<usize> {
        let mut winners: Vec<usize> = self
            .players
            .iter()
            .enumerate()
            .take(self.player_count)
            .filter_map(|(seat, player)| {
                (!player.discards.is_empty()
                    && player
                        .discards
                        .iter()
                        .all(|discard| discard.tile.is_1_9_honour() && !discard.is_called))
                .then_some(seat)
            })
            .collect();
        winners.sort_by_key(|&seat| (seat + self.player_count - self.dealer) % self.player_count);
        winners
    }

    /// Settles one or more simultaneous Nagashi Mangan wins.
    ///
    /// Each winner receives an independent tsumo-mangan payment. As with
    /// multiple ron, only the first winner receives continuance bonuses and
    /// riichi deposits.
    fn do_nagashi_mangan(&mut self, winners: Vec<usize>) -> bool {
        struct WinnerData {
            seat: usize,
            deltas: [i32; 4],
            score_points: i32,
        }

        let riichi_sticks = self.riichi_sticks;
        let mut winner_data = Vec::with_capacity(winners.len());
        for &winner in &winners {
            let result = scoring::check_nagashi_mangan(
                &self.players[winner],
                self.round_wind,
                &self.settings,
            );
            let Some(score_result) = result.score_result else {
                continue;
            };
            let is_first_winner = winner_data.is_empty();
            let honba = if is_first_winner { self.honba } else { 0 };
            let deltas = scoring::calculate_tsumo_score_deltas(
                winner,
                &score_result,
                self.players[winner].is_dealer(),
                self.dealer,
                honba,
                self.player_count,
                self.settings.tsumo_loss,
            );
            let riichi_bonus = if is_first_winner {
                riichi_sticks as i32 * RIICHI_STICK_VALUE
            } else {
                0
            };
            winner_data.push(WinnerData {
                seat: winner,
                score_points: deltas[winner] + riichi_bonus,
                deltas,
            });
        }

        if winner_data.is_empty() {
            return false;
        }

        for data in &winner_data {
            for (player, delta) in self.players.iter_mut().zip(data.deltas) {
                player.score += delta;
            }
        }
        if riichi_sticks > 0 {
            self.players[winner_data[0].seat].score += riichi_sticks as i32 * RIICHI_STICK_VALUE;
            self.riichi_sticks = 0;
        }

        let event_winners: Vec<NagashiManganWinner> = winner_data
            .iter()
            .map(|data| NagashiManganWinner {
                wind: self.players[data.seat].seat_wind,
                score_points: data.score_points,
            })
            .collect();
        let result_winners = winner_data.iter().map(|data| data.seat).collect();
        let scores = self.get_scores();
        let player_hands = self.build_player_hands();
        for seat in 0..self.player_count {
            self.events.push((
                seat,
                ServerEvent::RoundNagashiMangan {
                    winners: event_winners.clone(),
                    scores,
                    riichi_sticks,
                    player_hands: player_hands.clone(),
                },
            ));
        }

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::NagashiMangan {
            winners: result_winners,
        });
        true
    }

    /// Checks for abortive draws: four winds (四風連打) and four riichi (四家立直).
    pub(super) fn check_special_draws(&mut self) {
        if self.settings.four_winds_draw && self.check_four_winds_draw() {
            self.declare_special_draw(DrawReason::FourWinds, None);
            return;
        }

        if self.settings.four_riichi_draw && self.check_four_riichi_draw() {
            self.declare_special_draw(DrawReason::FourRiichi, None);
        }
    }

    /// Whether the four-winds abortive draw applies: every player has
    /// discarded exactly one tile, all the same wind, with no calls made.
    pub(super) fn check_four_winds_draw(&self) -> bool {
        let players = &self.players[..self.player_count];
        for player in players {
            if player.discards.len() != 1 {
                return false;
            }
            if player.discards[0].is_called {
                return false;
            }
        }

        let first_tile = players[0].discards[0].tile;
        if !first_tile.is_wind() {
            return false;
        }

        players
            .iter()
            .all(|p| p.discards[0].tile.get() == first_tile.get())
    }

    /// Whether the four-riichi abortive draw applies: every active player
    /// has declared riichi.
    pub(super) fn check_four_riichi_draw(&self) -> bool {
        self.players[..self.player_count]
            .iter()
            .all(|p| p.is_riichi)
    }

    /// Whether the current player may declare a nine-terminals abortive
    /// draw: no discard made yet, and the hand plus the drawn tile holds
    /// nine or more distinct terminal/honour kinds.
    pub(super) fn check_nine_terminals(&self) -> bool {
        let player = &self.players[self.current_player];
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

    /// Handles the nine-terminals declaration.
    ///
    /// - `declare=true`: abort the hand.
    /// - `declare=false`: continue, moving on to the normal discard phase.
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

            // Re-send TileDrawn to prompt the discard: the response to the
            // first TileDrawn was rejected while in the WaitForNineTerminals
            // phase, so without a re-send the client never gets a chance to
            // discard and the hand stalls.
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

    /// Total number of quads declared by all players.
    pub(super) fn total_kan_count(&self) -> usize {
        self.players.iter().map(|p| p.kan_count()).sum()
    }

    /// Whether the four-quads abortive draw applies: four quads declared by
    /// two or more players. A single player holding all four quads may be
    /// heading for the Four Quads yakuman, so play continues.
    pub(super) fn check_four_kans_draw(&self) -> bool {
        if self.total_kan_count() < 4 {
            return false;
        }
        let players_with_kan = self.players.iter().filter(|p| p.kan_count() > 0).count();
        players_with_kan >= 2
    }

    /// Declares an abortive draw.
    pub(super) fn declare_special_draw(&mut self, reason: DrawReason, declarer: Option<Wind>) {
        let scores = self.get_scores();
        let player_hands = self.build_player_hands();
        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::SpecialDraw);

        for i in 0..self.player_count {
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
