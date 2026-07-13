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
