//! Local adapter: wires the server and client together in-process.
//! Game flow and CPU handling are delegated to
//! mahjong_server::driver::GameDriver, with a delay on CPU discards to
//! simulate thinking time.

use macroquad::miniquad::date;
use mahjong_server::cpu::client::CpuConfig;
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::{ClientAction, ServerEvent};
use mahjong_server::table::GameSettings;

use super::GameAdapter;

/// The human player's seat index (0 = East / dealer).
const HUMAN_SEAT: usize = 0;

/// Delay between CPU actions, in seconds.
const CPU_ACTION_DELAY_SECONDS: f64 = 1.0;

/// Local adapter embedding the server.
pub struct LocalAdapter {
    driver: GameDriver,
}

impl LocalAdapter {
    /// Creates an adapter with the given game and CPU settings.
    ///
    /// The human sits at seat 0; CPUs fill seats 1-3 (1-2 in
    /// three-player games).
    pub fn with_settings(settings: GameSettings, cpu_configs: [CpuConfig; 3]) -> Self {
        let player_count = settings.rules.player_count();
        let mut driver = GameDriver::new(settings);
        for (i, config) in cpu_configs.into_iter().enumerate().take(player_count - 1) {
            driver.set_cpu(HUMAN_SEAT + 1 + i, config);
        }
        driver.set_cpu_action_delay(CPU_ACTION_DELAY_SECONDS);
        LocalAdapter { driver }
    }

    /// Starts the game (deals the first hand).
    pub fn start_game(&mut self) {
        self.driver.start_game();
    }
}

impl GameAdapter for LocalAdapter {
    fn send_action(&mut self, action: ClientAction) {
        self.driver
            .handle_action_at(HUMAN_SEAT, action, date::now());
    }

    fn poll_events(&mut self) -> Vec<ServerEvent> {
        self.driver.drain_events_at(HUMAN_SEAT, date::now())
    }

    fn tick(&mut self) {
        self.driver.tick_at(date::now());
    }

    fn request_next_round(&mut self) {
        self.driver.next_round_at(date::now());
    }

    fn is_game_over(&self) -> bool {
        self.driver.is_game_over()
    }
}
