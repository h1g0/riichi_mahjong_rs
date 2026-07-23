//! Game adapters: the abstraction boundary between the client UI and the
//! server logic, letting local play (embedded server) and online play
//! (over the network) share one interface.

mod local;
mod remote;

pub use local::LocalAdapter;
pub use remote::{ConnStatus, RemoteAdapter, RoomView, error_code_message};

use mahjong_core::settings::Lang;
use mahjong_server::protocol::{ClientAction, ServerEvent};

/// The game server as seen from the client UI.
///
/// The main loop sends actions and drains events through this trait
/// without knowing whether the server is local or remote.
pub trait GameAdapter {
    /// Sends a player action.
    fn send_action(&mut self, action: ClientAction);

    /// Drains the events addressed to this player.
    fn poll_events(&mut self) -> Vec<ServerEvent>;

    /// Advances the game one tick.
    fn tick(&mut self);

    /// Acknowledges the result screen and requests the next hand.
    fn request_next_round(&mut self);

    /// Whether the game is over.
    fn is_game_over(&self) -> bool;

    /// Status text (connection issues etc.); None when all is well.
    fn status_text(&self, _lang: Lang) -> Option<String> {
        None
    }

    /// Seconds left on the turn timer; None when unlimited.
    fn turn_remaining_secs(&self) -> Option<u32> {
        None
    }
}

/// The active in-game adapter, kept as an enum so an online connection
/// can move back to the lobby after the final results.
pub enum ActiveAdapter {
    Local(Box<LocalAdapter>),
    Remote(Box<RemoteAdapter>),
}

impl ActiveAdapter {
    /// Recovers the remote connection when an online game returns to its
    /// room. A local adapter has no lobby connection to recover.
    pub fn into_remote(self) -> Option<RemoteAdapter> {
        match self {
            ActiveAdapter::Local(_) => None,
            ActiveAdapter::Remote(remote) => Some(*remote),
        }
    }
}

impl GameAdapter for ActiveAdapter {
    fn send_action(&mut self, action: ClientAction) {
        match self {
            ActiveAdapter::Local(adapter) => adapter.send_action(action),
            ActiveAdapter::Remote(adapter) => adapter.send_action(action),
        }
    }

    fn poll_events(&mut self) -> Vec<ServerEvent> {
        match self {
            ActiveAdapter::Local(adapter) => adapter.poll_events(),
            ActiveAdapter::Remote(adapter) => adapter.poll_events(),
        }
    }

    fn tick(&mut self) {
        match self {
            ActiveAdapter::Local(adapter) => adapter.tick(),
            ActiveAdapter::Remote(adapter) => adapter.tick(),
        }
    }

    fn request_next_round(&mut self) {
        match self {
            ActiveAdapter::Local(adapter) => adapter.request_next_round(),
            ActiveAdapter::Remote(adapter) => adapter.request_next_round(),
        }
    }

    fn is_game_over(&self) -> bool {
        match self {
            ActiveAdapter::Local(adapter) => adapter.is_game_over(),
            ActiveAdapter::Remote(adapter) => adapter.is_game_over(),
        }
    }

    fn status_text(&self, lang: Lang) -> Option<String> {
        match self {
            ActiveAdapter::Local(adapter) => adapter.status_text(lang),
            ActiveAdapter::Remote(adapter) => adapter.status_text(lang),
        }
    }

    fn turn_remaining_secs(&self) -> Option<u32> {
        match self {
            ActiveAdapter::Local(adapter) => adapter.turn_remaining_secs(),
            ActiveAdapter::Remote(adapter) => adapter.turn_remaining_secs(),
        }
    }
}
