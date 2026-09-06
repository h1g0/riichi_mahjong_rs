//! Turn flow: draws, discards, kans, and North extraction.

use mahjong_core::tile::{Tile, TileType};

use crate::protocol::{CallType, DrawReason, ServerEvent};

use super::{Round, TurnPhase};

impl Round {
    /// Runs the draw phase: takes one tile from the wall for the
    /// current player.
    pub fn do_draw(&mut self) -> bool {
        if self.phase != TurnPhase::Draw {
            return false;
        }

        // Temporary furiten lasts only until the player's own next draw.
        self.players[self.current_player].is_temporary_furiten = false;

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

        // Offer the nine-terminals abortive draw when eligible.
        if self.settings.nine_terminals_draw && self.check_nine_terminals() {
            self.phase = TurnPhase::WaitForNineTerminals;
            self.events
                .push((self.current_player, ServerEvent::NineTerminalsAvailable));
        }

        true
    }

    /// Discards a tile (`None` discards the drawn tile), then checks the
    /// other players' call options and moves to the WaitForCalls phase
    /// when any exist.
    pub fn do_discard(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let hand_index = self.discard_hand_index(self.current_player, tile);
        let Some(discarded) = self.players[self.current_player].try_discard(tile) else {
            return false;
        };

        // try_discard() already cleared the ippatsu flag; the riichi
        // declaration discard goes through do_riichi(), which restores it.
        self.announce_discard_and_check_calls(
            discarded,
            self.current_player,
            tile.is_none(),
            hand_index,
        );

        true
    }

    /// Executes a concealed or promoted kan.
    pub fn do_kan(&mut self, tile_type: TileType) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }
        if !crate::legality::kan_replacement_available(
            self.wall.remaining(),
            self.total_kan_count(),
        ) {
            return false;
        }

        let player_idx = self.current_player;
        if self.players[player_idx].is_riichi {
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
        // Only the ankan path reaches this line; kakan and rejections
        // returned early above.
        self.invalidate_first_turn_flags();

        let caller_wind = self.players[player_idx].seat_wind;
        let open = self.players[player_idx].hand.melds().last().unwrap();
        let tiles = open.expanded_tiles();
        let called_tile = Tile::new(tile_type);

        for i in 0..self.player_count {
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

    /// Extracts a North tile as pei dora (three-player games).
    ///
    /// - A turn action, not a call, so ippatsu and first-turn flags survive.
    /// - Unlike a kan, no new dora indicator is revealed.
    /// - The replacement comes directly from the live wall's tail, keeping
    ///   `remaining()` and last-tile bookkeeping consistent.
    /// - Winning on the replacement draw is not After a Quad (嶺上開花).
    pub fn do_pei(&mut self) -> bool {
        if !(self.settings.three_player && self.settings.nuki_dora) {
            return false;
        }
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }
        if self.wall.is_empty() {
            return false;
        }

        let player_idx = self.current_player;
        if !self.players[player_idx].do_pei() {
            return false;
        }

        // Counts are indexed by wind so clients need no seat mapping.
        let declarer_wind = self.players[player_idx].seat_wind;
        let mut pei_counts = [0u8; 4];
        for p in self.players.iter().take(self.player_count) {
            pei_counts[p.seat_wind.to_index()] = p.pei_tiles.len() as u8;
        }
        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::PeiDeclared {
                    player: declarer_wind,
                    pei_counts,
                },
            ));
        }

        // The North may have come from the hand, so resync it.
        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        let Some(tile) = self.wall.draw_replacement_from_tail() else {
            return false;
        };
        self.players[player_idx].draw(tile);
        self.last_draw_was_dead_wall = false;
        self.push_draw_events(player_idx, tile, "pei_draw");
        true
    }

    /// Returns the 0-based position of a hand discard within the sorted
    /// hand (excluding the drawn tile).
    ///
    /// Uses the same lookup as `Player::try_discard` (first exact match),
    /// run before the discard. None on tsumogiri or when the tile is absent.
    pub(super) fn discard_hand_index(
        &self,
        player_idx: usize,
        tile: Option<Tile>,
    ) -> Option<usize> {
        let target = tile?;
        self.players[player_idx]
            .hand
            .tiles()
            .iter()
            .position(|t| *t == target)
    }

    /// Marks the player's latest discard as claimed by a call.
    pub(super) fn mark_last_discard_as_called(&mut self, discarder: usize) {
        if let Some(last_discard) = self.players[discarder].discards.last_mut() {
            last_discard.is_called = true;
        }
    }

    /// A call or kan breaks every player's ippatsu and interrupts the
    /// first go-around (which also cancels the four-winds abortive draw).
    pub(super) fn invalidate_first_turn_flags(&mut self) {
        for player in &mut self.players {
            player.is_ippatsu = false;
            player.first_turn_interrupted = true;
        }
    }

    pub(super) fn reveal_new_dora_indicator(&mut self) {
        self.wall.add_dora_indicator();
        let dora_indicators = self.wall.dora_indicators();
        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::DoraIndicatorsUpdated {
                    dora_indicators: dora_indicators.clone(),
                },
            ));
        }
    }

    pub(super) fn draw_after_kan(&mut self, player_idx: usize) {
        // The four-quads abortive draw is checked right after the fourth kan.
        if self.settings.four_kans_draw && self.check_four_kans_draw() {
            self.declare_special_draw(DrawReason::FourKans, None);
            return;
        }

        // A replacement draw is still the player's own draw,
        // so temporary furiten ends here too.
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

    /// Sends the authoritative hand back to a player whose discard or
    /// other own-turn action was rejected.
    ///
    /// Clients apply discards to their local hand optimistically before
    /// sending, so a silent rejection leaves the client's hand out of sync;
    /// every later discard of that tile then keeps getting rejected and the
    /// game appears frozen (#294). `HandUpdated` restores the hand, and if
    /// it is the player's turn with a drawn tile, `TileDrawn` is re-sent so
    /// they can discard again (to that player only; no `OtherPlayerDrew`).
    pub(crate) fn resync_hand(&mut self, player_idx: usize) {
        if player_idx >= self.player_count {
            return;
        }

        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        if self.phase == TurnPhase::WaitForDiscard
            && self.current_player == player_idx
            && let Some(drawn) = self.players[player_idx].hand.drawn()
        {
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

    /// Queues the post-draw notifications: `TileDrawn` with the tile and
    /// available actions to the drawer, `OtherPlayerDrew` to everyone else.
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
        for i in 0..self.player_count {
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
}
