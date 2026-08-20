//! Recording a whole game as a fully revealed mjai log (*replay mode*).
//!
//! [`MjaiEncoder`] alone works from one seat and therefore has to conceal what
//! that seat cannot see. A recorder collects all four seats' streams instead,
//! which between them hold every tile, and replays them into the encoder with
//! the gaps filled in.
//!
//! Only two things are actually missing from a single seat's view — the other
//! players' starting hands and their draws. Everything else is already public
//! (discards, calls, dora) or rides along on the result event (final hands,
//! ura dora). So one seat drives the output as the backbone and the other
//! three serve purely as lookup tables, which keeps the event ordering exactly
//! as the server produced it.
//!
//! # Collecting the streams
//!
//! Every seat must buffer its events. A seat driven by [`GameDriver::set_cpu`]
//! does not, so give each seat a shadow CPU and then hand it control:
//!
//! ```no_run
//! # use mahjong_server::driver::GameDriver;
//! # use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
//! # use mahjong_server::table::GameSettings;
//! let mut driver = GameDriver::new(GameSettings::default());
//! for seat in 0..4 {
//!     driver.set_shadow_cpu(seat, CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced));
//!     driver.set_cpu_controlled(seat, true);
//! }
//! ```
//!
//! [`GameDriver::set_cpu`]: mahjong_server::driver::GameDriver::set_cpu

use std::collections::VecDeque;

use mahjong_server::protocol::ServerEvent;

use crate::encode::{MjaiEncoder, Reveal};
use crate::event::{Actor, MjaiEvent};

/// Collects every seat's events and renders them as a revealed mjai log.
pub struct MjaiRecorder {
    names: Vec<String>,
    streams: Vec<Vec<ServerEvent>>,
}

impl MjaiRecorder {
    /// Creates a recorder that will announce the players under `names`.
    pub fn new(names: Vec<String>) -> Self {
        Self {
            names,
            streams: vec![Vec::new(); 4],
        }
    }

    /// Appends events drained from one seat.
    ///
    /// Call this for every seat on every drain, in seat order, so each seat's
    /// stream keeps the order the server produced.
    pub fn record<I>(&mut self, seat: usize, events: I)
    where
        I: IntoIterator<Item = ServerEvent>,
    {
        if let Some(stream) = self.streams.get_mut(seat) {
            stream.extend(events);
        }
    }

    /// Returns whether anything has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.streams.iter().all(|stream| stream.is_empty())
    }

    /// Renders the recording as a fully revealed mjai log.
    pub fn finish(&self) -> Vec<MjaiEvent> {
        let hands: Vec<Vec<&[ServerEvent]>> =
            self.streams.iter().map(|s| split_into_hands(s)).collect();

        // The backbone is whichever seat recorded the most hands; a seat that
        // joined late would otherwise truncate the log.
        let Some(backbone_seat) = (0..hands.len()).max_by_key(|seat| hands[*seat].len()) else {
            return Vec::new();
        };
        let hand_count = hands[backbone_seat].len();

        let mut encoder = MjaiEncoder::new(self.names.clone());
        let mut log = Vec::new();
        for hand_index in 0..hand_count {
            encoder.set_reveal(Some(self.reveal_for(&hands, hand_index)));
            for event in hands[backbone_seat][hand_index] {
                log.extend(encoder.encode(event));
            }
        }
        if !log.is_empty() {
            log.extend(encoder.end_game());
        }
        log
    }

    /// Gathers the hidden information every seat holds for one hand.
    fn reveal_for(&self, hands: &[Vec<&[ServerEvent]>], hand_index: usize) -> Reveal {
        let player_count = self.streams.len();
        let mut reveal = Reveal {
            tehais: vec![Vec::new(); player_count],
            draws: vec![VecDeque::new(); player_count],
        };
        for seat_hands in hands.iter() {
            let Some(events) = seat_hands.get(hand_index) else {
                continue;
            };
            let Some(actor) = actor_of_hand(events, player_count) else {
                continue;
            };
            for event in events.iter() {
                match event {
                    ServerEvent::GameStarted { hand, .. } => {
                        reveal.tehais[actor] = hand.clone();
                    }
                    ServerEvent::TileDrawn { tile, .. } => {
                        reveal.draws[actor].push_back(*tile);
                    }
                    _ => {}
                }
            }
        }
        reveal
    }
}

/// Splits one seat's stream into per-hand slices at each hand start.
///
/// Anything before the first `GameStarted` is dropped: an mjai log cannot
/// describe play outside a hand, so there is nothing to render it as.
fn split_into_hands(stream: &[ServerEvent]) -> Vec<&[ServerEvent]> {
    let starts: Vec<usize> = stream
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event, ServerEvent::GameStarted { .. }))
        .map(|(index, _)| index)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(nth, start)| {
            let end = starts.get(nth + 1).copied().unwrap_or(stream.len());
            &stream[*start..end]
        })
        .collect()
}

/// Derives the absolute seat of the player whose hand slice this is.
///
/// Uses the same rule as the encoder: seat winds are handed out from the
/// dealer, who is always seat `round_number % player_count`.
fn actor_of_hand(events: &[ServerEvent], player_count: usize) -> Option<Actor> {
    events.iter().find_map(|event| match event {
        ServerEvent::GameStarted {
            seat_wind,
            round_number,
            ..
        } => Some((round_number + seat_wind.to_index()) % player_count),
        _ => None,
    })
}

/// Renders a recorded log as newline-delimited JSON, one event per line.
pub fn to_json_lines(log: &[MjaiEvent]) -> Result<String, serde_json::Error> {
    let mut out = String::new();
    for event in log {
        out.push_str(&crate::to_json(event)?);
        out.push('\n');
    }
    Ok(out)
}
