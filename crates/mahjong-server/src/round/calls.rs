//! Call detection and resolution (ron, pon, kan, chii).

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType};
use mahjong_core::winning_hand::name::Kind;

use crate::player::Player;
use crate::protocol::{AvailableCall, CallType, DrawReason, ServerEvent};
use crate::scoring;

use super::{
    CallResolution, CallResponse, CallState, RIICHI_STICK_VALUE, Round, RoundResult, TurnPhase,
};

/// Whether applying this call leaves at least one legal discard under
/// the swap-calling rule.
fn call_leaves_legal_discard(
    player: &Player,
    called_tile: Tile,
    hand_tiles: [Tile; 2],
    category: MeldType,
) -> bool {
    let mut remaining = player.hand.tiles().to_vec();
    for target in hand_tiles {
        let Some(position) = remaining.iter().position(|tile| *tile == target) else {
            return false;
        };
        remaining.remove(position);
    }

    let mut meld_tiles = vec![called_tile, hand_tiles[0], hand_tiles[1]];
    meld_tiles.sort();
    let forbidden = Meld {
        tiles: meld_tiles,
        category,
        from: MeldFrom::Unknown,
        called_tile: Some(called_tile),
    }
    .forbidden_swap_tiles();

    remaining
        .iter()
        .any(|tile| !forbidden.contains(&tile.get()))
}

impl Round {
    /// Announces a discard, checks call options, and advances the phase.
    ///
    /// Shared post-discard flow for `do_discard` and `do_riichi`.
    pub(super) fn announce_discard_and_check_calls(
        &mut self,
        discarded: Tile,
        discarder: usize,
        is_tsumogiri: bool,
        hand_index: Option<usize>,
    ) {
        let discarder_wind = self.players[discarder].seat_wind;
        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::TileDiscarded {
                    player: discarder_wind,
                    tile: discarded,
                    is_tsumogiri,
                    hand_index,
                },
            ));
        }

        let call_state = self.check_available_calls(discarded, discarder);
        let has_any_calls = call_state.available_calls.iter().any(|c| !c.is_empty());

        if has_any_calls {
            self.phase = TurnPhase::WaitForCalls;

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
            self.current_player = self.next_seat(discarder);
            self.phase = TurnPhase::Draw;

            self.check_special_draws();
        }
    }

    /// Collects every player's call options for a discard.
    pub(super) fn check_available_calls(
        &self,
        discarded_tile: Tile,
        discarder: usize,
    ) -> CallState {
        let wall_exhausted = self.wall.is_empty();
        // A discard after a replacement draw is followed by exhaustion, but
        // it is not the last live-wall discard and cannot award Houtei.
        let is_last_tile_claim = wall_exhausted && !self.last_draw_was_dead_wall;
        let mut available_calls: [Vec<AvailableCall>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        // Players with no options (and the dummy seat) count as already
        // responded, so resolution waits only on real choices.
        let mut responded = [true; 4];

        for i in 0..self.player_count {
            if i == discarder {
                continue;
            }

            let player = &self.players[i];

            // Furiten blocks ron entirely; riichi players may still ron.
            if !player.is_furiten() {
                let win_result = scoring::check_ron_with_settings(
                    player,
                    discarded_tile,
                    self.round_wind,
                    is_last_tile_claim,
                    &self.settings,
                );
                if win_result.is_win {
                    available_calls[i].push(AvailableCall::Ron);
                }
            }

            // A riichi player cannot call anything except ron. The final
            // discard likewise has no following turn in which a meld caller
            // could discard, so only ron remains legal.
            if player.is_riichi || wall_exhausted {
                if !available_calls[i].is_empty() {
                    responded[i] = false;
                }
                continue;
            }

            let mut pon_opts = player.pon_options(discarded_tile);
            if self.settings.forbid_swap_calling {
                pon_opts.retain(|option| {
                    call_leaves_legal_discard(player, discarded_tile, *option, MeldType::Pon)
                });
            }
            if !pon_opts.is_empty() {
                available_calls[i].push(AvailableCall::Pon { options: pon_opts });
            }

            // No further quads once four exist on the table.
            if self.total_kan_count() < 4 && player.can_daiminkan(discarded_tile) {
                available_calls[i].push(AvailableCall::Daiminkan);
            }

            // Chii is only allowed from the left player (= the next seat)
            // and does not exist in three-player games.
            let next_player = self.next_seat(discarder);
            if !self.settings.three_player && i == next_player {
                let mut chi_opts = player.chi_options(discarded_tile);
                if self.settings.forbid_swap_calling {
                    chi_opts.retain(|option| {
                        call_leaves_legal_discard(player, discarded_tile, *option, MeldType::Chi)
                    });
                }
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

    /// Accepts a player's call response (Ron/Pon/Chi/Pass) and, once all
    /// responses are in, resolves the calls by priority.
    pub fn respond_to_call(&mut self, player_idx: usize, response: CallResponse) -> bool {
        if self.phase != TurnPhase::WaitForCalls {
            return false;
        }
        if player_idx >= self.player_count {
            return false;
        }

        let call_state = match self.call_state.as_mut() {
            Some(cs) => cs,
            None => return false,
        };

        if call_state.responded[player_idx] {
            return false;
        }

        match response {
            CallResponse::Ron => {
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
                let valid = call_state.available_calls[player_idx].iter().any(|c| {
                    if let AvailableCall::Pon { options } = c {
                        options.contains(&hand_tile_types)
                    } else {
                        false
                    }
                });
                if valid {
                    // A later network response must not overwrite the first
                    // caller at the same call priority.
                    if call_state.pon_declared.is_none() {
                        call_state.pon_declared = Some((player_idx, hand_tile_types));
                    }
                } else {
                    return false;
                }
            }
            CallResponse::Daiminkan => {
                if call_state.available_calls[player_idx]
                    .iter()
                    .any(|c| matches!(c, AvailableCall::Daiminkan))
                {
                    // A later network response must not overwrite the first
                    // caller at the same call priority.
                    if call_state.daiminkan_declared.is_none() {
                        call_state.daiminkan_declared = Some(player_idx);
                    }
                } else {
                    return false;
                }
            }
            CallResponse::Chi { hand_tile_types } => {
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
            CallResponse::Pass => {}
        }

        call_state.responded[player_idx] = true;

        if call_state.responded.iter().all(|&r| r) {
            self.resolve_calls();
        }

        true
    }

    /// Resolves the calls, in priority order:
    /// ron > called quad > pon > chii > pass.
    fn resolve_calls(&mut self) {
        let call_state = self.call_state.take().unwrap();

        // Passing on an available ron makes the player furiten.
        for i in 0..self.player_count {
            let had_ron = call_state.available_calls[i]
                .iter()
                .any(|c| matches!(c, AvailableCall::Ron));
            let declared_ron = call_state.ron_declared.contains(&i);

            if had_ron && !declared_ron {
                if self.players[i].is_riichi {
                    // Permanent for the rest of the hand under riichi.
                    self.players[i].is_riichi_furiten = true;
                } else {
                    // Otherwise only until the player's own next draw.
                    self.players[i].is_temporary_furiten = true;
                }
            }
        }

        if !call_state.ron_declared.is_empty() {
            let is_robbing_a_quad =
                matches!(call_state.resolution, CallResolution::AfterKakan { .. });
            let discarder = call_state.discarder;
            let winning_tile = call_state.discarded_tile;
            let ron_count = call_state.ron_declared.len();

            // Sort by turn-order priority from the discarder
            // (right, across, left).
            let mut sorted_winners = call_state.ron_declared.clone();
            sorted_winners
                .sort_by_key(|&p| (p + self.player_count - discarder) % self.player_count);

            if ron_count >= 3 && self.settings.triple_ron_draw {
                // The triple-ron abortive draw outranks everything else.
                self.declare_special_draw(DrawReason::TripleRon, None);
                return;
            }

            let winners = if ron_count >= 2 && self.settings.multiple_ron {
                sorted_winners
            } else {
                // Head bump: only the player closest in turn order wins.
                vec![sorted_winners[0]]
            };

            self.execute_ron(winners, discarder, winning_tile, is_robbing_a_quad);
            return;
        }

        if let CallResolution::AfterKakan { caller, tile_type } = call_state.resolution {
            self.execute_kakan(caller, tile_type);
            return;
        }

        if let Some(caller) = call_state.daiminkan_declared {
            self.execute_daiminkan(caller, call_state.discarder, call_state.discarded_tile);
            return;
        }

        if let Some((caller, hand_tile_types)) = call_state.pon_declared {
            self.execute_pon(
                caller,
                call_state.discarder,
                call_state.discarded_tile,
                hand_tile_types,
            );
            return;
        }

        if let Some((caller, hand_tile_types)) = call_state.chi_declared {
            self.execute_chi(
                caller,
                call_state.discarder,
                call_state.discarded_tile,
                hand_tile_types,
            );
            return;
        }

        // Everyone passed; move on to the next player.
        self.current_player = self.next_seat(call_state.discarder);
        self.phase = TurnPhase::Draw;

        self.check_special_draws();
    }

    /// Executes a ron win, covering single, double, and triple ron.
    ///
    /// - `winners` is sorted by turn-order priority from the discarder.
    /// - The honba bonus and riichi deposits go only to the first winner
    ///   in turn order.
    pub(super) fn execute_ron(
        &mut self,
        winners: Vec<usize>,
        loser: usize,
        winning_tile: Tile,
        is_robbing_a_quad: bool,
    ) {
        let is_last_tile = self.wall.is_empty() && !self.last_draw_was_dead_wall;
        let dora_indicators = self.wall.dora_indicators();
        let riichi_sticks = self.riichi_sticks;
        let player_hands = self.build_player_hands();

        struct WinnerData {
            winner: usize,
            score_result: mahjong_core::scoring::score::ScoreResult,
            deltas: [i32; 4],
            uradora_indicators: Vec<Tile>,
            score_points: i32,
            honba: usize,
            honba_points: i32,
        }

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
            // Liability payment (pao / 包): the liable player splits the
            // payment with the deal-in player when a qualifying yakuman
            // was completed.
            let pao_players = self.pao_players_for_win(winner, &score_result.yaku_list);
            let deltas = if pao_players.is_empty() {
                scoring::calculate_ron_score_deltas(
                    winner,
                    loser,
                    &score_result,
                    winner_is_dealer,
                    honba_for_this,
                )
            } else {
                scoring::calculate_ron_score_deltas_with_pao_players(
                    winner,
                    loser,
                    &pao_players,
                    &score_result,
                    winner_is_dealer,
                    honba_for_this,
                )
            };

            let riichi_bonus = if winner_data.is_empty() {
                (riichi_sticks as i32) * RIICHI_STICK_VALUE
            } else {
                0
            };
            let score_points = deltas[winner] + riichi_bonus;
            let honba_points = honba_for_this as i32 * 300;

            winner_data.push(WinnerData {
                winner,
                score_result,
                deltas,
                uradora_indicators,
                score_points,
                honba: honba_for_this,
                honba_points,
            });
        }

        // Safety net: if no declared ron actually validates,
        // keep the game moving instead of stalling.
        if winner_data.is_empty() {
            self.current_player = self.next_seat(loser);
            self.phase = TurnPhase::Draw;
            return;
        }

        for wd in &winner_data {
            for i in 0..self.player_count {
                self.players[i].score += wd.deltas[i];
            }
        }
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
                        honba: wd.honba,
                        honba_points: wd.honba_points,
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

    /// Records a liability payment (pao / 包) when a call locks in
    /// a yakuman.
    ///
    /// Called right after a pon or called quad. The discarder who fed the
    /// call becomes liable when it completes:
    /// - Big Dragons: the third dragon triplet
    /// - Big Winds: the fourth wind triplet
    /// - Four Quads: the fourth quad via a called quad
    fn record_pao_if_confirmed(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        is_daiminkan: bool,
    ) {
        if !self.settings.yakuman_pao {
            return;
        }

        let tt = called_tile.get();
        let count_melds_in = |range: std::ops::RangeInclusive<TileType>| {
            self.players[caller]
                .hand
                .melds()
                .iter()
                .filter(|meld| meld.category != MeldType::Chi)
                .filter(|meld| range.contains(&meld.tiles[0].get()))
                .count()
        };
        let dragon_meld_count = count_melds_in(Tile::Z5..=Tile::Z7);
        let wind_meld_count = count_melds_in(Tile::Z1..=Tile::Z4);

        if (Tile::Z5..=Tile::Z7).contains(&tt) && dragon_meld_count == 3 {
            self.pao[caller].push((Kind::BigDragons, discarder));
        }
        if (Tile::Z1..=Tile::Z4).contains(&tt) && wind_meld_count == 4 {
            self.pao[caller].push((Kind::BigWinds, discarder));
        }
        if is_daiminkan && self.players[caller].kan_count() == 4 {
            self.pao[caller].push((Kind::FourQuads, discarder));
        }
    }

    /// Executes a pon.
    pub(super) fn execute_pon(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        let from = Player::meld_from_relative(caller, discarder, self.player_count);
        self.players[caller].do_pon(called_tile, hand_tile_types, from);
        self.record_pao_if_confirmed(caller, discarder, called_tile, false);

        self.mark_last_discard_as_called(discarder);

        self.invalidate_first_turn_flags();

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

        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        self.apply_swap_call_restriction(caller);
        self.current_player = caller;
        self.phase = TurnPhase::WaitForDiscard;
    }

    /// Executes a called quad (daiminkan / 大明槓).
    pub(super) fn execute_daiminkan(&mut self, caller: usize, discarder: usize, called_tile: Tile) {
        let from = Player::meld_from_relative(caller, discarder, self.player_count);
        self.players[caller].do_daiminkan(called_tile, from);
        self.record_pao_if_confirmed(caller, discarder, called_tile, true);

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

    /// Executes a chii.
    pub(super) fn execute_chi(
        &mut self,
        caller: usize,
        discarder: usize,
        called_tile: Tile,
        hand_tile_types: [Tile; 2],
    ) {
        self.players[caller].do_chi(called_tile, hand_tile_types);

        self.mark_last_discard_as_called(discarder);

        self.invalidate_first_turn_flags();

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

        self.events.push((
            caller,
            ServerEvent::HandUpdated {
                hand: self.players[caller].hand.tiles().to_vec(),
            },
        ));

        self.apply_swap_call_restriction(caller);
        self.current_player = caller;
        self.phase = TurnPhase::WaitForDiscard;
    }

    /// Applies the swap-calling (kuikae) discard restriction to the caller
    /// when the rule is enabled.
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
