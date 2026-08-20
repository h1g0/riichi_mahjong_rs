//! mjai protocol support.
//!
//! [mjai] is the de facto interchange format for Japanese riichi mahjong AI.
//! Speaking it lets this project plug into an existing ecosystem in both
//! directions: an mjai bot such as Mortal can take a seat at our table, and
//! our own CPU opponent can be reviewed and benchmarked by mjai tooling.
//!
//! The `server` feature, on by default, adds translation to and from
//! `mahjong_server::protocol`. With it turned off the crate is just the wire
//! format — event types, tile notation, and newline-delimited JSON framing —
//! and depends only on `mahjong-core`.
//!
//! # Modes
//!
//! The format has two modes. *In-game mode* is what a live player sees: tiles
//! it is not entitled to know are written `"?"`, and a win reports no score
//! breakdown. *Replay mode* reveals everything. Both decode into the same
//! [`MjaiEvent`]; concealment shows up as [`MjaiTile::Hidden`] and as absent
//! optional fields.
//!
//! # Compatibility
//!
//! The published specification leaves `hora` and `ryukyoku` unfinished, so
//! those follow the reference implementation (gimite/mjai). Unknown draw
//! reasons round-trip verbatim rather than failing to parse. This is a
//! clean-room implementation written from the public specification and from
//! observed JSON; no code was taken from any existing implementation.
//!
//! [mjai]: https://mjai.app/
//!
//! # Example
//!
//! ```
//! use mahjong_mjai::{MjaiEvent, from_json, to_json};
//!
//! let event = from_json(r#"{"type":"dahai","actor":1,"pai":"5pr","tsumogiri":false}"#)?;
//! assert_eq!(event.actor(), Some(1));
//! assert_eq!(to_json(&event)?, r#"{"type":"dahai","actor":1,"pai":"5pr","tsumogiri":false}"#);
//! # Ok::<(), serde_json::Error>(())
//! ```

#[cfg(feature = "server")]
pub mod encode;
pub mod event;
pub mod tile;
pub mod yaku;

#[cfg(feature = "server")]
pub use encode::MjaiEncoder;
pub use event::{Actor, MjaiEvent, RyukyokuReason};
pub use tile::{MjaiTile, tile_from_str, tile_to_str, wind_from_str, wind_to_str};
pub use yaku::score_item_name;

/// Parses one event from a single line of JSON.
pub fn from_json(line: &str) -> Result<MjaiEvent, serde_json::Error> {
    serde_json::from_str(line)
}

/// Renders one event as a single line of JSON, without a trailing newline.
pub fn to_json(event: &MjaiEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(event)
}

/// Parses a newline-delimited stream of events.
///
/// Blank lines are skipped: some tools pad their output with them, and a log
/// that ends in a newline would otherwise fail on the final empty line.
pub fn from_json_lines(input: &str) -> Result<Vec<MjaiEvent>, serde_json::Error> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(from_json)
        .collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod yaku_tests;

#[cfg(all(test, feature = "server"))]
mod encode_tests;
