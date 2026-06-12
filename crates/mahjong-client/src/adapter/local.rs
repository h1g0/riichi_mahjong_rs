//! ローカルアダプター
//!
//! サーバとクライアントを同一プロセス内で直接接続する。
//! ゲーム進行とCPU処理は mahjong_server::driver::GameDriver に委譲する。

use mahjong_server::cpu::client::CpuConfig;
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::{ClientAction, ServerEvent};
use mahjong_server::table::GameSettings;

use super::GameAdapter;

/// 人間プレイヤーの座席インデックス（0 = 東家/親）
const HUMAN_SEAT: usize = 0;

/// ローカルアダプター: サーバを内蔵し、直接通信する
pub struct LocalAdapter {
    driver: GameDriver,
}

impl LocalAdapter {
    /// 指定したCPU設定でアダプターを作成する
    ///
    /// 人間は座席0、CPUは座席1〜3に割り当てる。
    pub fn with_cpu_configs(cpu_configs: [CpuConfig; 3]) -> Self {
        let mut driver = GameDriver::new(GameSettings::default());
        for (i, config) in cpu_configs.into_iter().enumerate() {
            driver.set_cpu(HUMAN_SEAT + 1 + i, config);
        }
        LocalAdapter { driver }
    }

    /// ゲームを開始する（最初の局を開始）
    pub fn start_game(&mut self) {
        self.driver.start_game();
    }
}

impl GameAdapter for LocalAdapter {
    fn send_action(&mut self, action: ClientAction) {
        self.driver.handle_action(HUMAN_SEAT, action);
    }

    fn poll_events(&mut self) -> Vec<ServerEvent> {
        self.driver.drain_events(HUMAN_SEAT)
    }

    fn tick(&mut self) {
        self.driver.tick();
    }

    fn request_next_round(&mut self) {
        self.driver.next_round();
    }

    fn is_game_over(&self) -> bool {
        self.driver.is_game_over()
    }
}
