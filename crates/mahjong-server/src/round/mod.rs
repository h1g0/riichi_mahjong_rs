//! One hand of play, driving the turn flow:
//! draw, discard, call resolution, next turn.

mod calls;
#[cfg(debug_assertions)]
mod diagnostics;
mod draws;
#[cfg(test)]
mod test_helpers;
mod turn;
mod win;

use std::sync::OnceLock;

use mahjong_core::scoring::score::ScoreItem;
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, TileType, Wind};
use mahjong_core::winning_hand::name::Kind;

use crate::player::Player;
use crate::protocol::{AvailableCall, CallType, MeldTiles, PlayerHandInfo, ServerEvent};
use crate::wall::Wall;

/// Value of one riichi deposit
const RIICHI_STICK_VALUE: i32 = 1000;
/// Whether verbose round diagnostics were explicitly enabled for this process.
pub(super) fn diagnostics_enabled() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("MAHJONG_ROUND_DIAGNOSTICS").is_some())
}

/// Phase within a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnPhase {
    /// The current player draws
    Draw,
    /// Waiting for the current player's discard
    WaitForDiscard,
    /// Waiting for the other players' call responses after a discard
    WaitForCalls,
    /// Waiting for the player to accept or decline a nine-terminals draw
    WaitForNineTerminals,
    /// The hand is over
    RoundOver,
}

/// Outcome of a hand.
#[derive(Debug, Clone)]
pub enum RoundResult {
    /// Win by self-draw
    Tsumo { winner: usize, winning_tile: Tile },
    /// Win by ron, covering single, double, and triple ron
    Ron {
        /// Winners in turn-order priority from the discarder
        /// (right, across, left)
        winners: Vec<usize>,
        loser: usize,
        winning_tile: Tile,
    },
    /// Exhaustive draw: the live wall ran out
    ExhaustiveDraw {
        /// Whether the dealer was tenpai (decides dealer continuation)
        dealer_tenpai: bool,
    },
    /// Nagashi Mangan at live-wall exhaustion, possibly for multiple players
    NagashiMangan {
        /// Winners in turn order from the dealer
        winners: Vec<usize>,
    },
    /// Abortive draw (four winds, four riichi, nine terminals, ...)
    SpecialDraw,
}

/// Where play resumes after the calls resolve.
#[derive(Debug, Clone)]
enum CallResolution {
    /// Normal post-discard flow
    AfterDiscard,
    /// Resume a promoted kan after the robbing-a-quad (搶槓) window closes
    AfterKakan { caller: usize, tile_type: TileType },
}

/// State held while waiting for call responses.
#[derive(Debug, Clone)]
pub struct CallState {
    /// The discarded tile
    pub discarded_tile: Tile,
    /// Who discarded it
    pub discarder: usize,
    /// Calls available to each player (empty = cannot call)
    pub available_calls: [Vec<AvailableCall>; 4],
    /// Whether each player has responded (true = responded or not involved)
    pub responded: [bool; 4],
    /// Players who declared ron (multiple for double/triple ron)
    pub ron_declared: Vec<usize>,
    /// Pon declarer and the two hand tiles used
    pub pon_declared: Option<(usize, [Tile; 2])>,
    /// Called-quad declarer
    pub daiminkan_declared: Option<usize>,
    /// Chii declarer and the two hand tiles used
    pub chi_declared: Option<(usize, [Tile; 2])>,
    /// Where play resumes once everyone has responded
    resolution: CallResolution,
}

/// State of one hand.
pub struct Round {
    /// The wall
    pub wall: Wall,
    /// The four players
    pub players: [Player; 4],
    /// Round wind
    pub round_wind: Wind,
    /// Dealer's player index (0-3)
    pub dealer: usize,
    /// Current player's index (0-3)
    pub current_player: usize,
    /// Continuance counter (honba / 本場)
    pub honba: usize,
    /// Riichi deposits on the table
    pub riichi_sticks: usize,
    /// Current turn phase
    pub phase: TurnPhase,
    /// Outcome, set when the hand ends
    pub result: Option<RoundResult>,
    /// Queued outbound events
    events: Vec<(usize, ServerEvent)>,
    /// State while waiting for call responses
    pub call_state: Option<CallState>,
    /// Whether the latest draw came from the dead wall
    pub last_draw_was_dead_wall: bool,
    /// Liability payment (pao / 包) records: per player, a list of
    /// (locked-in yakuman, liable player). Recorded the moment a call
    /// locks in Big Dragons, Big Winds, or Four Quads.
    pub pao: [Vec<(Kind, usize)>; 4],
    /// Number of players (4, or 3 in three-player games where
    /// seat 3 is a dummy)
    pub player_count: usize,
    /// Rule settings
    pub settings: Settings,
}

impl Round {
    /// Starts a new hand.
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
            Wall::new_with_red_fives(settings.three_player, settings.red_fives),
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

    /// Starts a new hand with a seeded, deterministic wall, for
    /// simulations and reproducible tests.
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
            Wall::new_with_seed_and_red_fives(seed, settings.three_player, settings.red_fives),
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

    /// Shared constructor taking a prepared wall.
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
        let player_count = settings.player_count();
        let dealt = wall.deal(player_count);

        // Assign seat winds: the dealer is East, then South and West
        // (and North) counter-clockwise. In three-player games there is
        // no North seat and seat 3 is a dummy.
        let winds: [Wind; 4] = std::array::from_fn(|i| {
            if i < player_count {
                Wind::from_index((i + player_count - dealer) % player_count)
            } else {
                Wind::North
            }
        });

        let players: [Player; 4] = std::array::from_fn(|i| {
            if i < player_count {
                Player::new(winds[i], dealt[i].clone(), initial_scores[i])
            } else {
                // Dummy seat: empty hand, zero score; can never be
                // tenpai, furiten, or riichi.
                Player::new(winds[i], Vec::new(), 0)
            }
        });

        let dora_indicators = wall.dora_indicators();

        let mut events = Vec::new();
        for (i, player) in players.iter().enumerate().take(player_count) {
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
                    three_player: settings.three_player,
                    nuki_dora: settings.three_player && settings.nuki_dora,
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
            pao: std::array::from_fn(|_| Vec::new()),
            player_count,
            settings,
        }
    }

    /// Returns one liability entry for each pao-covered yakuman unit in the
    /// winning hand. A double-yakuman Big Winds record therefore repeats its
    /// liable player twice. The order follows when liability was locked in.
    pub(super) fn pao_players_for_win(
        &self,
        winner: usize,
        yaku_list: &[(ScoreItem, u32)],
    ) -> Vec<usize> {
        self.pao[winner]
            .iter()
            .flat_map(|(pao_kind, liable)| {
                let units = yaku_list
                    .iter()
                    .find_map(|(item, han)| {
                        matches!(item, ScoreItem::Yaku(kind) if kind == pao_kind)
                            .then_some((*han / 13) as usize)
                    })
                    .unwrap_or(0);
                std::iter::repeat_n(*liable, units)
            })
            .collect()
    }

    /// Next seat in turn order, wrapping at the player count.
    /// Bundles the table facts the legality rules need.
    pub(crate) fn table_context(&self) -> crate::legality::TableContext<'_> {
        crate::legality::TableContext {
            round_wind: self.round_wind,
            settings: &self.settings,
            wall_remaining: self.wall.remaining(),
            last_draw_was_dead_wall: self.last_draw_was_dead_wall,
            total_kan_count: self.total_kan_count(),
        }
    }

    fn next_seat(&self, seat: usize) -> usize {
        (seat + 1) % self.player_count
    }

    /// Builds every player's revealed hand info
    /// (the dummy seat is excluded in three-player games).
    fn build_player_hands(&self) -> Vec<PlayerHandInfo> {
        self.players
            .iter()
            .take(self.player_count)
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
                    pei: p.pei_tiles.clone(),
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

    /// Drains the queued events as (recipient player index, event) pairs.
    pub fn drain_events(&mut self) -> Vec<(usize, ServerEvent)> {
        std::mem::take(&mut self.events)
    }

    /// Advances an auto player's turn by one draw-and-discard
    /// (always discarding the drawn tile).
    pub fn advance_auto_player(&mut self) -> bool {
        if self.phase == TurnPhase::RoundOver {
            return false;
        }

        if !self.do_draw() {
            return false;
        }

        if self.phase == TurnPhase::RoundOver {
            return true;
        }

        self.do_discard(None)
    }

    /// Whether the hand is over.
    pub fn is_over(&self) -> bool {
        self.phase == TurnPhase::RoundOver
    }
}

/// A player's response to a call opportunity.
#[derive(Debug, Clone)]
pub enum CallResponse {
    Ron,
    /// Pon with the two hand tiles to use (red fives distinct)
    Pon {
        hand_tile_types: [Tile; 2],
    },
    Daiminkan,
    /// Chii with the two hand tiles to use (red fives distinct)
    Chi {
        hand_tile_types: [Tile; 2],
    },
    Pass,
}

#[cfg(test)]
mod tests;
