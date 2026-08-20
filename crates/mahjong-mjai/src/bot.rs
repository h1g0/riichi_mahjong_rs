//! Running this project's CPU opponent as an mjai bot.
//!
//! [`MjaiBot`] is the whole pipeline in one place: mjai events in, mjai actions
//! out, with [`crate::decode::MjaiDecoder`] rebuilding the server's view and
//! `CpuClient` making the decisions. It holds no I/O of its own, so it can be
//! driven over a pipe, over a socket, or straight from a test.
//!
//! # The protocol is one response per event
//!
//! An mjai player answers *every* event it is sent, with `none` when it has
//! nothing to declare. [`MjaiBot::respond`] therefore always yields an event.
//!
//! # Riichi takes two turns
//!
//! mjai splits a riichi across two exchanges: the player declares `reach`, the
//! host echoes it back, and only then does the declaring discard follow. The
//! server models it as one action, so the bot holds the chosen discard until
//! its own `reach` comes back around.

use mahjong_core::settings::Settings;
use mahjong_core::tile::Tile;
use mahjong_server::cpu::client::{CpuClient, CpuConfig};
use mahjong_server::protocol::ClientAction;

use crate::decode::MjaiDecoder;
use crate::event::{Actor, MjaiEvent};

/// Protocol version announced in the handshake.
const PROTOCOL_VERSION: u32 = 3;

/// Plays a seat by feeding decoded events to the CPU opponent.
pub struct MjaiBot {
    name: String,
    config: CpuConfig,
    settings: Settings,
    /// Built once the game starts and the seat is known.
    seat: Option<Seat>,
    /// A riichi discard chosen but not yet sent, waiting for the declaration
    /// to come back from the host.
    pending_riichi_discard: Option<Option<Tile>>,
}

/// The parts that only exist once the bot knows which seat it is playing.
struct Seat {
    actor: Actor,
    decoder: MjaiDecoder,
    cpu: CpuClient,
}

impl MjaiBot {
    /// Creates a bot that will introduce itself as `name`.
    pub fn new(name: impl Into<String>, config: CpuConfig, settings: Settings) -> Self {
        Self {
            name: name.into(),
            config,
            settings,
            seat: None,
            pending_riichi_discard: None,
        }
    }

    /// The seat this bot is playing, once the game has started.
    pub fn actor(&self) -> Option<Actor> {
        self.seat.as_ref().map(|seat| seat.actor)
    }

    /// Answers one event from the host.
    ///
    /// Always returns something, because the protocol expects a reply to every
    /// event; [`MjaiEvent::Pass`] is the reply that declares nothing.
    pub fn respond(&mut self, event: &MjaiEvent) -> MjaiEvent {
        match event {
            MjaiEvent::Hello { .. } => {
                return MjaiEvent::Join {
                    name: self.name.clone(),
                    room: "default".to_owned(),
                };
            }
            MjaiEvent::StartGame { id, .. } => {
                // The seat is only known here, so everything that depends on it
                // is built now rather than at construction.
                let actor = id.unwrap_or(0);
                self.seat = Some(Seat {
                    actor,
                    decoder: MjaiDecoder::with_settings(actor, self.settings.clone()),
                    cpu: CpuClient::new_with_rules(self.config.clone(), &self.settings),
                });
                self.pending_riichi_discard = None;
                return MjaiEvent::Pass;
            }
            _ => {}
        }

        let Some(seat) = self.seat.as_mut() else {
            // Nothing can be decided before the game starts.
            return MjaiEvent::Pass;
        };

        // A riichi declared last turn is now confirmed; the discard that was
        // held back goes out in reply to it.
        if let MjaiEvent::Reach { actor } = event
            && *actor == seat.actor
            && let Some(tile) = self.pending_riichi_discard.take()
        {
            seat.decoder.decode(event);
            return seat
                .decoder
                .encode_action(&ClientAction::Discard { tile })
                .unwrap_or(MjaiEvent::Pass);
        }

        let mut response = None;
        for server_event in seat.decoder.decode(event) {
            let Some(action) = seat.cpu.handle_event(&server_event) else {
                continue;
            };
            // Declining a ron makes the hand furiten, which the log will not
            // tell the decoder — only the bot knows it passed.
            if matches!(action, ClientAction::Pass)
                && matches!(
                    server_event,
                    mahjong_server::protocol::ServerEvent::CallAvailable { .. }
                )
            {
                seat.decoder.declined_ron();
            }
            if let ClientAction::Riichi { tile } = action {
                // Declare now, discard when the declaration comes back.
                self.pending_riichi_discard = Some(tile);
                response = Some(MjaiEvent::Reach { actor: seat.actor });
                continue;
            }
            if let Some(rendered) = seat.decoder.encode_action(&action) {
                response = Some(rendered);
            }
        }
        response.unwrap_or(MjaiEvent::Pass)
    }

    /// The handshake a host opens with, provided so tests and in-process hosts
    /// do not have to spell it out.
    pub fn hello() -> MjaiEvent {
        MjaiEvent::Hello {
            protocol: "mjai".to_owned(),
            protocol_version: PROTOCOL_VERSION,
        }
    }
}
