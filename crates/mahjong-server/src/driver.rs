//! ゲームドライバー
//!
//! `Table` と CPU クライアントを束ね、イベントポンプを回す同期ロジック。
//! I/O を持たないため、ローカル対戦（クライアント内蔵）と
//! オンライン対戦（ネットワークサーバのルーム）の両方から再利用できる。
//!
//! CPU プレイヤーは人間と同じプロトコル（ServerEvent / ClientAction）で
//! やり取りし、CPU が操作していない座席のイベントは座席ごとのバッファに溜める。
//!
//! CPU の打牌に思考時間を演出する遅延を入れる場合は
//! [`set_cpu_action_delay`](GameDriver::set_cpu_action_delay) で有効化し、
//! `*_at` 系メソッドで現在時刻（秒）を渡す。時刻を注入する設計のため
//! このクレートは時計の実装（macroquad 等）に依存しない。
//! 遅延を設定しない場合（ネットワークサーバなど）は即時に進行する。

use std::collections::VecDeque;

use crate::cpu::client::{CpuClient, CpuConfig};
use crate::protocol::{ClientAction, ServerEvent};
use crate::round::TurnPhase;
use crate::table::{GameSettings, Table};

/// 思考待ち中のCPUアクション（同一イベント処理で生じた応答の単位）
///
/// 同じイベント処理で生じた応答は、鳴き解決などのため同時に適用する。
struct PendingCpuActionBatch {
    actions: Vec<(usize, ClientAction)>,
    ready_at: f64,
}

/// 座席に割り当てられたCPUクライアント
struct CpuSeat {
    client: CpuClient,
    /// この座席をCPUが操作するか
    ///
    /// false の場合は「シャドーCPU」: イベントを受け取って内部状態を
    /// 追跡するだけで、アクションは出さない（人間が操作する座席）。
    /// 切断時に true に切り替えるだけで即座に代打ちできる。
    controlled: bool,
}

/// ゲームドライバー: 卓とCPUクライアントを所有し、ゲームを進行する
pub struct GameDriver {
    table: Table,
    /// 各座席のCPUクライアント（Noneなら人間のみの座席）
    cpus: [Option<CpuSeat>; 4],
    /// CPUが操作していない座席向けのイベントバッファ
    event_buffers: [Vec<ServerEvent>; 4],
    /// CPUアクションの適用間隔（秒）。Noneなら即時適用
    action_delay: Option<f64>,
    /// 思考待ち中のCPUアクション（イベント処理単位のFIFO）
    pending_cpu_batches: VecDeque<PendingCpuActionBatch>,
}

impl GameDriver {
    /// 全座席が人間のドライバーを作成する
    pub fn new(settings: GameSettings) -> Self {
        GameDriver {
            table: Table::new(settings),
            cpus: [None, None, None, None],
            event_buffers: [const { Vec::new() }; 4],
            action_delay: None,
            pending_cpu_batches: VecDeque::new(),
        }
    }

    /// 指定した座席にCPUクライアントを割り当てる（CPUが操作する）
    pub fn set_cpu(&mut self, seat: usize, config: CpuConfig) {
        if seat < 4 {
            self.cpus[seat] = Some(CpuSeat {
                client: CpuClient::new(config),
                controlled: true,
            });
        }
    }

    /// 指定した座席にシャドーCPUを割り当てる
    ///
    /// イベントを配信して内部状態を追跡させるが、アクションは出さない。
    /// 人間の座席に割り当てておくと、切断時に
    /// [`set_cpu_controlled`](Self::set_cpu_controlled) で即座に代打ちできる。
    pub fn set_shadow_cpu(&mut self, seat: usize, config: CpuConfig) {
        if seat < 4 {
            self.cpus[seat] = Some(CpuSeat {
                client: CpuClient::new(config),
                controlled: false,
            });
        }
    }

    /// 指定した座席のCPU操作を切り替える
    ///
    /// CPUクライアントが割り当てられていない座席は切り替えられず false を返す。
    pub fn set_cpu_controlled(&mut self, seat: usize, controlled: bool) -> bool {
        match self.cpus.get_mut(seat) {
            Some(Some(cpu)) => {
                cpu.controlled = controlled;
                true
            }
            _ => false,
        }
    }

    /// 指定した座席をCPUが操作しているか
    pub fn is_cpu_controlled(&self, seat: usize) -> bool {
        matches!(self.cpus.get(seat), Some(Some(cpu)) if cpu.controlled)
    }

    /// CPUアクションの適用に遅延（秒）を設定する
    ///
    /// 有効にした場合は `tick_at` / `handle_action_at` / `drain_events_at` /
    /// `next_round_at` で現在時刻を渡して進行させること。
    pub fn set_cpu_action_delay(&mut self, seconds: f64) {
        self.action_delay = Some(seconds);
    }

    /// 卓への参照を取得する
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// 卓への可変参照を取得する
    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    /// ゲームを開始する（最初の局を開始）
    pub fn start_game(&mut self) {
        self.pending_cpu_batches.clear();
        self.table.start_round();
        self.pump(None);
    }

    /// シード値を指定してゲームを開始する（テスト・再現用）
    pub fn start_game_with_seed(&mut self, seed: u64) {
        self.pending_cpu_batches.clear();
        self.table.start_round_with_seed(seed);
        self.pump(None);
    }

    /// 指定した座席のイベントを取得する（CPU操作中の座席は常に空）
    pub fn drain_events(&mut self, seat: usize) -> Vec<ServerEvent> {
        self.drain_events_impl(seat, None)
    }

    /// 現在時刻を渡してイベントを取得する（CPU遅延が有効な場合に使う）
    pub fn drain_events_at(&mut self, seat: usize, now: f64) -> Vec<ServerEvent> {
        self.drain_events_impl(seat, Some(now))
    }

    fn drain_events_impl(&mut self, seat: usize, now: Option<f64>) -> Vec<ServerEvent> {
        // まず未処理イベントを処理
        self.pump(now);

        match self.event_buffers.get_mut(seat) {
            Some(buffer) => std::mem::take(buffer),
            None => Vec::new(),
        }
    }

    /// 指定した座席のアクションを処理する
    ///
    /// 手番違いやフェーズ違いなど無効なアクションは `false` を返す。
    pub fn handle_action(&mut self, seat: usize, action: ClientAction) -> bool {
        self.handle_action_impl(seat, action, None)
    }

    /// 現在時刻を渡してアクションを処理する（CPU遅延が有効な場合に使う）
    pub fn handle_action_at(&mut self, seat: usize, action: ClientAction, now: f64) -> bool {
        self.handle_action_impl(seat, action, Some(now))
    }

    fn handle_action_impl(&mut self, seat: usize, action: ClientAction, now: Option<f64>) -> bool {
        let accepted = self.table.handle_action(seat, action);
        // アクション後のイベントを処理
        self.pump(now);
        accepted
    }

    /// ゲームを1ティック進める
    /// - Drawフェーズ: ツモを実行（全プレイヤー共通）
    /// - 人間プレイヤーの手番ならUIで入力待ち
    /// - CPUプレイヤーの手番ならイベント配信で自動判断
    pub fn tick(&mut self) {
        self.tick_impl(None);
    }

    /// 現在時刻を渡してゲームを1ティック進める（CPU遅延が有効な場合に使う）
    pub fn tick_at(&mut self, now: f64) {
        self.tick_impl(Some(now));
    }

    fn tick_impl(&mut self, now: Option<f64>) {
        // CPUが操作したティックでは次のツモまで進めず、操作順を画面に反映する。
        if self.pump(now) || !self.pending_cpu_batches.is_empty() {
            return;
        }

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
            TurnPhase::WaitForDiscard
            | TurnPhase::WaitForCalls
            | TurnPhase::WaitForNineTerminals
            | TurnPhase::RoundOver => {
                // 人間プレイヤーの入力待ち or 既に処理済み
            }
        }

        // 生成されたイベントを処理（CPU配信 + 人間バッファリング）
        self.pump(now);
    }

    /// 人間の入力が必要になるか局が終わるまでゲームを進める
    ///
    /// ツモフェーズを繰り返し実行し、CPUのアクションはイベントポンプで
    /// 自動処理する。フレームループを持たないネットワークサーバ向けで、
    /// CPU遅延の設定に関わらず即時に進行する。牌山が有限なので必ず停止する。
    pub fn run_until_blocked(&mut self) {
        loop {
            let round = match self.table.current_round() {
                Some(r) => r,
                None => return,
            };
            if round.is_over() || round.phase != TurnPhase::Draw {
                return;
            }
            self.tick();
        }
    }

    /// 指定した座席が入力待ちなら既定のアクション（ツモ切り/パス/続行）を実行する
    ///
    /// CPU代打ちへの切り替え直後や行動タイムアウト時に、入力待ちで
    /// 停止したゲームを進めるために使う。実行したら true を返す。
    pub fn force_default_action(&mut self, seat: usize) -> bool {
        let action = {
            let round = match self.table.current_round() {
                Some(r) => r,
                None => return false,
            };
            if round.is_over() {
                return false;
            }
            match round.phase {
                TurnPhase::WaitForDiscard if round.current_player == seat => {
                    ClientAction::Discard { tile: None }
                }
                TurnPhase::WaitForCalls => {
                    let pending = round
                        .call_state
                        .as_ref()
                        .map(|cs| !cs.responded[seat])
                        .unwrap_or(false);
                    if !pending {
                        return false;
                    }
                    ClientAction::Pass
                }
                TurnPhase::WaitForNineTerminals if round.current_player == seat => {
                    ClientAction::NineTerminals { declare: false }
                }
                _ => return false,
            }
        };
        self.handle_action(seat, action)
    }

    /// 現在の局が終了しているか
    pub fn is_round_over(&self) -> bool {
        match self.table.current_round() {
            Some(r) => r.is_over(),
            None => true,
        }
    }

    /// 次の局を開始する
    pub fn next_round(&mut self) {
        self.next_round_impl(None);
    }

    /// 現在時刻を渡して次の局を開始する（CPU遅延が有効な場合に使う）
    pub fn next_round_at(&mut self, now: f64) {
        self.next_round_impl(Some(now));
    }

    fn next_round_impl(&mut self, now: Option<f64>) {
        self.pending_cpu_batches.clear();
        self.table.finish_round();
        if !self.table.is_game_over {
            self.table.start_round();
            // 新局のイベントを処理
            self.pump(now);
        }
    }

    /// ゲームが終了しているか
    pub fn is_game_over(&self) -> bool {
        self.table.is_game_over
    }

    /// イベントポンプを回す。CPUがアクションを適用したら true を返す
    ///
    /// 遅延が設定され `now` が渡された場合はペース制御付きで1回分だけ処理し、
    /// それ以外は静止するまで即時にループする。
    fn pump(&mut self, now: Option<f64>) -> bool {
        match (self.action_delay, now) {
            (Some(delay), Some(now)) => self.pump_paced(delay, now),
            _ => {
                self.pump_immediate();
                false
            }
        }
    }

    /// サーバからイベントを取得し、CPUに配信しつつ人間イベントをバッファする
    ///
    /// CPUのアクションが新たなイベントを生成する可能性があるためループする。
    fn pump_immediate(&mut self) {
        // 残っている待機バッチがあれば先に適用する（遅延未使用なら常に空）
        while let Some(batch) = self.pending_cpu_batches.pop_front() {
            for (seat, action) in batch.actions {
                self.table.handle_action(seat, action);
            }
        }

        loop {
            let all_events = self.table.drain_events();
            if all_events.is_empty() {
                break;
            }

            self.buffer_human_events(&all_events);

            // CPUプレイヤーにイベントを配信してアクションを収集
            let cpu_actions = self.collect_cpu_actions(&all_events);

            if cpu_actions.is_empty() {
                break;
            }

            // CPUのアクションをサーバに送信（これが新たなイベントを生成する）
            for (seat, action) in cpu_actions {
                self.table.handle_action(seat, action);
            }
        }
    }

    /// ペース制御付きでイベントを1回分処理する
    ///
    /// CPUのアクションは即時適用せず待機キューへ追加し、`ready_at` を
    /// 迎えたバッチを1ティックに1つだけ適用する。
    fn pump_paced(&mut self, delay: f64, now: f64) -> bool {
        let cpu_acted = self.apply_ready_cpu_batch(now);

        let all_events = self.table.drain_events();
        if all_events.is_empty() {
            return cpu_acted;
        }

        self.buffer_human_events(&all_events);

        // CPUプレイヤーにイベントを配信してアクションを収集
        let cpu_actions = self.collect_cpu_actions(&all_events);
        self.schedule_cpu_actions(cpu_actions, delay, now);

        cpu_acted
    }

    /// CPUが操作していない座席のイベントをバッファに追加する
    fn buffer_human_events(&mut self, events: &[(usize, ServerEvent)]) {
        for (seat, event) in events {
            if !self.is_cpu_controlled(*seat) {
                self.event_buffers[*seat].push(event.clone());
            }
        }
    }

    /// CPUアクションのバッチを待機キューへ追加する
    fn schedule_cpu_actions(&mut self, actions: Vec<(usize, ClientAction)>, delay: f64, now: f64) {
        if actions.is_empty() {
            return;
        }

        let ready_at = self
            .pending_cpu_batches
            .back()
            .map_or(now + delay, |pending| pending.ready_at.max(now) + delay);

        self.pending_cpu_batches
            .push_back(PendingCpuActionBatch { actions, ready_at });
    }

    /// 適用時刻を迎えた先頭のバッチを1つだけ適用する
    fn apply_ready_cpu_batch(&mut self, now: f64) -> bool {
        let Some(pending) = self.pending_cpu_batches.front() else {
            return false;
        };
        if pending.ready_at > now {
            return false;
        }

        let pending = self.pending_cpu_batches.pop_front().unwrap();
        for (seat, action) in pending.actions {
            self.table.handle_action(seat, action);
        }

        true
    }

    /// CPUクライアントにイベントを配信し、操作中の座席のアクションを収集する
    ///
    /// シャドーCPU（controlled = false）にもイベントは配信するが、
    /// 返ってきたアクションは捨てる。
    fn collect_cpu_actions(
        &mut self,
        events: &[(usize, ServerEvent)],
    ) -> Vec<(usize, ClientAction)> {
        let mut actions = Vec::new();

        for (seat, event) in events {
            if let Some(cpu) = &mut self.cpus[*seat]
                && let Some(action) = cpu.client.handle_event(event)
                && cpu.controlled
            {
                actions.push((*seat, action));
            }
        }

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::personalities::default_cpu_configs;
    use crate::player::Player;
    use mahjong_core::hand::Hand;
    use mahjong_core::tile::Tile;

    /// 座席0だけ人間、残りをCPUにしたドライバーを作成する
    fn driver_with_three_cpus() -> GameDriver {
        let mut driver = GameDriver::new(GameSettings::default());
        let configs = default_cpu_configs();
        for (i, config) in configs.into_iter().enumerate() {
            driver.set_cpu(i + 1, config);
        }
        driver
    }

    /// シード指定で開始した局が人間座席にイベントを届けることを確認するテスト
    #[test]
    fn test_seeded_round_delivers_events_to_human_seat() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);

        let events = driver.drain_events(0);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::GameStarted { .. })),
            "GameStartedイベントが人間座席に届いていない"
        );

        // CPU座席のバッファは常に空
        for seat in 1..4 {
            assert!(driver.drain_events(seat).is_empty());
        }
    }

    /// 人間が全ツモ切りで局が最後まで進行することを確認するテスト
    #[test]
    fn test_seeded_round_runs_to_completion_with_tsumogiri_human() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);
        let _ = driver.drain_events(0);

        // 1局は十分に終わる回数だけ回す（無限ループ防止の上限つき）
        for _ in 0..1000 {
            if driver.is_round_over() {
                break;
            }

            driver.tick();
            let events = driver.drain_events(0);

            for event in &events {
                match event {
                    ServerEvent::TileDrawn { can_tsumo, .. } => {
                        // ツモ切り（和了可能ならツモ和了）
                        if *can_tsumo {
                            driver.handle_action(0, ClientAction::Tsumo);
                        } else {
                            driver.handle_action(0, ClientAction::Discard { tile: None });
                        }
                    }
                    ServerEvent::CallAvailable { .. } => {
                        driver.handle_action(0, ClientAction::Pass);
                    }
                    ServerEvent::NineTerminalsAvailable => {
                        driver.handle_action(0, ClientAction::NineTerminals { declare: false });
                    }
                    _ => {}
                }
            }
        }

        assert!(driver.is_round_over(), "局が終了しなかった");

        // 局終了イベント（和了または流局）が人間座席に届いている
        let round = driver.table().current_round().unwrap();
        assert!(round.result.is_some(), "局結果が設定されていない");
    }

    /// シャドーCPUはイベントを受け取ってもアクションを出さないことを確認
    #[test]
    fn test_shadow_cpu_does_not_act() {
        let mut driver = driver_with_three_cpus();
        // 座席0にシャドーCPUを割り当てる（人間扱いのまま）
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);
        assert!(!driver.is_cpu_controlled(0));

        driver.start_game_with_seed(42);
        driver.run_until_blocked();

        // 座席0の入力待ちで停止している（シャドーCPUが勝手に打牌していない）
        let round = driver.table().current_round().unwrap();
        assert!(!round.is_over());
        assert_eq!(round.current_player, 0);
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);

        // 座席0にはイベントが届いている
        let events = driver.drain_events(0);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::TileDrawn { .. })),
            "シャドーCPU座席にTileDrawnが届いていない"
        );
    }

    /// シャドーCPUを操作状態に切り替えると局が自動進行することを確認
    #[test]
    fn test_shadow_cpu_takeover_completes_round() {
        let mut driver = driver_with_three_cpus();
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);

        driver.start_game_with_seed(42);
        driver.run_until_blocked();
        assert!(!driver.is_round_over());

        // 代打ちに切り替え
        assert!(driver.set_cpu_controlled(0, true));
        assert!(driver.is_cpu_controlled(0));

        // 切り替え時点で座席0は打牌待ちのため、既定アクションで進める。
        // 以降は全席CPUなので局は最後まで自動進行する。
        assert!(driver.force_default_action(0));
        driver.run_until_blocked();

        assert!(driver.is_round_over(), "代打ち後に局が進行しなかった");
    }

    /// CPUクライアント未割り当ての座席は操作切り替えできないことを確認
    #[test]
    fn test_set_cpu_controlled_requires_cpu_client() {
        let mut driver = GameDriver::new(GameSettings::default());
        assert!(!driver.set_cpu_controlled(0, true));
        assert!(!driver.is_cpu_controlled(0));
    }

    /// run_until_blocked が人間の打牌待ちで停止することを確認
    #[test]
    fn test_run_until_blocked_stops_at_human_turn() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(123);
        driver.run_until_blocked();

        let round = driver.table().current_round().unwrap();
        if !round.is_over() {
            // 停止位置はDrawフェーズ以外（人間入力待ちか局終了）
            assert_ne!(round.phase, TurnPhase::Draw);
        }
    }

    /// カン後にゲームが進行できることを確認するテスト
    #[test]
    fn test_kan_advances_game() {
        let mut driver = driver_with_three_cpus();

        // ゲームを開始
        driver.table_mut().start_round();

        // 座席0 (人間) の手牌を「暗カン可能」なものに設定
        {
            let round = driver.table_mut().current_round_mut().unwrap();
            let seat_wind = round.players[0].seat_wind;
            // 1mが3枚(main)+1枚(drawn)=4枚 → 暗カン可能
            let hand = Hand::from("2p3p4p5s6s7s7m8m9m1m1m1m 1m");
            round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
            round.players[0].draw(hand.drawn().unwrap());
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        // 初期イベントを処理
        driver.pump(None);
        let _ = driver.drain_events(0);

        // 座席0 が暗カンを実行
        let kan_result = driver.handle_action(
            0,
            ClientAction::Kan {
                tile_index: Tile::M1 as usize,
            },
        );
        assert!(kan_result, "カンが失敗した");

        // カン後の状態確認
        {
            let round = driver.table().current_round().unwrap();
            assert_eq!(
                round.phase,
                TurnPhase::WaitForDiscard,
                "カン後のフェーズがWaitForDiscardでない"
            );
            assert_eq!(round.current_player, 0, "カン後の現在プレイヤーが0でない");
            assert!(
                round.players[0].hand.drawn().is_some(),
                "カン後に嶺上牌が設定されていない"
            );
        }

        // イベントを取得
        let events = driver.drain_events(0);
        let has_tile_drawn = events
            .iter()
            .any(|e| matches!(e, ServerEvent::TileDrawn { .. }));
        assert!(
            has_tile_drawn,
            "カン後にTileDrawnイベントが来なかった: {:?}",
            events
                .iter()
                .map(std::mem::discriminant)
                .collect::<Vec<_>>()
        );

        // 打牌して進行できることを確認
        let discard_result = driver.handle_action(0, ClientAction::Discard { tile: None });
        assert!(discard_result, "カン後の打牌が失敗した");
    }

    /// CPUプレイヤーがカンした後にゲームが正しく進行することを確認
    #[test]
    fn test_cpu_kan_advances_game() {
        let mut driver = driver_with_three_cpus();

        driver.table_mut().start_round();

        // 座席1 (CPU) の手牌を「暗カン可能」なものに設定
        {
            let round = driver.table_mut().current_round_mut().unwrap();
            let seat_wind = round.players[1].seat_wind;
            let hand = Hand::from("2p3p4p5s6s7s7m8m9m1m1m1m 1m");
            round.players[1] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
            round.players[1].draw(hand.drawn().unwrap());
            round.current_player = 1;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        // イベント処理（CPUが自動的にカンまたは打牌する）
        driver.pump(None);

        // ゲームが進行した（RoundOverでなくWaitForDiscardかDrawになっている）ことを確認
        let phase = {
            let round = driver.table().current_round().unwrap();
            round.phase.clone()
        };
        assert!(
            phase == TurnPhase::WaitForDiscard
                || phase == TurnPhase::Draw
                || phase == TurnPhase::WaitForCalls,
            "CPUカン後にゲームが詰まった: フェーズ = {:?}",
            phase
        );

        // さらに10ターン分ゲームを進める（フリーズしないことを確認）
        for _i in 0..10 {
            driver.tick();
            let _ = driver.drain_events(0);
            {
                let round = driver.table().current_round().unwrap();
                if round.is_over() {
                    break;
                }
            }
            // WaitForDiscardかつ人間の番なら打牌
            {
                let (phase, current_player) = {
                    let round = driver.table().current_round().unwrap();
                    (round.phase.clone(), round.current_player)
                };
                if phase == TurnPhase::WaitForDiscard && current_player == 0 {
                    driver.handle_action(0, ClientAction::Discard { tile: None });
                }
            }
            // WaitForCallsかつ人間に鳴き機会があればパス
            {
                let (phase, human_responded) = {
                    let round = driver.table().current_round().unwrap();
                    let responded = round
                        .call_state
                        .as_ref()
                        .map(|cs| cs.responded[0])
                        .unwrap_or(true);
                    (round.phase.clone(), responded)
                };
                if phase == TurnPhase::WaitForCalls && !human_responded {
                    driver.handle_action(0, ClientAction::Pass);
                }
            }
        }
    }

    /// 全席人間の場合はCPUが動かず、各席にイベントが届くことを確認
    #[test]
    fn test_all_human_seats_buffer_events_independently() {
        let mut driver = GameDriver::new(GameSettings::default());
        driver.start_game_with_seed(7);

        for seat in 0..4 {
            let events = driver.drain_events(seat);
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, ServerEvent::GameStarted { .. })),
                "座席{}にGameStartedが届いていない",
                seat
            );
        }
    }

    /// CPUアクションが設定した遅延の後に適用されることを確認
    #[test]
    fn test_cpu_action_is_applied_after_configured_delay() {
        let mut driver = driver_with_three_cpus();
        driver.set_cpu_action_delay(1.0);
        driver.table_mut().start_round();

        {
            let round = driver.table_mut().current_round_mut().unwrap();
            round.current_player = 1;
            round.phase = TurnPhase::WaitForDiscard;
            round.players[1].draw(Tile::new(Tile::Z7));
            round.drain_events();
        }

        driver.schedule_cpu_actions(vec![(1, ClientAction::Discard { tile: None })], 1.0, 10.0);

        driver.pump(Some(10.999));
        assert_eq!(
            driver.table().current_round().unwrap().phase,
            TurnPhase::WaitForDiscard
        );

        driver.pump(Some(11.0));
        assert_ne!(
            driver.table().current_round().unwrap().phase,
            TurnPhase::WaitForDiscard
        );
    }

    /// 1ティックに1バッチしか適用されないことを確認
    #[test]
    fn test_only_one_cpu_action_batch_is_applied_per_tick() {
        let mut driver = driver_with_three_cpus();
        driver.set_cpu_action_delay(1.0);
        driver.table_mut().start_round();

        {
            let round = driver.table_mut().current_round_mut().unwrap();
            for player_idx in [0, 2, 3] {
                let seat_wind = round.players[player_idx].seat_wind;
                let score = round.players[player_idx].score;
                round.players[player_idx] = Player::new(seat_wind, Vec::new(), score);
            }
            round.current_player = 1;
            round.phase = TurnPhase::WaitForDiscard;
            round.players[1].draw(Tile::new(Tile::Z7));
            round.drain_events();
        }

        driver.schedule_cpu_actions(vec![(1, ClientAction::Discard { tile: None })], 1.0, 10.0);
        driver.schedule_cpu_actions(vec![(2, ClientAction::Discard { tile: None })], 1.0, 10.0);

        driver.tick_at(11.0);

        let round = driver.table().current_round().unwrap();
        assert_eq!(round.current_player, 2);
        assert_eq!(round.phase, TurnPhase::Draw);
        assert_eq!(driver.pending_cpu_batches.len(), 1);
        assert_eq!(driver.pending_cpu_batches.front().unwrap().ready_at, 12.0);
    }

    /// 遅延未設定なら待機キューを使わず即時進行することを確認
    #[test]
    fn test_no_delay_keeps_immediate_progression() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);
        driver.tick();
        assert!(driver.pending_cpu_batches.is_empty());
    }
}
