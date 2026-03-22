//! ローカルアダプター
//!
//! サーバとクライアントを同一プロセス内で直接接続する。
//! CPUプレイヤーは人間と同じプロトコル（ServerEvent / ClientAction）で
//! サーバとやり取りする。

use mahjong_server::cpu::client::{CpuClient, CpuConfig};
use mahjong_server::cpu::personalities::default_cpu_configs;
use mahjong_server::protocol::{ClientAction, ServerEvent};
use mahjong_server::round::TurnPhase;
use mahjong_server::table::{GameSettings, Table};

/// ローカルアダプター: サーバを内蔵し、直接通信する
pub struct LocalAdapter {
    table: Table,
    /// 人間プレイヤーのインデックス（0 = 東家/親）
    pub human_player: usize,
    /// CPUクライアント（人間以外の3人）
    cpu_clients: [Option<CpuClient>; 4],
    /// 人間プレイヤー向けのイベントバッファ
    human_event_buffer: Vec<ServerEvent>,
}

impl LocalAdapter {
    /// デフォルト設定でアダプターを作成する
    #[allow(dead_code)]
    pub fn new() -> Self {
        let configs = default_cpu_configs();
        Self::with_cpu_configs(configs)
    }

    /// 指定したCPU設定でアダプターを作成する
    pub fn with_cpu_configs(cpu_configs: [CpuConfig; 3]) -> Self {
        let human_player = 0;

        // 人間以外のプレイヤーにCPUクライアントを割り当て
        let mut cpu_clients: [Option<CpuClient>; 4] = [None, None, None, None];
        let mut config_idx = 0;
        for i in 0..4 {
            if i != human_player {
                cpu_clients[i] = Some(CpuClient::new(cpu_configs[config_idx].clone()));
                config_idx += 1;
            }
        }

        LocalAdapter {
            table: Table::new(GameSettings::default()),
            human_player,
            cpu_clients,
            human_event_buffer: Vec::new(),
        }
    }

    /// CPU設定を変更する
    #[allow(dead_code)]
    pub fn set_cpu_config(&mut self, player_idx: usize, config: CpuConfig) {
        if player_idx != self.human_player && player_idx < 4 {
            self.cpu_clients[player_idx] = Some(CpuClient::new(config));
        }
    }

    /// ゲームを開始する（最初の局を開始）
    pub fn start_game(&mut self) {
        self.table.start_round();
        // 局開始時のイベントをCPUに配信（人間イベントはバッファへ）
        self.process_all_events();
    }

    /// 人間プレイヤー向けのイベントを取得する
    pub fn poll_events(&mut self, player_idx: usize) -> Vec<ServerEvent> {
        // まず未処理イベントを処理
        self.process_all_events();

        if player_idx == self.human_player {
            std::mem::take(&mut self.human_event_buffer)
        } else {
            Vec::new()
        }
    }

    /// 人間プレイヤーのアクションを送信する
    pub fn send_action(&mut self, action: ClientAction) {
        self.table.handle_action(self.human_player, action);
        // アクション後のイベントを処理
        self.process_all_events();
    }

    /// ゲームを1ティック進める
    /// - Drawフェーズ: ツモを実行（全プレイヤー共通）
    /// - 人間プレイヤーの手番ならUIで入力待ち
    /// - CPUプレイヤーの手番ならイベント配信で自動判断
    pub fn tick(&mut self) {
        let round = match self.table.current_round_mut() {
            Some(r) => r,
            None => return,
        };

        if round.is_over() {
            return;
        }

        match round.phase {
            TurnPhase::Draw => {
                // ツモフェーズ: 牌を引く（全プレイヤー共通）
                round.do_draw();
            }
            TurnPhase::WaitForDiscard | TurnPhase::WaitForCalls | TurnPhase::RoundOver => {
                // 人間プレイヤーの入力待ち or 既に処理済み
            }
        }

        // 生成されたイベントを処理（CPU配信 + 人間バッファリング）
        self.process_all_events();
    }

    /// サーバからイベントを取得し、CPUに配信しつつ人間イベントをバッファする
    ///
    /// CPUのアクションが新たなイベントを生成する可能性があるためループする。
    fn process_all_events(&mut self) {
        loop {
            let all_events = self.table.drain_events();
            if all_events.is_empty() {
                break;
            }

            // 人間プレイヤーのイベントをバッファに追加
            for (idx, event) in &all_events {
                if *idx == self.human_player {
                    self.human_event_buffer.push(event.clone());
                }
            }

            // CPUプレイヤーにイベントを配信してアクションを収集
            let cpu_actions = self.collect_cpu_actions(&all_events);

            if cpu_actions.is_empty() {
                break;
            }

            // CPUのアクションをサーバに送信（これが新たなイベントを生成する）
            for (player_idx, action) in cpu_actions {
                self.table.handle_action(player_idx, action);
            }
        }
    }

    /// CPUプレイヤーにイベントを配信し、アクションを収集する
    fn collect_cpu_actions(
        &mut self,
        events: &[(usize, ServerEvent)],
    ) -> Vec<(usize, ClientAction)> {
        let mut actions = Vec::new();

        for (player_idx, event) in events {
            if *player_idx == self.human_player {
                continue;
            }

            if let Some(cpu) = &mut self.cpu_clients[*player_idx] {
                if let Some(action) = cpu.handle_event(event) {
                    actions.push((*player_idx, action));
                }
            }
        }

        actions
    }

    /// 現在の局が終了しているか
    #[allow(dead_code)]
    pub fn is_round_over(&self) -> bool {
        match self.table.current_round() {
            Some(r) => r.is_over(),
            None => true,
        }
    }

    /// 次の局を開始する
    pub fn next_round(&mut self) {
        self.table.finish_round();
        if !self.table.is_game_over {
            self.table.start_round();
            // 新局のイベントを処理
            self.process_all_events();
        }
    }

    /// ゲームが終了しているか
    pub fn is_game_over(&self) -> bool {
        self.table.is_game_over
    }

    /// 人間プレイヤーの手番か（打牌待ち）
    #[allow(dead_code)]
    pub fn is_human_turn(&self) -> bool {
        match self.table.current_round() {
            Some(r) => {
                r.current_player == self.human_player
                    && r.phase == TurnPhase::WaitForDiscard
            }
            None => false,
        }
    }

    /// 鳴き待ちフェーズで人間プレイヤーに鳴き候補があるか
    #[allow(dead_code)]
    pub fn is_human_call_pending(&self) -> bool {
        match self.table.current_round() {
            Some(r) => {
                if r.phase != TurnPhase::WaitForCalls {
                    return false;
                }
                if let Some(ref cs) = r.call_state {
                    !cs.responded[self.human_player]
                } else {
                    false
                }
            }
            None => false,
        }
    }
}
