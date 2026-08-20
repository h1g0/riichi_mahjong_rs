//! Tile notation used on the wire by the mjai protocol.
//!
//! mjai writes suited tiles as rank plus suit letter (`"1m"`, `"9s"`), red
//! fives with a trailing `r` (`"5mr"`), and honours as single letters — `E`,
//! `S`, `W`, `N` for the winds and `P`, `F`, `C` for the white, green, and red
//! dragons. A concealed tile is written `"?"`.

use mahjong_core::tile::{Tile, TileType, Wind};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// mjai notation for the 34 tile kinds, indexed by [`TileType`].
///
/// Differs from `Tile`'s own ASCII table only in the honours: mjai spells them
/// with letters instead of `1z`..`7z`.
const NAMES: [&str; Tile::LEN] = [
    "1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m", //
    "1p", "2p", "3p", "4p", "5p", "6p", "7p", "8p", "9p", //
    "1s", "2s", "3s", "4s", "5s", "6s", "7s", "8s", "9s", //
    "E", "S", "W", "N", "P", "F", "C",
];

/// Returns the mjai notation for `tile`.
pub fn tile_to_str(tile: Tile) -> &'static str {
    if tile.is_red_dora() {
        match tile.get() {
            Tile::M5 => return "5mr",
            Tile::P5 => return "5pr",
            Tile::S5 => return "5sr",
            // mjai has no notation for a red flag on anything but a five.
            // Emit the plain name rather than inventing one; the flag is not
            // reachable through normal play, so failing here would be worse.
            _ => {}
        }
    }
    NAMES[tile.get() as usize]
}

/// Parses mjai tile notation. Returns `None` if `s` is not a tile.
///
/// Note that `"?"` is *not* accepted here — a concealed slot is only valid in
/// the fields that allow it, which use [`MjaiTile`].
pub fn tile_from_str(s: &str) -> Option<Tile> {
    if let Some(base) = s.strip_suffix('r') {
        let kind = kind_from_str(base)?;
        // Refuse a red flag on a non-five so a malformed log fails loudly
        // instead of decoding into a tile that cannot exist in a real wall.
        return match kind {
            Tile::M5 | Tile::P5 | Tile::S5 => Some(Tile::new_red(kind)),
            _ => None,
        };
    }
    kind_from_str(s).map(Tile::new)
}

fn kind_from_str(s: &str) -> Option<TileType> {
    NAMES
        .iter()
        .position(|name| *name == s)
        .map(|index| index as TileType)
        .or_else(|| numeric_honour(s))
}

/// Accepts `1z`..`7z` for the honours.
///
/// This is not mjai notation and is never emitted, but several bridges from
/// Tenhou logs produce it. Accepting it on input costs nothing and avoids
/// spurious decode failures against those tools.
fn numeric_honour(s: &str) -> Option<TileType> {
    let bytes = s.as_bytes();
    match bytes {
        [rank @ b'1'..=b'7', b'z'] => Some(Tile::Z1 + (rank - b'1') as TileType),
        _ => None,
    }
}

/// Returns the mjai notation for a wind, as used by the `bakaze` field.
pub fn wind_to_str(wind: Wind) -> &'static str {
    match wind {
        Wind::East => "E",
        Wind::South => "S",
        Wind::West => "W",
        Wind::North => "N",
    }
}

/// Parses a wind in mjai notation. Returns `None` if `s` is not a wind.
pub fn wind_from_str(s: &str) -> Option<Wind> {
    match s {
        "E" => Some(Wind::East),
        "S" => Some(Wind::South),
        "W" => Some(Wind::West),
        "N" => Some(Wind::North),
        _ => None,
    }
}

/// A tile slot that the protocol may conceal.
///
/// In-game mode hides information the receiving player is not entitled to —
/// an opponent's draw, or the opponents' starting hands — by writing `"?"`.
/// Replay mode reveals everything and so never produces [`MjaiTile::Hidden`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MjaiTile {
    /// A revealed tile.
    Known(Tile),
    /// Information withheld from this recipient (`"?"`).
    Hidden,
}

impl MjaiTile {
    /// Returns the tile if it is revealed.
    pub fn known(self) -> Option<Tile> {
        match self {
            MjaiTile::Known(tile) => Some(tile),
            MjaiTile::Hidden => None,
        }
    }

    /// Returns whether the slot is concealed.
    pub fn is_hidden(self) -> bool {
        matches!(self, MjaiTile::Hidden)
    }
}

impl From<Tile> for MjaiTile {
    fn from(tile: Tile) -> Self {
        MjaiTile::Known(tile)
    }
}

impl Serialize for MjaiTile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            MjaiTile::Known(tile) => serializer.serialize_str(tile_to_str(*tile)),
            MjaiTile::Hidden => serializer.serialize_str("?"),
        }
    }
}

impl<'de> Deserialize<'de> for MjaiTile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw == "?" {
            return Ok(MjaiTile::Hidden);
        }
        tile_from_str(&raw)
            .map(MjaiTile::Known)
            .ok_or_else(|| D::Error::custom(format!("invalid mjai tile: {raw}")))
    }
}

/// `serde` adapter for a single revealed [`Tile`] field.
///
/// `Tile`'s own derived representation is a struct; on the wire mjai needs the
/// string notation, so every revealed tile field opts in with
/// `#[serde(with = "tile_str")]`.
pub mod tile_str {
    use super::{Tile, tile_from_str, tile_to_str};
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(tile: &Tile, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(tile_to_str(*tile))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Tile, D::Error> {
        let raw = String::deserialize(deserializer)?;
        tile_from_str(&raw).ok_or_else(|| D::Error::custom(format!("invalid mjai tile: {raw}")))
    }
}

/// `serde` adapter for a sequence of revealed tiles, such as `consumed`.
pub mod tile_vec {
    use super::{Tile, tile_from_str, tile_to_str};
    use serde::de::Error as _;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(tiles: &[Tile], serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(tiles.len()))?;
        for tile in tiles {
            seq.serialize_element(tile_to_str(*tile))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Tile>, D::Error> {
        let raw = Vec::<String>::deserialize(deserializer)?;
        raw.into_iter()
            .map(|name| {
                tile_from_str(&name)
                    .ok_or_else(|| D::Error::custom(format!("invalid mjai tile: {name}")))
            })
            .collect()
    }
}

/// `serde` adapter for a wind field such as `bakaze`.
pub mod wind_str {
    use super::{Wind, wind_from_str, wind_to_str};
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(wind: &Wind, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(wind_to_str(*wind))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Wind, D::Error> {
        let raw = String::deserialize(deserializer)?;
        wind_from_str(&raw).ok_or_else(|| D::Error::custom(format!("invalid mjai wind: {raw}")))
    }
}

/// `serde` adapter for an optional sequence of revealed tiles.
///
/// Paired with `skip_serializing_if`, so a `None` is left out of the object
/// entirely rather than written as a null.
pub mod opt_tile_vec {
    use super::{Tile, tile_from_str, tile_to_str};
    use serde::de::Error as _;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        tiles: &Option<Vec<Tile>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match tiles {
            Some(tiles) => {
                let mut seq = serializer.serialize_seq(Some(tiles.len()))?;
                for tile in tiles {
                    seq.serialize_element(tile_to_str(*tile))?;
                }
                seq.end()
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec<Tile>>, D::Error> {
        let Some(raw) = Option::<Vec<String>>::deserialize(deserializer)? else {
            return Ok(None);
        };
        raw.into_iter()
            .map(|name| {
                tile_from_str(&name)
                    .ok_or_else(|| D::Error::custom(format!("invalid mjai tile: {name}")))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}
