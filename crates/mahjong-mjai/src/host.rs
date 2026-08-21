//! Seating an mjai player at a game this project is running.
//!
//! [`MjaiHost`] is the mirror of [`crate::bot`]: it turns the server's events
//! into the mjai a player expects, and turns that player's declarations back
//! into `ClientAction`s the server accepts. It holds no I/O, so the player on
//! the other side can be an in-process [`crate::bot::MjaiBot`], a subprocess
//! speaking over pipes, or anything else that answers events.
//!
//! # Riichi takes two turns
//!
//! A player declares `reach` and only names the discard afterwards, while the
//! server wants both at once. The host therefore holds the declaration back
//! until the discard arrives and then submits them together, which is also why
//! [`MjaiHost::to_client_action`] returns nothing for the `reach` itself.

use mahjong_core::tile::Tile;
use mahjong_server::protocol::{ClientAction, ServerEvent};

use crate::encode::MjaiEncoder;
use crate::event::{Actor, MjaiEvent};

/// Bridges one seat of a running game to an mjai player.
pub struct MjaiHost {
    encoder: MjaiEncoder,
    /// Whether the player has declared riichi but not yet named the discard.
    awaiting_riichi_discard: bool,
}

impl MjaiHost {
    /// Creates a host for a seat, announcing the table under `names`.
    pub fn new(names: Vec<String>) -> Self {
        Self {
            encoder: MjaiEncoder::new(names),
            awaiting_riichi_discard: false,
        }
    }

    /// The seat being hosted, once a hand has started.
    pub fn actor(&self) -> Option<Actor> {
        self.encoder.self_actor()
    }

    /// Turns one server event into the mjai events the player should see.
    pub fn encode(&mut self, event: &ServerEvent) -> Vec<MjaiEvent> {
        self.encoder.encode(event)
    }

    /// Announces the end of the match.
    pub fn end_game(&self) -> Vec<MjaiEvent> {
        self.encoder.end_game()
    }

    /// Turns a player's declaration into the action the server expects.
    ///
    /// Returns `None` when the declaration is not an action on its own — a
    /// pass with nothing to pass on, or the first half of a riichi.
    pub fn to_client_action(&mut self, event: &MjaiEvent) -> Option<ClientAction> {
        if event.validate().is_err()
            || event
                .actor()
                .is_some_and(|actor| self.actor() != Some(actor))
        {
            return None;
        }
        match event {
            MjaiEvent::Reach { .. } => {
                // Held until the declaring discard names the tile.
                self.awaiting_riichi_discard = true;
                None
            }
            MjaiEvent::Dahai { pai, tsumogiri, .. } => {
                let tile = discard_tile(*pai, *tsumogiri);
                if self.awaiting_riichi_discard {
                    self.awaiting_riichi_discard = false;
                    Some(ClientAction::Riichi { tile })
                } else {
                    Some(ClientAction::Discard { tile })
                }
            }
            MjaiEvent::Hora { actor, target, .. } => {
                if actor == target {
                    Some(ClientAction::Tsumo)
                } else {
                    Some(ClientAction::Ron)
                }
            }
            MjaiEvent::Pon { consumed, .. } => {
                two_tiles(consumed).map(|tiles| ClientAction::Pon { tiles })
            }
            MjaiEvent::Chi { consumed, .. } => {
                two_tiles(consumed).map(|tiles| ClientAction::Chi { tiles })
            }
            // Every quad is one action to the server, identified by tile kind.
            MjaiEvent::Daiminkan { pai, .. } | MjaiEvent::Kakan { pai, .. } => {
                Some(ClientAction::Kan {
                    tile_index: pai.get() as usize,
                })
            }
            MjaiEvent::Ankan { consumed, .. } => consumed.first().map(|tile| ClientAction::Kan {
                tile_index: tile.get() as usize,
            }),
            MjaiEvent::Ryukyoku {
                reason: crate::event::RyukyokuReason::Kyushukyuhai,
                actor: Some(_),
                ..
            } => Some(ClientAction::NineTerminals { declare: true }),
            MjaiEvent::Pass => Some(ClientAction::Pass),
            // Nothing a player declares.
            _ => None,
        }
    }

    /// Whether the player is midway through declaring a riichi.
    pub fn awaiting_riichi_discard(&self) -> bool {
        self.awaiting_riichi_discard
    }

    /// The action to take for a prompt mjai cannot put to the player.
    ///
    /// The server asks whether to abort the hand in a separate event, while an
    /// mjai player derives the option from its draw and either declares
    /// `ryukyoku` or passes. [`MjaiHost::encode`] therefore produces nothing
    /// for the later server prompt. If the player passed, something still has
    /// to answer that prompt or the hand stops; declining matches the server's
    /// own default and keeps the hand going.
    ///
    /// A host must call this for every server event it forwards, and submit
    /// whatever comes back alongside the player's own declarations.
    pub fn unrepresentable_prompt(&self, event: &ServerEvent) -> Option<ClientAction> {
        match event {
            ServerEvent::NineTerminalsAvailable => {
                Some(ClientAction::NineTerminals { declare: false })
            }
            _ => None,
        }
    }
}

/// The tile a discard names, or `None` when it is the drawn tile.
///
/// mjai always spells the tile out even for a tsumogiri, but the server wants
/// `None` there so it takes the tile from the draw slot rather than searching
/// the hand for a copy that may not be the one that was drawn.
fn discard_tile(pai: Tile, tsumogiri: bool) -> Option<Tile> {
    if tsumogiri { None } else { Some(pai) }
}

fn two_tiles(consumed: &[Tile]) -> Option<[Tile; 2]> {
    match consumed {
        [first, second] => Some([*first, *second]),
        _ => None,
    }
}
