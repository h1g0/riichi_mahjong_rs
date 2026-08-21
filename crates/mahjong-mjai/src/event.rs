//! mjai protocol events.
//!
//! One event is one JSON object, and a game is a stream of them separated by
//! newlines. The same enum covers both directions: the host announces what
//! happened with these events, and a player answers with the subset it is
//! allowed to declare (see [`MjaiEvent::is_player_response`]).

use mahjong_core::tile::{Tile, Wind};
use serde::{Deserialize, Serialize};

use crate::tile::{MjaiTile, tile_str, tile_vec, wind_str};

/// Why a hand ended without a win.
///
/// The upstream specification leaves this field unspecified, so the spellings
/// below are those of the reference implementation (gimite/mjai). Anything
/// unrecognised is preserved verbatim in [`RyukyokuReason::Other`] so that a
/// log from another implementation survives a decode/encode round trip
/// instead of failing to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RyukyokuReason {
    /// Exhaustive draw: the live wall ran out (荒牌平局)
    Fanpai,
    /// Nine-terminals abortive draw (九種九牌)
    Kyushukyuhai,
    /// Four-winds abortive draw (四風連打)
    Sufonrenta,
    /// Four-riichi abortive draw (四家立直)
    Suchareach,
    /// Triple-ron abortive draw (三家和)
    Sanchaho,
    /// Four-quads abortive draw (四槓散了)
    Sukaikan,
    /// Nagashi Mangan (流し満貫)
    Nagashimangan,
    /// A reason this crate does not know, kept as written.
    Other(String),
}

impl RyukyokuReason {
    /// Returns the wire spelling.
    pub fn as_str(&self) -> &str {
        match self {
            RyukyokuReason::Fanpai => "fanpai",
            RyukyokuReason::Kyushukyuhai => "kyushukyuhai",
            RyukyokuReason::Sufonrenta => "sufonrenta",
            RyukyokuReason::Suchareach => "suchareach",
            RyukyokuReason::Sanchaho => "sanchaho",
            RyukyokuReason::Sukaikan => "sukaikan",
            RyukyokuReason::Nagashimangan => "nagashimangan",
            RyukyokuReason::Other(raw) => raw,
        }
    }
}

impl From<&str> for RyukyokuReason {
    fn from(raw: &str) -> Self {
        match raw {
            "fanpai" => RyukyokuReason::Fanpai,
            "kyushukyuhai" => RyukyokuReason::Kyushukyuhai,
            "sufonrenta" => RyukyokuReason::Sufonrenta,
            "suchareach" => RyukyokuReason::Suchareach,
            "sanchaho" => RyukyokuReason::Sanchaho,
            "sukaikan" => RyukyokuReason::Sukaikan,
            "nagashimangan" => RyukyokuReason::Nagashimangan,
            other => RyukyokuReason::Other(other.to_owned()),
        }
    }
}

impl Serialize for RyukyokuReason {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RyukyokuReason {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(RyukyokuReason::from(raw.as_str()))
    }
}

/// A seat, numbered 0-3 and fixed for the whole game.
///
/// Seat 0 is the dealer of the opening hand. Unlike a seat wind this does not
/// rotate, so translating to and from `Wind` needs the current hand number.
pub type Actor = usize;

/// One mjai protocol event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MjaiEvent {
    /// Host handshake that opens the stream.
    Hello {
        protocol: String,
        protocol_version: u32,
    },

    /// Player's reply to [`MjaiEvent::Hello`].
    Join { name: String, room: String },

    /// The game is starting.
    StartGame {
        /// Seat this player occupies; absent in a replay.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<Actor>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        names: Vec<String>,
    },

    /// A new hand is starting.
    StartKyoku {
        /// Round wind (bakaze / 場風)
        #[serde(with = "wind_str")]
        bakaze: Wind,
        /// Hand number within the round wind, 1-based (East 1 = 1)
        kyoku: u8,
        /// Continuance counter (honba / 本場)
        honba: u32,
        /// Riichi deposits carried onto the table (kyotaku / 供託)
        kyotaku: u32,
        /// Seat of the dealer
        oya: Actor,
        /// Dora indicator turned up at the start of the hand
        #[serde(with = "tile_str")]
        dora_marker: Tile,
        /// Starting scores, indexed by seat
        scores: Vec<i32>,
        /// Starting hands, indexed by seat. In-game mode conceals every hand
        /// but the recipient's.
        tehais: Vec<Vec<MjaiTile>>,
    },

    /// A player drew from the live wall.
    Tsumo {
        actor: Actor,
        /// Concealed unless `actor` is the recipient.
        pai: MjaiTile,
    },

    /// A player discarded.
    Dahai {
        actor: Actor,
        #[serde(with = "tile_str")]
        pai: Tile,
        /// Whether the tile just drawn was discarded directly (tsumogiri)
        tsumogiri: bool,
    },

    /// A sequence was called (chii / 吃).
    Chi {
        actor: Actor,
        /// Seat the tile was taken from
        target: Actor,
        #[serde(with = "tile_str")]
        pai: Tile,
        /// The two hand tiles completing the sequence
        #[serde(with = "tile_vec")]
        consumed: Vec<Tile>,
    },

    /// A triplet was called (pon / 碰).
    Pon {
        actor: Actor,
        target: Actor,
        #[serde(with = "tile_str")]
        pai: Tile,
        /// The two hand tiles completing the triplet
        #[serde(with = "tile_vec")]
        consumed: Vec<Tile>,
    },

    /// A quad was called on a discard (daiminkan / 大明槓).
    Daiminkan {
        actor: Actor,
        target: Actor,
        #[serde(with = "tile_str")]
        pai: Tile,
        /// The three hand tiles completing the quad
        #[serde(with = "tile_vec")]
        consumed: Vec<Tile>,
    },

    /// A concealed quad (ankan / 暗槓).
    Ankan {
        actor: Actor,
        /// All four tiles
        #[serde(with = "tile_vec")]
        consumed: Vec<Tile>,
    },

    /// A quad promoted from an existing triplet (kakan / 加槓).
    Kakan {
        actor: Actor,
        /// The tile added to the triplet
        #[serde(with = "tile_str")]
        pai: Tile,
        /// The three tiles of the existing triplet
        #[serde(with = "tile_vec")]
        consumed: Vec<Tile>,
    },

    /// A new dora indicator was turned up.
    Dora {
        #[serde(with = "tile_str")]
        dora_marker: Tile,
    },

    /// A player declared riichi. The declaring discard follows as a separate
    /// [`MjaiEvent::Dahai`]; the deposit is only taken once
    /// [`MjaiEvent::ReachAccepted`] arrives.
    Reach { actor: Actor },

    /// The riichi declaration stood and the deposit was placed.
    ReachAccepted {
        actor: Actor,
        /// Score change for the deposit, per seat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deltas: Option<Vec<i32>>,
        /// Scores after the deposit, per seat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scores: Option<Vec<i32>>,
    },

    /// A player won.
    Hora {
        actor: Actor,
        /// Seat that dealt in; equal to `actor` on a self-draw win
        target: Actor,
        #[serde(with = "tile_str")]
        pai: Tile,
        /// Minipoints. Absent in in-game mode, where a player is not told the
        /// full breakdown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fu: Option<u32>,
        /// Han
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fan: Option<u32>,
        /// Awarded yaku as (name, han) pairs
        #[serde(default, skip_serializing_if = "Option::is_none")]
        yakus: Option<Vec<(String, u32)>>,
        /// The winner's concealed hand, revealed at the win. Melds are not
        /// repeated here; they already appear as call events.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "crate::tile::opt_tile_vec"
        )]
        hora_tehais: Option<Vec<Tile>>,
        /// Ura dora indicators, turned up only on a riichi win.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "crate::tile::opt_tile_vec"
        )]
        uradora_markers: Option<Vec<Tile>>,
        /// Total points awarded to the winner
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hora_points: Option<i32>,
        /// Score change per seat
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deltas: Option<Vec<i32>>,
        /// Scores after payment, per seat
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scores: Option<Vec<i32>>,
        /// Seat carrying a liability payment (pao / 包)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pao: Option<Actor>,
    },

    /// The hand ended without a win.
    Ryukyoku {
        reason: RyukyokuReason,
        /// Seat declaring a nine-terminals abortive draw. Absent for
        /// host-announced draws without a single declarer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actor: Option<Actor>,
        /// Whether each seat was ready. Present when the host supplied the
        /// end-of-hand state.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tenpais: Option<Vec<bool>>,
        /// Concealed hands at the end of the hand, indexed by seat. In-game
        /// mode hides hands that were not revealed; replay mode reveals all.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tehais: Option<Vec<Vec<MjaiTile>>>,
        /// Score change per seat
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deltas: Option<Vec<i32>>,
        /// Scores after payment, per seat
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scores: Option<Vec<i32>>,
    },

    /// The hand is over.
    EndKyoku,

    /// The game is over.
    EndGame,

    /// Declining to act.
    ///
    /// Named `Pass` rather than `None` so it does not read as `Option::None`
    /// at every call site; the wire name is still `"none"`.
    #[serde(rename = "none")]
    Pass,
}

impl MjaiEvent {
    /// Returns the acting seat, if the event has one.
    pub fn actor(&self) -> Option<Actor> {
        match self {
            MjaiEvent::Tsumo { actor, .. }
            | MjaiEvent::Dahai { actor, .. }
            | MjaiEvent::Chi { actor, .. }
            | MjaiEvent::Pon { actor, .. }
            | MjaiEvent::Daiminkan { actor, .. }
            | MjaiEvent::Ankan { actor, .. }
            | MjaiEvent::Kakan { actor, .. }
            | MjaiEvent::Reach { actor }
            | MjaiEvent::ReachAccepted { actor, .. }
            | MjaiEvent::Hora { actor, .. } => Some(*actor),
            MjaiEvent::Ryukyoku { actor, .. } => *actor,
            _ => None,
        }
    }

    /// Returns whether a player may send this event as its own move.
    ///
    /// The host may send every variant; a player may only declare the subset
    /// that corresponds to an action it is entitled to take.
    pub fn is_player_response(&self) -> bool {
        matches!(
            self,
            MjaiEvent::Join { .. }
                | MjaiEvent::Dahai { .. }
                | MjaiEvent::Chi { .. }
                | MjaiEvent::Pon { .. }
                | MjaiEvent::Daiminkan { .. }
                | MjaiEvent::Ankan { .. }
                | MjaiEvent::Kakan { .. }
                | MjaiEvent::Reach { .. }
                | MjaiEvent::Hora { .. }
                | MjaiEvent::Ryukyoku {
                    reason: RyukyokuReason::Kyushukyuhai,
                    actor: Some(_),
                    ..
                }
                | MjaiEvent::Pass
        )
    }

    /// Checks actor bounds and the arity of `consumed` against the call type.
    ///
    /// `consumed` is typed as a `Vec` rather than a fixed-size array so that a
    /// wrong length from a peer surfaces here as a protocol error, instead of
    /// as a `serde` type error that would abort the whole stream.
    pub fn validate(&self) -> Result<(), String> {
        let out_of_range = |actor: &Actor| (*actor >= 4).then_some(*actor);
        let invalid_actor = match self {
            MjaiEvent::StartGame {
                id: Some(actor), ..
            }
            | MjaiEvent::StartKyoku { oya: actor, .. }
            | MjaiEvent::Tsumo { actor, .. }
            | MjaiEvent::Dahai { actor, .. }
            | MjaiEvent::Ankan { actor, .. }
            | MjaiEvent::Kakan { actor, .. }
            | MjaiEvent::Reach { actor }
            | MjaiEvent::ReachAccepted { actor, .. }
            | MjaiEvent::Ryukyoku {
                actor: Some(actor), ..
            } => out_of_range(actor),
            MjaiEvent::Chi { actor, target, .. }
            | MjaiEvent::Pon { actor, target, .. }
            | MjaiEvent::Daiminkan { actor, target, .. }
            | MjaiEvent::Hora { actor, target, .. } => {
                out_of_range(actor).or_else(|| out_of_range(target))
            }
            _ => None,
        };
        if let Some(actor) = invalid_actor {
            return Err(format!("actor must be in 0..4, got {actor}"));
        }
        if let MjaiEvent::Ryukyoku {
            reason,
            actor: Some(_),
            ..
        } = self
            && !matches!(reason, RyukyokuReason::Kyushukyuhai)
        {
            return Err("only a nine-terminals draw may name an actor".to_owned());
        }

        let (label, consumed, expected) = match self {
            MjaiEvent::Chi { consumed, .. } => ("chi", consumed, 2),
            MjaiEvent::Pon { consumed, .. } => ("pon", consumed, 2),
            MjaiEvent::Daiminkan { consumed, .. } => ("daiminkan", consumed, 3),
            MjaiEvent::Kakan { consumed, .. } => ("kakan", consumed, 3),
            MjaiEvent::Ankan { consumed, .. } => ("ankan", consumed, 4),
            _ => return Ok(()),
        };
        if consumed.len() != expected {
            return Err(format!(
                "{label}: expected {expected} consumed tiles, got {}",
                consumed.len()
            ));
        }
        Ok(())
    }
}
