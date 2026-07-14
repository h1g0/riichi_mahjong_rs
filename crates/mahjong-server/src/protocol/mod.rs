//! Server-client protocol.
//!
//! Message definitions designed with online play in mind; LocalAdapter
//! exchanges these messages directly in-process.

pub mod net;

use mahjong_core::scoring::score::{ScoreItem, ScoreRank};
use mahjong_core::tile::{Tile, Wind};
use serde::{Deserialize, Serialize};

/// Reason a hand ended in a draw.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawReason {
    /// Exhaustive draw: the live wall ran out (荒牌流局)
    Exhaustive,
    /// Four-winds abortive draw (四風連打)
    FourWinds,
    /// Four-riichi abortive draw (四家立直)
    FourRiichi,
    /// Nine-terminals abortive draw (九種九牌)
    NineTerminals,
    /// Four-quads abortive draw (四槓散了)
    FourKans,
    /// Triple-ron abortive draw (三家和)
    TripleRon,
}

/// Kind of call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallType {
    Ron,
    Pon,
    Chi,
    /// Concealed quad (暗槓)
    Ankan,
    /// Called quad (大明槓)
    Daiminkan,
    /// Promoted quad (加槓)
    Kakan,
}

/// A player's hand as revealed at the end of a hand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHandInfo {
    /// Seat wind
    pub wind: Wind,
    /// Concealed part of the hand
    pub hand: Vec<Tile>,
    /// Melded groups
    pub melds: Vec<MeldTiles>,
    /// North tiles set aside as pei dora (three-player only; always empty
    /// in four-player games)
    #[serde(default)]
    pub pei: Vec<Tile>,
}

/// Tiles of one meld.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeldTiles {
    /// Kind of call
    pub call_type: CallType,
    /// Tiles revealed by the call
    pub tiles: Vec<Tile>,
}

/// One recipient of a Nagashi Mangan payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NagashiManganWinner {
    /// Seat wind of the winner
    pub wind: Wind,
    /// Points gained from this win, including any continuance and riichi
    /// deposits assigned to this winner
    pub score_points: i32,
}

/// A call the player may make, carried by the CallAvailable event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvailableCall {
    Ron,
    /// Pon; each option lists the two hand tiles to use (red fives distinct)
    Pon {
        options: Vec<[Tile; 2]>,
    },
    Daiminkan,
    /// Chii; each option lists the two hand tiles to use (red fives distinct)
    Chi {
        options: Vec<[Tile; 2]>,
    },
}

/// Events sent from the server to a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerEvent {
    /// A new hand has started
    GameStarted {
        /// This client's seat wind
        seat_wind: Wind,
        /// This client's starting hand
        hand: Vec<Tile>,
        /// Starting scores
        scores: [i32; 4],
        /// Round wind
        round_wind: Wind,
        /// Revealed dora indicators
        dora_indicators: Vec<Tile>,
        /// Hand number, 0-based (East 1 = 0, East 2 = 1, ...)
        round_number: usize,
        /// Hands in the whole game (East-only = 4, hanchan = 8);
        /// used to detect the final hand
        total_rounds: usize,
        /// Continuance counter (honba / 本場)
        honba: usize,
        /// Riichi deposits on the table
        riichi_sticks: usize,
        /// Three-player game: affects player count, tile set, and the
        /// dora chain
        #[serde(default)]
        three_player: bool,
        /// Whether pei dora (North extraction) is enabled
        /// (can only be true in three-player games)
        #[serde(default)]
        nuki_dora: bool,
    },

    /// This client drew a tile
    TileDrawn {
        /// The drawn tile
        tile: Tile,
        /// Tiles left in the live wall
        remaining_tiles: usize,
        /// Whether a tsumo win is possible
        can_tsumo: bool,
        /// Whether riichi may be declared
        can_riichi: bool,
        /// Whether the player is furiten (may win by tsumo only, not ron)
        is_furiten: bool,
    },

    /// Another player drew a tile (the tile itself stays hidden)
    OtherPlayerDrew {
        /// Seat wind of the player who drew
        player: Wind,
        /// Tiles left in the live wall
        remaining_tiles: usize,
    },

    /// A tile was discarded
    TileDiscarded {
        /// Seat wind of the discarder
        player: Wind,
        /// The discarded tile
        tile: Tile,
        /// Whether the drawn tile was discarded directly (tsumogiri)
        is_tsumogiri: bool,
        /// For a hand discard (tedashi), the 0-based position within the
        /// sorted hand (excluding the drawn tile) before the discard; None
        /// on tsumogiri. At a real table everyone can see which part of the
        /// hand a discard came from, so clients use this to animate the gap
        /// closing in an opponent's hand.
        #[serde(default)]
        hand_index: Option<usize>,
    },

    /// This client may call on a discard
    CallAvailable {
        /// The discarded tile
        tile: Tile,
        /// Seat wind of the discarder
        discarder: Wind,
        /// Calls the player may make
        calls: Vec<AvailableCall>,
    },

    /// A player made a call
    PlayerCalled {
        /// Seat wind of the caller
        player: Wind,
        /// Kind of call
        call_type: CallType,
        /// The called discard
        called_tile: Tile,
        /// Tiles revealed by the call
        tiles: Vec<Tile>,
    },

    /// The revealed dora indicators changed
    DoraIndicatorsUpdated {
        /// Currently revealed dora indicators
        dora_indicators: Vec<Tile>,
    },

    /// A player declared riichi
    PlayerRiichi {
        /// Seat wind of the declarer
        player: Wind,
        /// Scores after the riichi deposit
        scores: [i32; 4],
        /// Riichi deposits on the table
        riichi_sticks: usize,
    },

    /// A player extracted a North tile as pei dora (three-player games)
    PeiDeclared {
        /// Seat wind of the declarer
        player: Wind,
        /// Extracted North count per player, indexed by wind
        /// (East = 0, South = 1, West = 2)
        pei_counts: [u8; 4],
    },

    /// Hand resync after a call or riichi
    HandUpdated {
        /// The updated hand
        hand: Vec<Tile>,
    },

    /// The hand ended with a win
    RoundWon {
        /// Seat wind of the winner
        winner: Wind,
        /// Seat wind of the deal-in player; None on tsumo
        loser: Option<Wind>,
        /// The winning tile
        winning_tile: Tile,
        /// Scores after the payments
        scores: [i32; 4],
        /// Awarded yaku and dora as (item, han) pairs;
        /// the client localizes the display names
        yaku_list: Vec<(ScoreItem, u32)>,
        /// Han
        han: u32,
        /// Fu (minipoints)
        fu: u32,
        /// Points gained by the winner
        score_points: i32,
        /// Score rank (mangan etc.; usually `ScoreRank::Normal`)
        rank: ScoreRank,
        /// Whether the winning hand was open; used to reconstruct the
        /// "(Open)" suffix on yaku names
        has_opened: bool,
        /// Ura dora indicators (revealed only on a riichi win)
        uradora_indicators: Vec<Tile>,
        /// Riichi deposits on the table before the win
        riichi_sticks: usize,
        /// Every player's revealed hand
        player_hands: Vec<PlayerHandInfo>,
    },

    /// The live wall was exhausted and one or more players completed
    /// Nagashi Mangan. This is separate from `RoundWon` because there is no
    /// winning tile.
    RoundNagashiMangan {
        /// Qualifying players in turn order from the dealer
        winners: Vec<NagashiManganWinner>,
        /// Scores after every Nagashi Mangan payment
        scores: [i32; 4],
        /// Riichi deposits on the table before they were awarded
        riichi_sticks: usize,
        /// Every player's revealed hand
        player_hands: Vec<PlayerHandInfo>,
    },

    /// This client may declare a nine-terminals abortive draw
    NineTerminalsAvailable,

    /// The hand ended in a draw
    RoundDraw {
        /// Scores after any noten penalty
        scores: [i32; 4],
        /// Why the hand was drawn
        reason: DrawReason,
        /// Seat winds of tenpai players (exhaustive draws only)
        tenpai: Vec<Wind>,
        /// Riichi deposits on the table
        riichi_sticks: usize,
        /// Every player's revealed hand
        player_hands: Vec<PlayerHandInfo>,
        /// Seat wind of the declarer (nine-terminals draws only)
        declarer: Option<Wind>,
    },
}

/// Actions sent from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientAction {
    /// Discard a tile
    Discard {
        /// The tile to discard; None discards the drawn tile (tsumogiri)
        tile: Option<Tile>,
    },

    /// Declare a tsumo win
    Tsumo,

    /// Declare a ron win
    Ron,

    /// Declare riichi
    Riichi {
        /// The tile to discard; None discards the drawn tile
        tile: Option<Tile>,
    },

    /// Call chii
    Chi {
        /// The two hand tiles to use (actual tiles; red fives distinct)
        tiles: [Tile; 2],
    },

    /// Call pon
    Pon {
        /// The two hand tiles to use (actual tiles; red fives distinct)
        tiles: [Tile; 2],
    },

    /// Declare a kan (concealed, called, or promoted)
    Kan {
        /// Tile kind to kan
        tile_index: usize,
    },

    /// Extract a North tile as pei dora (three-player games): reveal a North
    /// from the hand or the drawn tile, then draw a replacement
    Pei,

    /// Pass on a call or ron
    Pass,

    /// Respond to a nine-terminals offer (true = abort, false = continue)
    NineTerminals { declare: bool },
}
