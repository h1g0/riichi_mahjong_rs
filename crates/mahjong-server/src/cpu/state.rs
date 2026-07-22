//! The game state a CPU maintains.
//!
//! Reconstructed from the `ServerEvent` stream plus immutable table rules:
//! it holds only information available to a human player.

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, Wind};

use crate::protocol::{AvailableCall, CallType, ServerEvent};

/// CPU-side game state, built entirely from ServerEvents.
#[derive(Debug, Clone)]
pub struct CpuGameState {
    // --- Own info ---
    /// Own hand
    pub my_hand: Vec<Tile>,
    /// The drawn tile
    pub my_drawn: Option<Tile>,
    /// Own seat wind
    pub my_seat_wind: Wind,
    /// Whether we have declared riichi
    pub is_riichi: bool,

    // --- Flags carried by TileDrawn ---
    /// Whether tsumo is possible
    pub can_tsumo: bool,
    /// Whether riichi may be declared
    pub can_riichi: bool,
    /// Whether we are furiten
    pub is_furiten: bool,

    // --- Public info for all players ---
    /// Scores
    pub scores: [i32; 4],
    /// Discards per player, indexed by wind (East = 0, ...)
    pub all_discards: [Vec<Tile>; 4],
    /// Discards that were called and now also appear in a meld
    called_discards: Vec<Tile>,
    /// Riichi state per player
    pub player_riichi: [bool; 4],
    /// Melds per player
    pub player_melds: [Vec<Meld>; 4],
    /// Dora indicators
    pub dora_indicators: Vec<Tile>,
    /// Round wind
    pub round_wind: Wind,
    /// Tiles left in the live wall
    pub remaining_tiles: usize,
    /// Hand number (East 1 = 0, East 2 = 1, ...)
    pub round_number: usize,
    /// Hands in the whole game (0 = unknown)
    pub total_rounds: usize,
    /// Continuance counter (honba)
    pub honba: usize,
    /// Riichi deposits on the table
    pub riichi_sticks: usize,
    /// Three-player game: affects the tile set and dora chain
    pub three_player: bool,
    /// Whether pei dora is enabled (three-player only)
    pub nuki_dora: bool,
    /// Whether All Inside is valid after opening the hand
    pub opened_all_inside: bool,
    /// Whether special double-yakuman variants are enabled
    pub double_yakuman: bool,
    /// Extracted North count per player, indexed by wind
    pub pei_counts: [u8; 4],

    // --- Pending calls ---
    /// Calls currently available to us
    pub pending_calls: Vec<AvailableCall>,
    /// The tile the calls are on
    pub pending_call_tile: Option<Tile>,

    // --- Post-call discard flags ---
    /// Whether a discard is due after our call
    pub need_discard_after_call: bool,
    /// Whether the last call was a kan (waiting for the replacement draw)
    pub pending_kan_draw: bool,
    /// Whether our own pon/chii succeeded and the next HandUpdated should
    /// trigger a discard decision.
    ///
    /// HandUpdated is also used to resync after a rejected discard (#294),
    /// so receiving one does not by itself mean a discard is due; only a
    /// HandUpdated following our own pon/chii PlayerCalled does.
    pub pending_call_discard: bool,
}

impl CpuGameState {
    pub fn new() -> Self {
        CpuGameState {
            my_hand: Vec::new(),
            my_drawn: None,
            my_seat_wind: Wind::East,
            is_riichi: false,
            can_tsumo: false,
            can_riichi: false,
            is_furiten: false,
            scores: [0; 4],
            all_discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            called_discards: Vec::new(),
            player_riichi: [false; 4],
            player_melds: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            dora_indicators: Vec::new(),
            round_wind: Wind::East,
            remaining_tiles: 0,
            round_number: 0,
            total_rounds: 0,
            honba: 0,
            riichi_sticks: 0,
            three_player: false,
            nuki_dora: false,
            opened_all_inside: true,
            double_yakuman: true,
            pei_counts: [0; 4],
            pending_calls: Vec::new(),
            pending_call_tile: None,
            need_discard_after_call: false,
            pending_kan_draw: false,
            pending_call_discard: false,
        }
    }

    pub fn player_count(&self) -> usize {
        if self.three_player { 3 } else { 4 }
    }

    /// Maps a wind to a player index (East = 0, ..., North = 3).
    pub fn wind_to_index(wind: Wind) -> usize {
        match wind {
            Wind::East => 0,
            Wind::South => 1,
            Wind::West => 2,
            Wind::North => 3,
        }
    }

    /// Applies a ServerEvent to the state.
    pub fn update(&mut self, event: &ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                round_wind,
                dora_indicators,
                round_number,
                total_rounds,
                honba,
                riichi_sticks,
                three_player,
                nuki_dora,
                ..
            } => {
                // A new hand: reset everything.
                self.my_hand = hand.clone();
                self.my_drawn = None;
                self.my_seat_wind = *seat_wind;
                self.is_riichi = false;
                self.can_tsumo = false;
                self.can_riichi = false;
                self.is_furiten = false;
                self.scores = *scores;
                self.all_discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.called_discards.clear();
                self.player_riichi = [false; 4];
                self.player_melds = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.dora_indicators = dora_indicators.clone();
                self.round_wind = *round_wind;
                self.three_player = *three_player;
                self.nuki_dora = *nuki_dora;
                self.pei_counts = [0; 4];
                // Four-player: 136 - 14 dead wall - 4 x 13 dealt = 70.
                // Three-player: 108 - 14 - 3 x 13 = 55.
                self.remaining_tiles = if *three_player { 55 } else { 70 };
                self.round_number = *round_number;
                self.total_rounds = *total_rounds;
                self.honba = *honba;
                self.riichi_sticks = *riichi_sticks;
                self.pending_calls.clear();
                self.pending_call_tile = None;
                self.need_discard_after_call = false;
                self.pending_kan_draw = false;
                self.pending_call_discard = false;
            }

            ServerEvent::TileDrawn {
                tile,
                remaining_tiles,
                can_tsumo,
                can_riichi,
                is_furiten,
            } => {
                self.my_drawn = Some(*tile);
                self.remaining_tiles = *remaining_tiles;
                self.can_tsumo = *can_tsumo;
                self.can_riichi = *can_riichi;
                self.is_furiten = *is_furiten;
                self.need_discard_after_call = false;
                self.pending_call_discard = false;
            }

            ServerEvent::OtherPlayerDrew {
                remaining_tiles, ..
            } => {
                self.remaining_tiles = *remaining_tiles;
            }

            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
                ..
            } => {
                let idx = Self::wind_to_index(*player);
                self.all_discards[idx].push(*tile);

                if *player == self.my_seat_wind {
                    if *is_tsumogiri {
                        self.my_drawn = None;
                    } else {
                        // Hand discard: remove the tile and merge in the
                        // drawn tile.
                        if let Some(pos) = self.my_hand.iter().position(|t| *t == *tile) {
                            self.my_hand.remove(pos);
                        }
                        if let Some(drawn) = self.my_drawn {
                            self.my_hand.push(drawn);
                            self.my_hand.sort();
                        }
                        self.my_drawn = None;
                    }
                }
            }

            ServerEvent::CallAvailable { tile, calls, .. } => {
                self.pending_calls = calls.clone();
                self.pending_call_tile = Some(*tile);
            }

            ServerEvent::PlayerCalled {
                player,
                call_type,
                tiles,
                called_tile,
                ..
            } => {
                let idx = Self::wind_to_index(*player);
                let category = match call_type {
                    CallType::Chi => MeldType::Chi,
                    CallType::Pon => MeldType::Pon,
                    CallType::Ankan | CallType::Daiminkan => MeldType::Kan,
                    CallType::Kakan => MeldType::Kakan,
                    CallType::Ron => MeldType::Pon, // Unused fallback.
                };
                let from = match call_type {
                    CallType::Ankan => MeldFrom::Myself,
                    _ => MeldFrom::Unknown,
                };
                if matches!(
                    call_type,
                    CallType::Chi | CallType::Pon | CallType::Daiminkan
                ) {
                    self.called_discards.push(*called_tile);
                }

                if *call_type == CallType::Kakan {
                    if let Some(meld) = self.player_melds[idx].iter_mut().find(|meld| {
                        meld.category == MeldType::Pon
                            && meld.tiles.first().map(|tile| tile.get()) == Some(called_tile.get())
                    }) {
                        meld.category = MeldType::Kakan;
                        meld.tiles = tiles.clone();
                        meld.called_tile = Some(*called_tile);
                    } else {
                        self.player_melds[idx].push(Meld {
                            tiles: tiles.clone(),
                            category,
                            from,
                            called_tile: Some(*called_tile),
                        });
                    }
                } else {
                    self.player_melds[idx].push(Meld {
                        tiles: tiles.clone(),
                        category,
                        from,
                        called_tile: Some(*called_tile),
                    });
                }

                self.pending_calls.clear();
                self.pending_call_tile = None;
                self.pending_kan_draw = matches!(
                    call_type,
                    CallType::Ankan | CallType::Daiminkan | CallType::Kakan
                );
                // Our own pon/chii means the next HandUpdated needs a
                // discard decision.
                self.pending_call_discard = *player == self.my_seat_wind
                    && matches!(call_type, CallType::Pon | CallType::Chi);
            }

            ServerEvent::PlayerRiichi {
                player,
                scores,
                riichi_sticks,
            } => {
                let idx = Self::wind_to_index(*player);
                self.player_riichi[idx] = true;
                self.scores = *scores;
                self.riichi_sticks = *riichi_sticks;
                if *player == self.my_seat_wind {
                    self.is_riichi = true;
                }
            }

            ServerEvent::DoraIndicatorsUpdated { dora_indicators } => {
                self.dora_indicators = dora_indicators.clone();
            }

            ServerEvent::PeiDeclared { player, pei_counts } => {
                self.pei_counts = *pei_counts;
                if *player == self.my_seat_wind {
                    // Our own extraction: no discard until the replacement
                    // TileDrawn arrives, so the HandUpdated that follows
                    // must not trigger one.
                    self.pending_kan_draw = true;
                }
            }

            ServerEvent::HandUpdated { hand } => {
                self.my_hand = hand.clone();
                self.my_drawn = None;
                if self.pending_kan_draw {
                    // After a kan, wait for the replacement TileDrawn.
                    self.pending_kan_draw = false;
                } else if self.pending_call_discard {
                    self.pending_call_discard = false;
                    self.need_discard_after_call = true;
                }
                // Any other HandUpdated is a pure resync (e.g. after a
                // rejected discard, #294) and must NOT trigger a discard:
                // doing so loops reject -> resync -> re-discard -> reject
                // forever and stalls the hand.
            }

            ServerEvent::RoundWon { scores, .. } => {
                self.scores = *scores;
            }

            ServerEvent::RoundNagashiMangan { scores, .. } => {
                self.scores = *scores;
            }

            ServerEvent::RoundDraw { scores, .. } => {
                self.scores = *scores;
            }

            ServerEvent::NineTerminalsAvailable => {
                // No state change; decide_nine_terminals handles the reply.
            }
        }
    }

    pub fn my_discards(&self) -> &[Tile] {
        &self.all_discards[Self::wind_to_index(self.my_seat_wind)]
    }

    /// Whether this is the final hand; false when total_rounds is unknown.
    pub fn is_final_round(&self) -> bool {
        self.total_rounds > 0 && self.round_number + 1 >= self.total_rounds
    }

    /// Whether the game is in its second half; false when unknown.
    pub fn is_second_half(&self) -> bool {
        self.total_rounds > 0 && self.round_number >= self.total_rounds / 2
    }

    pub fn my_score(&self) -> i32 {
        self.scores[Self::wind_to_index(self.my_seat_wind)]
    }

    /// Whether we are in (possibly shared) first place.
    pub fn is_top(&self) -> bool {
        let my = self.my_score();
        self.scores.iter().all(|&s| s <= my)
    }

    /// Minimum points needed to climb one place; None when leading.
    pub fn gap_to_next_rank(&self) -> Option<i32> {
        let my = self.my_score();
        self.scores
            .iter()
            .filter(|&&s| s > my)
            .map(|&s| s - my)
            .min()
    }

    pub fn my_melds(&self) -> &[Meld] {
        &self.player_melds[Self::wind_to_index(self.my_seat_wind)]
    }

    /// Own melds with quads truncated to three tiles, as `HandAnalyzer`
    /// expects; use this for shanten computations.
    pub fn my_melds_for_analysis(&self) -> Vec<Meld> {
        self.my_melds()
            .iter()
            .map(|open| {
                let mut o = open.clone();
                if o.tiles.len() > 3 {
                    o.tiles.truncate(3);
                }
                o
            })
            .collect()
    }

    /// The current turn number, 1-based, derived from our discard count.
    /// Called before discarding, it is the turn about to be played.
    pub fn turn(&self) -> usize {
        self.my_discards().len() + 1
    }

    /// Counts visible tiles per kind: own hand, everyone's discards,
    /// everyone's melds, and the dora indicators.
    pub fn visible_tile_counts(&self) -> [u8; 34] {
        let mut counts = [0u8; 34];

        for tile in &self.my_hand {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.my_drawn {
            counts[drawn.get() as usize] += 1;
        }

        for discards in &self.all_discards {
            for tile in discards {
                counts[tile.get() as usize] += 1;
            }
        }

        for melds in &self.player_melds {
            for meld in melds {
                for tile in &meld.tiles {
                    counts[tile.get() as usize] += 1;
                }
            }
        }

        for tile in &self.dora_indicators {
            counts[tile.get() as usize] += 1;
        }

        // Called discards appear both in the discard pool and in a meld;
        // subtract once to avoid double counting.
        for tile in &self.called_discards {
            let count = &mut counts[tile.get() as usize];
            *count = count.saturating_sub(1);
        }

        // Extracted North tiles appear in neither hands nor discards,
        // so add them separately.
        let total_pei: u8 = self.pei_counts.iter().sum();
        counts[Tile::Z4 as usize] = (counts[Tile::Z4 as usize] + total_pei).min(4);

        // In three-player games treat the nonexistent 2m-8m as fully
        // visible, so acceptance counts (4 - visible) and dead-tile checks
        // (visible >= 4) never treat them as live tiles.
        if self.three_player {
            for tile_type in Tile::M2..=Tile::M8 {
                counts[tile_type as usize] = 4;
            }
        }

        counts
    }
}

impl Default for CpuGameState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DrawReason, ServerEvent};
    use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
    use mahjong_core::scoring::score::ScoreRank;
    use mahjong_core::tile::{Tile, Wind};

    #[test]
    fn test_default_state_and_wind_to_index() {
        let state = CpuGameState::default();

        assert!(state.my_hand.is_empty());
        assert_eq!(state.my_drawn, None);
        assert_eq!(state.my_seat_wind, Wind::East);
        assert_eq!(state.scores, [0; 4]);
        assert!(state.opened_all_inside);
        assert_eq!(CpuGameState::wind_to_index(Wind::East), 0);
        assert_eq!(CpuGameState::wind_to_index(Wind::South), 1);
        assert_eq!(CpuGameState::wind_to_index(Wind::West), 2);
        assert_eq!(CpuGameState::wind_to_index(Wind::North), 3);
    }

    #[test]
    fn test_game_started_initializes_state() {
        let mut state = CpuGameState::new();
        let hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];

        state.update(&ServerEvent::GameStarted {
            seat_wind: Wind::South,
            hand: hand.clone(),
            scores: [25000; 4],
            round_wind: Wind::East,
            dora_indicators: vec![Tile::new(Tile::M5)],
            round_number: 0,
            total_rounds: 0,
            honba: 0,
            riichi_sticks: 0,
            three_player: false,
            nuki_dora: false,
        });

        assert_eq!(state.my_seat_wind, Wind::South);
        assert_eq!(state.my_hand, hand);
        assert_eq!(state.scores, [25000; 4]);
        assert_eq!(state.round_wind, Wind::East);
        assert_eq!(state.dora_indicators.len(), 1);
        assert_eq!(state.round_number, 0);
    }

    #[test]
    fn test_game_started_resets_existing_round_state() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![Tile::new(Tile::M1)];
        state.my_drawn = Some(Tile::new(Tile::P1));
        state.my_seat_wind = Wind::West;
        state.is_riichi = true;
        state.can_tsumo = true;
        state.can_riichi = true;
        state.is_furiten = true;
        state.scores = [1000, 2000, 3000, 4000];
        state.all_discards[0].push(Tile::new(Tile::M9));
        state.called_discards.push(Tile::new(Tile::M9));
        state.player_riichi = [true; 4];
        state.player_melds[0].push(Meld {
            tiles: vec![Tile::new(Tile::S1); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::S1)),
        });
        state.dora_indicators = vec![Tile::new(Tile::P9)];
        state.round_wind = Wind::South;
        state.remaining_tiles = 12;
        state.round_number = 9;
        state.honba = 3;
        state.riichi_sticks = 2;
        state.pending_calls = vec![AvailableCall::Ron];
        state.pending_call_tile = Some(Tile::new(Tile::M2));
        state.need_discard_after_call = true;
        state.pending_kan_draw = true;

        let hand = vec![Tile::new(Tile::S2), Tile::new(Tile::S3)];
        state.update(&ServerEvent::GameStarted {
            seat_wind: Wind::North,
            hand: hand.clone(),
            scores: [25000; 4],
            round_wind: Wind::East,
            dora_indicators: vec![Tile::new(Tile::Z1)],
            round_number: 4,
            total_rounds: 4,
            honba: 1,
            riichi_sticks: 1,
            three_player: false,
            nuki_dora: false,
        });

        assert_eq!(state.my_hand, hand);
        assert_eq!(state.my_drawn, None);
        assert_eq!(state.my_seat_wind, Wind::North);
        assert!(!state.is_riichi);
        assert!(!state.can_tsumo);
        assert!(!state.can_riichi);
        assert!(!state.is_furiten);
        assert_eq!(state.scores, [25000; 4]);
        assert!(state.all_discards.iter().all(Vec::is_empty));
        assert!(state.called_discards.is_empty());
        assert_eq!(state.player_riichi, [false; 4]);
        assert!(state.player_melds.iter().all(Vec::is_empty));
        assert_eq!(state.dora_indicators, vec![Tile::new(Tile::Z1)]);
        assert_eq!(state.round_wind, Wind::East);
        assert_eq!(state.remaining_tiles, 70);
        assert_eq!(state.round_number, 4);
        assert_eq!(state.honba, 1);
        assert_eq!(state.riichi_sticks, 1);
        assert!(state.pending_calls.is_empty());
        assert_eq!(state.pending_call_tile, None);
        assert!(!state.need_discard_after_call);
        assert!(!state.pending_kan_draw);
    }

    #[test]
    fn test_tile_drawn_updates_state() {
        let mut state = CpuGameState::new();
        state.update(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::P5),
            remaining_tiles: 50,
            can_tsumo: false,
            can_riichi: true,
            is_furiten: false,
        });

        assert_eq!(state.my_drawn, Some(Tile::new(Tile::P5)));
        assert_eq!(state.remaining_tiles, 50);
        assert!(!state.can_tsumo);
        assert!(state.can_riichi);
    }

    #[test]
    fn test_tile_drawn_clears_post_call_discard_flag() {
        let mut state = CpuGameState::new();
        state.need_discard_after_call = true;

        state.update(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::S5),
            remaining_tiles: 12,
            can_tsumo: true,
            can_riichi: false,
            is_furiten: true,
        });

        assert_eq!(state.my_drawn, Some(Tile::new(Tile::S5)));
        assert_eq!(state.remaining_tiles, 12);
        assert!(state.can_tsumo);
        assert!(!state.can_riichi);
        assert!(state.is_furiten);
        assert!(!state.need_discard_after_call);
    }

    #[test]
    fn test_other_player_drew_updates_remaining_tiles() {
        let mut state = CpuGameState::new();

        state.update(&ServerEvent::OtherPlayerDrew {
            player: Wind::West,
            remaining_tiles: 33,
        });

        assert_eq!(state.remaining_tiles, 33);
    }

    #[test]
    fn test_tile_discarded_updates_discards() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;

        state.update(&ServerEvent::TileDiscarded {
            player: Wind::South,
            tile: Tile::new(Tile::Z1),
            is_tsumogiri: false,
            hand_index: None,
        });

        assert_eq!(state.all_discards[1].len(), 1);
        assert_eq!(state.all_discards[1][0], Tile::new(Tile::Z1));
    }

    #[test]
    fn test_self_discard_without_drawn_tile_removes_from_hand_only() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];

        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::M2),
            is_tsumogiri: false,
            hand_index: None,
        });

        assert_eq!(state.my_drawn, None);
        assert_eq!(
            state.my_hand,
            vec![Tile::new(Tile::M1), Tile::new(Tile::M3)]
        );
        assert_eq!(state.all_discards[0], vec![Tile::new(Tile::M2)]);
    }

    #[test]
    fn test_call_available_records_pending_calls() {
        let mut state = CpuGameState::new();
        let option = [Tile::new(Tile::M1), Tile::new(Tile::M1)];

        state.update(&ServerEvent::CallAvailable {
            tile: Tile::new(Tile::M1),
            discarder: Wind::North,
            calls: vec![
                AvailableCall::Ron,
                AvailableCall::Pon {
                    options: vec![option],
                },
            ],
        });

        assert_eq!(state.pending_call_tile, Some(Tile::new(Tile::M1)));
        assert_eq!(state.pending_calls.len(), 2);
        assert!(matches!(state.pending_calls[0], AvailableCall::Ron));
        match &state.pending_calls[1] {
            AvailableCall::Pon { options } => assert_eq!(options, &vec![option]),
            call => panic!("expected pon call, got {call:?}"),
        }
    }

    #[test]
    fn test_player_called_variants_update_melds_and_kan_state() {
        let mut state = CpuGameState::new();
        state.pending_calls = vec![AvailableCall::Ron];
        state.pending_call_tile = Some(Tile::new(Tile::M2));

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::South,
            call_type: CallType::Chi,
            called_tile: Tile::new(Tile::M2),
            tiles: vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
            ],
        });

        let chi = &state.player_melds[1][0];
        assert_eq!(chi.category, MeldType::Chi);
        assert_eq!(chi.from, MeldFrom::Unknown);
        assert_eq!(state.called_discards, vec![Tile::new(Tile::M2)]);
        assert!(state.pending_calls.is_empty());
        assert_eq!(state.pending_call_tile, None);
        assert!(!state.pending_kan_draw);

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::West,
            call_type: CallType::Daiminkan,
            called_tile: Tile::new(Tile::P4),
            tiles: vec![Tile::new(Tile::P4); 4],
        });

        let daiminkan = &state.player_melds[2][0];
        assert_eq!(daiminkan.category, MeldType::Kan);
        assert_eq!(daiminkan.from, MeldFrom::Unknown);
        assert_eq!(
            state.called_discards,
            vec![Tile::new(Tile::M2), Tile::new(Tile::P4)]
        );
        assert!(state.pending_kan_draw);

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::North,
            call_type: CallType::Ankan,
            called_tile: Tile::new(Tile::S7),
            tiles: vec![Tile::new(Tile::S7); 4],
        });

        let ankan = &state.player_melds[3][0];
        assert_eq!(ankan.category, MeldType::Kan);
        assert_eq!(ankan.from, MeldFrom::Myself);
        assert_eq!(state.called_discards.len(), 2);
        assert!(state.pending_kan_draw);

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::East,
            call_type: CallType::Ron,
            called_tile: Tile::new(Tile::Z1),
            tiles: vec![Tile::new(Tile::Z1); 3],
        });

        let ron_fallback = &state.player_melds[0][0];
        assert_eq!(ron_fallback.category, MeldType::Pon);
        assert_eq!(state.called_discards.len(), 2);
        assert!(!state.pending_kan_draw);
    }

    #[test]
    fn test_player_riichi_updates_state() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;

        state.update(&ServerEvent::PlayerRiichi {
            player: Wind::East,
            scores: [24000, 25000, 25000, 25000],
            riichi_sticks: 1,
        });

        assert!(state.is_riichi);
        assert!(state.player_riichi[0]);
        assert_eq!(state.scores[0], 24000);
        assert_eq!(state.riichi_sticks, 1);
    }

    #[test]
    fn test_other_player_riichi_does_not_mark_self_riichi() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::North;

        state.update(&ServerEvent::PlayerRiichi {
            player: Wind::South,
            scores: [25000, 24000, 25000, 25000],
            riichi_sticks: 1,
        });

        assert!(!state.is_riichi);
        assert!(state.player_riichi[1]);
        assert_eq!(state.scores, [25000, 24000, 25000, 25000]);
        assert_eq!(state.riichi_sticks, 1);
    }

    #[test]
    fn test_dora_indicators_updated_replaces_indicators() {
        let mut state = CpuGameState::new();
        state.dora_indicators = vec![Tile::new(Tile::M1)];

        state.update(&ServerEvent::DoraIndicatorsUpdated {
            dora_indicators: vec![Tile::new(Tile::P9), Tile::new(Tile::Z5)],
        });

        assert_eq!(
            state.dora_indicators,
            vec![Tile::new(Tile::P9), Tile::new(Tile::Z5)]
        );
    }

    #[test]
    fn test_hand_updated_after_open_call_requires_discard() {
        let mut state = CpuGameState::new();
        state.my_drawn = Some(Tile::new(Tile::S9));
        // State right after our own pon/chii PlayerCalled.
        state.pending_call_discard = true;

        state.update(&ServerEvent::HandUpdated {
            hand: vec![Tile::new(Tile::M1), Tile::new(Tile::M2)],
        });

        assert_eq!(
            state.my_hand,
            vec![Tile::new(Tile::M1), Tile::new(Tile::M2)]
        );
        assert_eq!(state.my_drawn, None);
        assert!(state.need_discard_after_call);
        assert!(!state.pending_call_discard);
        assert!(!state.pending_kan_draw);
    }

    /// Regression: a HandUpdated without our own call (the post-rejection
    /// resync, #294) must not trigger a discard. Discarding here looped
    /// reject -> resync -> re-discard -> reject and stalled hands under
    /// CPU substitution.
    #[test]
    fn test_hand_updated_without_own_call_is_resync_only() {
        let mut state = CpuGameState::new();
        state.my_drawn = Some(Tile::new(Tile::S9));

        state.update(&ServerEvent::HandUpdated {
            hand: vec![Tile::new(Tile::M1), Tile::new(Tile::M2)],
        });

        assert_eq!(
            state.my_hand,
            vec![Tile::new(Tile::M1), Tile::new(Tile::M2)]
        );
        assert_eq!(state.my_drawn, None);
        assert!(!state.need_discard_after_call);
    }

    /// Only our own pon/chii PlayerCalled may set pending_call_discard.
    #[test]
    fn test_pending_call_discard_set_only_by_own_open_call() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::South;

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::West,
            call_type: CallType::Pon,
            called_tile: Tile::new(Tile::M2),
            tiles: vec![Tile::new(Tile::M2); 3],
        });
        assert!(!state.pending_call_discard);

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::South,
            call_type: CallType::Pon,
            called_tile: Tile::new(Tile::M3),
            tiles: vec![Tile::new(Tile::M3); 3],
        });
        assert!(state.pending_call_discard);

        // Our own kan waits for the replacement draw instead.
        state.update(&ServerEvent::PlayerCalled {
            player: Wind::South,
            call_type: CallType::Ankan,
            called_tile: Tile::new(Tile::S7),
            tiles: vec![Tile::new(Tile::S7); 4],
        });
        assert!(!state.pending_call_discard);
        assert!(state.pending_kan_draw);
    }

    #[test]
    fn test_hand_updated_after_kan_waits_for_rinshan_draw() {
        let mut state = CpuGameState::new();
        state.my_drawn = Some(Tile::new(Tile::S9));
        state.pending_kan_draw = true;

        state.update(&ServerEvent::HandUpdated {
            hand: vec![Tile::new(Tile::P1), Tile::new(Tile::P2)],
        });

        assert_eq!(
            state.my_hand,
            vec![Tile::new(Tile::P1), Tile::new(Tile::P2)]
        );
        assert_eq!(state.my_drawn, None);
        assert!(!state.need_discard_after_call);
        assert!(!state.pending_kan_draw);
    }

    #[test]
    fn test_round_end_events_update_scores() {
        let mut state = CpuGameState::new();

        state.update(&ServerEvent::RoundWon {
            winner: Wind::East,
            loser: Some(Wind::South),
            winning_tile: Tile::new(Tile::M1),
            scores: [35000, 15000, 25000, 25000],
            yaku_list: Vec::new(),
            han: 1,
            fu: 30,
            score_points: 1000,
            rank: ScoreRank::Normal,
            has_opened: false,
            uradora_indicators: Vec::new(),
            riichi_sticks: 0,
            honba: 0,
            honba_points: 0,
            player_hands: Vec::new(),
        });
        assert_eq!(state.scores, [35000, 15000, 25000, 25000]);

        state.update(&ServerEvent::RoundDraw {
            scores: [26000, 26000, 24000, 24000],
            reason: DrawReason::Exhaustive,
            tenpai: vec![Wind::East, Wind::South],
            riichi_sticks: 1,
            player_hands: Vec::new(),
            declarer: None,
        });
        assert_eq!(state.scores, [26000, 26000, 24000, 24000]);
    }

    #[test]
    fn test_nine_terminals_available_preserves_state() {
        let mut state = CpuGameState::new();
        state.scores = [25000, 26000, 24000, 25000];
        state.pending_calls = vec![AvailableCall::Ron];
        state.pending_call_tile = Some(Tile::new(Tile::Z1));

        state.update(&ServerEvent::NineTerminalsAvailable);

        assert_eq!(state.scores, [25000, 26000, 24000, 25000]);
        assert_eq!(state.pending_calls.len(), 1);
        assert_eq!(state.pending_call_tile, Some(Tile::new(Tile::Z1)));
    }

    #[test]
    fn test_round_position_helpers() {
        let mut state = CpuGameState::new();
        state.total_rounds = 4;

        state.round_number = 0;
        assert!(!state.is_final_round());
        assert!(!state.is_second_half());

        state.round_number = 2;
        assert!(!state.is_final_round());
        assert!(state.is_second_half());

        state.round_number = 3;
        assert!(state.is_final_round());

        // Unknown total_rounds must never report final/second half.
        state.total_rounds = 0;
        assert!(!state.is_final_round());
        assert!(!state.is_second_half());
    }

    #[test]
    fn test_score_position_helpers() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::South;
        state.scores = [25000, 30000, 20000, 25000];

        assert_eq!(state.my_score(), 30000);
        assert!(state.is_top());
        assert_eq!(state.gap_to_next_rank(), None);

        state.scores = [25000, 22000, 30000, 23000];
        assert_eq!(state.my_score(), 22000);
        assert!(!state.is_top());
        assert_eq!(state.gap_to_next_rank(), Some(1000));

        // A tied lead still counts as leading.
        state.scores = [30000, 30000, 20000, 20000];
        assert!(state.is_top());
        assert_eq!(state.gap_to_next_rank(), None);
    }

    #[test]
    fn test_turn_and_my_discards() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::South;

        assert_eq!(state.turn(), 1);
        assert!(state.my_discards().is_empty());

        // Other players' discards must not advance our turn counter.
        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::M1),
            is_tsumogiri: false,
            hand_index: None,
        });
        assert_eq!(state.turn(), 1);

        state.update(&ServerEvent::TileDiscarded {
            player: Wind::South,
            tile: Tile::new(Tile::P5),
            is_tsumogiri: true,
            hand_index: None,
        });
        assert_eq!(state.turn(), 2);
        assert_eq!(state.my_discards(), &[Tile::new(Tile::P5)]);
    }

    #[test]
    fn test_visible_tile_counts() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![Tile::new(Tile::M1), Tile::new(Tile::M1)];
        state.all_discards[0] = vec![Tile::new(Tile::M1)];

        let counts = state.visible_tile_counts();
        assert_eq!(counts[Tile::M1 as usize], 3);
    }

    #[test]
    fn test_visible_tile_counts_includes_drawn_dora_melds_and_saturates_called_discards() {
        let mut state = CpuGameState::new();
        state.my_drawn = Some(Tile::new(Tile::M2));
        state.dora_indicators = vec![Tile::new(Tile::M3)];
        state.player_melds[2].push(Meld {
            tiles: vec![Tile::new(Tile::M4); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::M4)),
        });
        state.called_discards.push(Tile::new(Tile::Z7));

        let counts = state.visible_tile_counts();
        assert_eq!(counts[Tile::M2 as usize], 1);
        assert_eq!(counts[Tile::M3 as usize], 1);
        assert_eq!(counts[Tile::M4 as usize], 3);
        assert_eq!(counts[Tile::Z7 as usize], 0);
    }

    #[test]
    fn test_called_tile_visible_count_not_double_counted() {
        let mut state = CpuGameState::new();
        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::M1),
            is_tsumogiri: false,
            hand_index: None,
        });
        state.update(&ServerEvent::PlayerCalled {
            player: Wind::South,
            call_type: CallType::Pon,
            called_tile: Tile::new(Tile::M1),
            tiles: vec![Tile::new(Tile::M1); 3],
        });

        let counts = state.visible_tile_counts();
        assert_eq!(counts[Tile::M1 as usize], 3);
        assert_eq!(
            state.all_discards[0].len(),
            1,
            "discards used for defensive evaluation should be retained"
        );
    }

    #[test]
    fn test_kakan_updates_existing_meld() {
        let mut state = CpuGameState::new();
        state.player_melds[0].push(Meld {
            tiles: vec![Tile::new(Tile::M1); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::M1)),
        });

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::East,
            call_type: CallType::Kakan,
            called_tile: Tile::new(Tile::M1),
            tiles: vec![Tile::new(Tile::M1); 4],
        });

        assert_eq!(state.player_melds[0].len(), 1);
        assert_eq!(state.player_melds[0][0].category, MeldType::Kakan);
        assert_eq!(state.player_melds[0][0].tiles.len(), 4);
        assert_eq!(state.visible_tile_counts()[Tile::M1 as usize], 4);
    }

    #[test]
    fn test_kakan_without_matching_pon_adds_new_meld() {
        let mut state = CpuGameState::new();
        state.player_melds[0].push(Meld {
            tiles: vec![Tile::new(Tile::M2); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(Tile::M2)),
        });

        state.update(&ServerEvent::PlayerCalled {
            player: Wind::East,
            call_type: CallType::Kakan,
            called_tile: Tile::new(Tile::M1),
            tiles: vec![Tile::new(Tile::M1); 4],
        });

        assert_eq!(state.player_melds[0].len(), 2);
        assert_eq!(state.player_melds[0][1].category, MeldType::Kakan);
        assert_eq!(state.player_melds[0][1].tiles, vec![Tile::new(Tile::M1); 4]);
        assert_eq!(
            state.player_melds[0][1].called_tile,
            Some(Tile::new(Tile::M1))
        );
        assert!(state.pending_kan_draw);
        assert!(state.called_discards.is_empty());
    }

    #[test]
    fn test_self_discard_hand_updates_my_hand() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        state.my_drawn = Some(Tile::new(Tile::P5));

        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::M1),
            is_tsumogiri: false,
            hand_index: None,
        });

        assert!(state.my_drawn.is_none());
        assert_eq!(state.my_hand.len(), 3);
        assert!(!state.my_hand.contains(&Tile::new(Tile::M1)));
        assert!(state.my_hand.contains(&Tile::new(Tile::P5)));
    }

    #[test]
    fn test_self_tsumogiri_keeps_my_hand() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        state.my_drawn = Some(Tile::new(Tile::P5));

        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::P5),
            is_tsumogiri: true,
            hand_index: None,
        });

        assert!(state.my_drawn.is_none());
        assert_eq!(state.my_hand.len(), 3);
        assert!(state.my_hand.contains(&Tile::new(Tile::M1)));
        assert!(!state.my_hand.contains(&Tile::new(Tile::P5)));
    }
}
