//! Which moves a player is allowed to make.
//!
//! These rules used to live inside `Round`, which meant they were only usable
//! by a game the server was itself running. They are stated here against a
//! [`Player`] plus a [`TableContext`] so that anything holding a reconstructed
//! player can ask the same questions — in particular a client rebuilding its
//! state from an external protocol, where the wire format carries the moves
//! that were made but not the moves that were available.
//!
//! `Round` remains the only caller that knows about turn phases; this module
//! answers only "is this legal for this player, given the table", never "is it
//! this player's turn".

use mahjong_core::hand_info::hand_analyzer;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, Wind};

use crate::player::Player;
use crate::protocol::AvailableCall;
use crate::scoring;

/// Minimum score needed to put up a riichi deposit.
pub const RIICHI_MIN_SCORE: i32 = 1000;

/// The table facts a legality check needs beyond the player themselves.
#[derive(Debug, Clone, Copy)]
pub struct TableContext<'a> {
    /// Round wind (bakaze / 場風)
    pub round_wind: Wind,
    /// Rules in force
    pub settings: &'a Settings,
    /// Tiles left in the live wall
    pub wall_remaining: usize,
    /// Whether the most recent draw was a quad replacement from the dead wall
    pub last_draw_was_dead_wall: bool,
    /// Quads declared so far by every player
    pub total_kan_count: usize,
}

impl TableContext<'_> {
    /// Whether a win now would be off the last live tile (haitei / houtei).
    ///
    /// A discard following a replacement draw is also followed by exhaustion,
    /// but it is not the last live-wall discard and so awards nothing.
    pub fn is_last_tile(&self) -> bool {
        self.wall_remaining == 0 && !self.last_draw_was_dead_wall
    }
}

/// Why a riichi declaration was refused.
///
/// Returned rather than folded into a boolean so callers can report the reason;
/// the round uses it for its rejection diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiichiRejection {
    /// Already declared riichi this hand
    AlreadyRiichi,
    /// The hand is open
    NotConcealed,
    /// Fewer than [`RIICHI_MIN_SCORE`] points, so the deposit cannot be paid
    ScoreTooLow,
    /// No tile left to draw after the declaring discard
    WallEmpty,
    /// The player is not holding a drawn tile
    NoDrawnTile,
    /// No discard leaves the hand ready
    NotReady,
}

/// Checks whether the player may declare riichi.
///
/// Requirements: a concealed hand, at least [`RIICHI_MIN_SCORE`] points, no
/// riichi already declared, a drawn tile in hand, at least one tile left in the
/// wall so a draw follows the declaring discard, and some discard that leaves
/// the hand ready.
pub fn check_riichi(player: &Player, ctx: &TableContext<'_>) -> Result<(), RiichiRejection> {
    if player.is_riichi {
        return Err(RiichiRejection::AlreadyRiichi);
    }
    if !player.is_menzen() {
        return Err(RiichiRejection::NotConcealed);
    }
    if player.score < RIICHI_MIN_SCORE {
        return Err(RiichiRejection::ScoreTooLow);
    }
    if ctx.wall_remaining < 1 {
        return Err(RiichiRejection::WallEmpty);
    }
    if player.hand.drawn().is_none() {
        return Err(RiichiRejection::NoDrawnTile);
    }

    let ready = can_riichi_with_discard(player, None)
        || player
            .hand
            .tiles()
            .iter()
            .copied()
            .any(|tile| can_riichi_with_discard(player, Some(tile)));
    if ready {
        Ok(())
    } else {
        Err(RiichiRejection::NotReady)
    }
}

/// Whether the player may declare riichi at all.
pub fn can_riichi(player: &Player, ctx: &TableContext<'_>) -> bool {
    check_riichi(player, ctx).is_ok()
}

/// Whether discarding `tile` leaves the hand ready. `None` discards the drawn
/// tile.
///
/// This is only the readiness half of the check; [`check_riichi`] covers the
/// rest of the requirements.
pub fn can_riichi_with_discard(player: &Player, tile: Option<Tile>) -> bool {
    let mut hand = player.hand.clone();

    match tile {
        Some(target) => {
            let drawn = hand.drawn();
            let tiles = hand.tiles_mut();
            let Some(index) = tiles.iter().position(|held| *held == target) else {
                return false;
            };
            tiles.remove(index);
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

/// Whether the player, holding a drawn tile, may win by self-draw.
pub fn can_tsumo(player: &Player, ctx: &TableContext<'_>) -> bool {
    scoring::check_win_with_settings(
        player,
        ctx.round_wind,
        true,
        ctx.is_last_tile(),
        ctx.last_draw_was_dead_wall,
        ctx.settings,
    )
    .is_win
}

/// Whether the player may win on `discarded_tile`.
///
/// Furiten blocks a ron outright; a player in riichi may still ron.
pub fn can_ron(player: &Player, discarded_tile: Tile, ctx: &TableContext<'_>) -> bool {
    if player.is_furiten() {
        return false;
    }
    scoring::check_ron_with_settings(
        player,
        discarded_tile,
        ctx.round_wind,
        ctx.is_last_tile(),
        ctx.settings,
    )
    .is_win
}

/// Collects one player's options for responding to a discard.
///
/// `is_left_of_discarder` says whether this player sits immediately after the
/// discarder, which is the only seat allowed to call a sequence.
pub fn available_calls_for(
    player: &Player,
    discarded_tile: Tile,
    is_left_of_discarder: bool,
    ctx: &TableContext<'_>,
) -> Vec<AvailableCall> {
    let mut calls = Vec::new();

    if can_ron(player, discarded_tile, ctx) {
        calls.push(AvailableCall::Ron);
    }

    // A player in riichi cannot call anything but ron. The final discard
    // likewise has no following turn in which a meld caller could discard, so
    // only ron remains legal.
    if player.is_riichi || ctx.wall_remaining == 0 {
        return calls;
    }

    let mut pon_options = player.pon_options(discarded_tile);
    if ctx.settings.forbid_swap_calling {
        pon_options.retain(|option| {
            call_leaves_legal_discard(player, discarded_tile, *option, MeldType::Pon)
        });
    }
    if !pon_options.is_empty() {
        calls.push(AvailableCall::Pon {
            options: pon_options,
        });
    }

    // No further quads once four exist on the table.
    if ctx.total_kan_count < 4 && player.can_daiminkan(discarded_tile) {
        calls.push(AvailableCall::Daiminkan);
    }

    // Chii comes only from the left and does not exist in three-player games.
    if !ctx.settings.three_player && is_left_of_discarder {
        let mut chi_options = player.chi_options(discarded_tile);
        if ctx.settings.forbid_swap_calling {
            chi_options.retain(|option| {
                call_leaves_legal_discard(player, discarded_tile, *option, MeldType::Chi)
            });
        }
        if !chi_options.is_empty() {
            calls.push(AvailableCall::Chi {
                options: chi_options,
            });
        }
    }

    calls
}

/// Whether applying this call leaves at least one legal discard under the
/// swap-calling rule.
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
