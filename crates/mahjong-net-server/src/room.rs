//! ルームアクター
//!
//! 1ルーム = 1 tokio タスク。ルームが `GameDriver`（卓 + CPU）を所有し、
//! 接続タスクからの `RoomMsg` を mpsc で逐次処理する。
//! 同期的な卓の操作が await をまたがないため、ゲーム状態のロックは不要。

use std::time::Duration;

use mahjong_server::cpu::personalities::default_cpu_configs;
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::ServerEvent;
use mahjong_server::protocol::net::{ClientMessage, ErrorCode, SeatInfo, ServerMessage};
use mahjong_server::table::GameSettings;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::lobby::Lobby;

/// ルームの動作タイミング設定
///
/// 本番は `Default`、テストでは短い値に差し替える。
#[derive(Debug, Clone, Copy)]
pub struct RoomConfig {
    /// 局結果画面からの自動進行までの猶予
    pub ready_timeout: Duration,
    /// 対局開始前のルームの生存期間
    pub lobby_timeout: Duration,
    /// 対局中に全員切断してからルームを破棄するまでの猶予
    pub abandoned_timeout: Duration,
    /// 1手番ごとの制限時間（None なら無制限）。
    /// 超過するとサーバが既定アクション（ツモ切り/パス）を代行する。
    pub action_timeout: Option<Duration>,
    /// CPUの打牌間隔（思考時間の演出）。0 なら即時に進行する。
    pub cpu_action_delay: Duration,
    /// ゲーム進行ティックの間隔。CPU遅延を計りながら進める粒度。
    pub tick_interval: Duration,
}

impl Default for RoomConfig {
    fn default() -> Self {
        RoomConfig {
            ready_timeout: Duration::from_secs(60),
            lobby_timeout: Duration::from_secs(30 * 60),
            abandoned_timeout: Duration::from_secs(5 * 60),
            action_timeout: Some(Duration::from_secs(90)),
            cpu_action_delay: Duration::from_secs(1),
            tick_interval: Duration::from_millis(100),
        }
    }
}

/// 接続タスクからルームアクターへのメッセージ
pub enum RoomMsg {
    /// 入室要求
    Join {
        /// 表示名
        name: String,
        /// セッショントークン
        token: String,
        /// この接続への送信チャネル
        tx: mpsc::Sender<ServerMessage>,
        /// 割り当てた座席と接続世代（またはエラー）の返信先
        reply: oneshot::Sender<Result<(usize, u64), ErrorCode>>,
    },
    /// 座席からのクライアントメッセージ
    FromSeat {
        /// 座席インデックス
        seat: usize,
        /// メッセージ本体
        msg: ClientMessage,
    },
    /// 明示的な退出
    Leave {
        /// 座席インデックス
        seat: usize,
    },
    /// 切断（ソケットが閉じた）
    Disconnected {
        /// 座席インデックス
        seat: usize,
        /// 切断した接続の世代番号（再接続との行き違いを防ぐ）
        conn_gen: u64,
    },
}

/// 入室の結果（接続タスクへ返す座席情報とルーム内部の追加情報）
struct JoinOutcome {
    seat: usize,
    conn_gen: u64,
    /// 既存の座席への再接続か（新規入室なら false）
    reconnect: bool,
}

/// 着席中のプレイヤー
struct Seat {
    /// セッショントークン（再接続時の照合に使う）
    token: String,
    name: String,
    /// 接続への送信チャネル（None = 切断中）
    tx: Option<mpsc::Sender<ServerMessage>>,
    /// 現在の接続の世代番号（再接続のたびに更新）
    conn_gen: u64,
    /// 現在の局の GameStarted 以降のイベント履歴（再接続時の再同期用）
    history: Vec<ServerEvent>,
}

/// ホストの座席インデックス（最初の入室者）
const HOST_SEAT: usize = 0;

/// ルームの状態
struct Room {
    code: String,
    settings: GameSettings,
    config: RoomConfig,
    seats: [Option<Seat>; 4],
    driver: Option<GameDriver>,
    /// 局結果の確認待ち中か
    awaiting_ready: bool,
    /// 各座席の次局進行確認
    ready: [bool; 4],
    /// GameOver を送信済みか
    game_over_sent: bool,
    /// 次局自動進行の期限
    ready_deadline: Option<Instant>,
    /// ルーム破棄の期限
    close_deadline: Option<Instant>,
    /// 手番の制限時間の期限
    action_deadline: Option<Instant>,
    /// ゲーム進行の時刻基準（対局開始時刻）。CPU遅延の計測に使う
    game_clock: Option<Instant>,
    /// ルームを閉じるフラグ
    closing: bool,
    /// 次に割り当てる接続の世代番号
    next_conn_gen: u64,
}

/// ルームアクターのメインループ
pub async fn run_room(
    code: String,
    settings: GameSettings,
    lobby: Lobby,
    mut rx: mpsc::Receiver<RoomMsg>,
    config: RoomConfig,
) {
    let mut room = Room {
        code: code.clone(),
        settings,
        config,
        seats: [None, None, None, None],
        driver: None,
        awaiting_ready: false,
        ready: [false; 4],
        game_over_sent: false,
        ready_deadline: None,
        close_deadline: Some(Instant::now() + config.lobby_timeout),
        action_deadline: None,
        game_clock: None,
        closing: false,
        next_conn_gen: 0,
    };

    // CPU遅延を計りながらゲームを進めるためのティック
    let mut game_tick = tokio::time::interval(config.tick_interval);
    game_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        let ready_at = deadline_or_far(room.ready_deadline);
        let close_at = deadline_or_far(room.close_deadline);
        let action_at = deadline_or_far(room.action_deadline);

        tokio::select! {
            msg = rx.recv() => match msg {
                Some(msg) => room.handle_msg(msg).await,
                None => break,
            },
            _ = game_tick.tick(), if room.needs_game_tick() => {
                room.game_tick().await;
            }
            _ = tokio::time::sleep_until(ready_at), if room.ready_deadline.is_some() => {
                tracing::debug!(code = room.code, "ready timeout; auto-advancing round");
                room.advance_round().await;
            }
            _ = tokio::time::sleep_until(action_at), if room.action_deadline.is_some() => {
                tracing::debug!(code = room.code, "action timeout; forcing default action");
                room.on_action_timeout().await;
            }
            _ = tokio::time::sleep_until(close_at), if room.close_deadline.is_some() => {
                tracing::info!(code = room.code, "room expired");
                room.closing = true;
            }
        }

        if room.closing {
            break;
        }
    }

    lobby.remove(&code);
}

/// select! のために None を遠い未来の時刻に変換する
///
/// `if` ガードで無効化されるため、この時刻が実際に使われることはない。
fn deadline_or_far(deadline: Option<Instant>) -> Instant {
    deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(365 * 24 * 3600))
}

impl Room {
    /// ゲームが開始済みか
    fn game_started(&self) -> bool {
        self.driver.is_some()
    }

    async fn handle_msg(&mut self, msg: RoomMsg) {
        match msg {
            RoomMsg::Join {
                name,
                token,
                tx,
                reply,
            } => match self.try_join(name, token, tx) {
                Ok(outcome) => {
                    let _ = reply.send(Ok((outcome.seat, outcome.conn_gen)));
                    if outcome.reconnect {
                        self.handle_reconnect(outcome.seat).await;
                    } else {
                        self.broadcast_room_state().await;
                    }
                }
                Err(code) => {
                    let _ = reply.send(Err(code));
                }
            },
            RoomMsg::FromSeat { seat, msg } => self.handle_client_message(seat, msg).await,
            RoomMsg::Leave { seat } => self.handle_departure(seat).await,
            RoomMsg::Disconnected { seat, conn_gen } => {
                // 古い接続からの遅延切断通知は無視する（再接続済みなら世代が進んでいる）
                if self.seats[seat]
                    .as_ref()
                    .is_some_and(|s| s.conn_gen == conn_gen)
                {
                    self.handle_departure(seat).await;
                }
            }
        }
    }

    /// 接続の世代番号を1つ払い出す
    fn alloc_conn_gen(&mut self) -> u64 {
        let generation = self.next_conn_gen;
        self.next_conn_gen += 1;
        generation
    }

    /// 入室または再接続を処理する
    ///
    /// 対局中はトークンが一致する切断済みの座席へ再接続させる。
    /// 開始前は空席へ新規入室させる。
    fn try_join(
        &mut self,
        name: String,
        token: String,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<JoinOutcome, ErrorCode> {
        if self.game_started() {
            // 再接続: トークンが一致し切断中の座席を探す
            let seat = self
                .seats
                .iter()
                .position(|s| {
                    s.as_ref()
                        .is_some_and(|seat| seat.token == token && seat.tx.is_none())
                })
                .ok_or(ErrorCode::GameInProgress)?;
            let conn_gen = self.alloc_conn_gen();
            let s = self.seats[seat].as_mut().expect("position found");
            s.tx = Some(tx);
            s.name = name;
            s.conn_gen = conn_gen;
            tracing::info!(code = self.code, seat, "player reconnected");
            return Ok(JoinOutcome {
                seat,
                conn_gen,
                reconnect: true,
            });
        }

        // 新規入室: 空席へ
        let seat = self
            .seats
            .iter()
            .position(|s| s.is_none())
            .ok_or(ErrorCode::RoomFull)?;
        let conn_gen = self.alloc_conn_gen();
        self.seats[seat] = Some(Seat {
            token,
            name,
            tx: Some(tx),
            conn_gen,
            history: Vec::new(),
        });
        tracing::info!(code = self.code, seat, "player joined");
        Ok(JoinOutcome {
            seat,
            conn_gen,
            reconnect: false,
        })
    }

    /// 再接続した座席へ CPU 代打ちを止め、状態を再同期する
    async fn handle_reconnect(&mut self, seat: usize) {
        // CPU代打ちを止めて人間の操作に戻す
        if let Some(driver) = self.driver.as_mut() {
            driver.set_cpu_controlled(seat, false);
        }
        // 全員へ接続状態の変化を通知
        self.broadcast(ServerMessage::PlayerConnectionChanged {
            seat,
            connected: true,
        })
        .await;
        // 再接続した座席へ最新の RoomState と現在の局の再生を送る
        self.send_room_state_to(seat).await;
        let history = self.seats[seat]
            .as_ref()
            .map(|s| s.history.clone())
            .unwrap_or_default();
        if let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.clone()) {
            let _ = tx.send(ServerMessage::Resync { events: history }).await;
        }
        // 再接続で操作待ちの主体が変わった可能性があるため期限を張り直す
        self.refresh_action_deadline().await;
    }

    async fn handle_client_message(&mut self, seat: usize, msg: ClientMessage) {
        match msg {
            ClientMessage::StartGame => self.handle_start_game(seat).await,
            ClientMessage::Action(action) => {
                if !self.game_started() || self.awaiting_ready {
                    self.send_error(seat, ErrorCode::InvalidAction, "no action expected now")
                        .await;
                    return;
                }
                let now = self.now_secs();
                let driver = self.driver.as_mut().expect("checked above");
                let accepted = driver.handle_action_at(seat, action.clone(), now);
                if !accepted {
                    let phase = driver
                        .table()
                        .current_round()
                        .map(|r| format!("{:?}", r.phase));
                    self.send_error(
                        seat,
                        ErrorCode::InvalidAction,
                        &format!("action rejected: seat={seat} action={action:?} phase={phase:?}"),
                    )
                    .await;
                }
                self.progress_game().await;
            }
            ClientMessage::ReadyNextRound => {
                if !self.awaiting_ready {
                    // 自動進行タイマーと行き違いになった遅延確認は無害なので
                    // エラーにせず黙って無視する
                    return;
                }
                self.ready[seat] = true;
                if self.all_connected_humans_ready() {
                    self.advance_round().await;
                }
            }
            // Hello / CreateRoom / JoinRoom / LeaveRoom は接続タスク側で処理済み
            _ => {
                self.send_error(seat, ErrorCode::BadMessage, "unexpected message")
                    .await;
            }
        }
    }

    async fn handle_start_game(&mut self, seat: usize) {
        if seat != HOST_SEAT {
            self.send_error(seat, ErrorCode::NotHost, "only the host can start")
                .await;
            return;
        }
        if self.game_started() {
            self.send_error(seat, ErrorCode::GameInProgress, "game already started")
                .await;
            return;
        }

        let mut driver = GameDriver::new(self.settings.clone());
        let configs = default_cpu_configs();
        for s in 0..4 {
            let config = configs[s % configs.len()].clone();
            if self.seats[s].is_some() {
                // 人間の座席にもシャドーCPUを常駐させ、切断時に即代打ちできるようにする
                driver.set_shadow_cpu(s, config);
            } else {
                driver.set_cpu(s, config);
            }
        }
        driver.set_cpu_action_delay(self.config.cpu_action_delay.as_secs_f64());
        driver.start_game();
        self.driver = Some(driver);
        // CPU遅延を計る時刻基準を起動。以降は now_secs() で経過秒を渡す
        self.game_clock = Some(Instant::now());
        // 開始したのでロビーの生存期限は解除
        self.close_deadline = None;

        tracing::info!(code = self.code, "game started");
        self.broadcast_room_state().await;
        self.progress_game().await;
    }

    /// 直前のアクション結果を配信し、局終了の確認・期限の再設定を行う
    ///
    /// CPUの進行は [`game_tick`](Self::game_tick) が遅延を計りながら進めるため、
    /// ここではツモまで一気に進めない（イベントの送出と状態更新のみ）。
    async fn progress_game(&mut self) {
        self.flush_events().await;
        self.check_round_end();
        self.refresh_action_deadline().await;
    }

    /// CPU遅延を計りながらゲームを1ティック進める
    ///
    /// 待機中のCPUアクションが期限を迎えれば適用し、ツモフェーズなら牌を引く。
    /// `needs_game_tick` が true の間だけ呼ばれる。
    async fn game_tick(&mut self) {
        let now = self.now_secs();
        if let Some(driver) = self.driver.as_mut() {
            driver.tick_at(now);
        }
        self.flush_events().await;
        self.check_round_end();
        self.refresh_action_deadline().await;
    }

    /// CPUの進行（ツモ・遅延待ちの打牌など）のために tick が必要か
    fn needs_game_tick(&self) -> bool {
        if self.awaiting_ready || self.game_over_sent {
            return false;
        }
        self.driver.as_ref().is_some_and(|d| d.needs_tick())
    }

    /// ゲーム時刻基準からの経過秒（CPU遅延の計測に渡す）
    fn now_secs(&self) -> f64 {
        self.game_clock
            .map(|c| c.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// 指定した座席が接続中の人間か
    fn is_connected_human(&self, seat: usize) -> bool {
        self.seats[seat].as_ref().is_some_and(|s| s.tx.is_some())
    }

    /// 手番の制限時間を再設定する
    ///
    /// 接続中の人間の操作待ちなら期限を張り直し、対象座席へ残り秒数を通知する。
    /// それ以外（CPU進行中・確認待ち・局終了）は期限を解除する。
    async fn refresh_action_deadline(&mut self) {
        let Some(timeout) = self.config.action_timeout else {
            self.action_deadline = None;
            return;
        };
        if self.awaiting_ready || self.game_over_sent {
            self.action_deadline = None;
            return;
        }

        let seats: Vec<usize> = self
            .driver
            .as_ref()
            .map(|d| d.pending_action_seats())
            .unwrap_or_default()
            .into_iter()
            .filter(|&s| self.is_connected_human(s))
            .collect();

        if seats.is_empty() {
            self.action_deadline = None;
            return;
        }

        self.action_deadline = Some(Instant::now() + timeout);
        // 端数は切り上げる（短い制限でも 0 秒表示にならないように）
        let seconds = timeout.as_secs_f64().ceil() as u32;
        for seat in seats {
            if let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.clone()) {
                let _ = tx.send(ServerMessage::TurnTimer { seconds }).await;
            }
        }
    }

    /// 手番の制限時間切れ: 待っている接続中の人間に既定アクションを代行する
    async fn on_action_timeout(&mut self) {
        let now = self.now_secs();
        let seats: Vec<usize> = self
            .driver
            .as_ref()
            .map(|d| d.pending_action_seats())
            .unwrap_or_default();
        for seat in seats {
            if self.is_connected_human(seat)
                && let Some(driver) = self.driver.as_mut()
            {
                tracing::info!(code = self.code, seat, "action timed out; auto-acting");
                driver.force_default_action_at(seat, now);
            }
        }
        self.action_deadline = None;
        self.progress_game().await;
    }

    /// 各座席のイベントを履歴へ記録し、接続中の座席へ送信する
    ///
    /// 履歴は局開始（GameStarted）でリセットし、現在の局のイベントだけを
    /// 保持する。切断中の座席でも履歴は記録され、再接続時の再同期に使う。
    async fn flush_events(&mut self) {
        if self.driver.is_none() {
            return;
        }
        // 先に全座席のイベントを取り出す（driver の借用をここで手放す）。
        // drain_events_at は CPU遅延を計りながらイベントを処理する。
        let now = self.now_secs();
        let per_seat: [Vec<ServerEvent>; 4] = {
            let driver = self.driver.as_mut().expect("checked above");
            std::array::from_fn(|seat| driver.drain_events_at(seat, now))
        };

        for (seat, events) in per_seat.into_iter().enumerate() {
            // 履歴へ記録（局開始でリセット）して送信チャネルを取り出す
            let tx = {
                let Some(s) = self.seats[seat].as_mut() else {
                    continue;
                };
                for event in &events {
                    if matches!(event, ServerEvent::GameStarted { .. }) {
                        s.history.clear();
                    }
                    s.history.push(event.clone());
                }
                s.tx.clone()
            };

            // 接続中の座席へ送信する
            if let Some(tx) = tx {
                for event in events {
                    if tx.send(ServerMessage::Event(event)).await.is_err() {
                        // 送信失敗は切断として扱う（Disconnected が後続で届く）
                        break;
                    }
                }
            }
        }
    }

    /// 局が終了していたら次局確認待ちに入る
    fn check_round_end(&mut self) {
        let Some(driver) = self.driver.as_ref() else {
            return;
        };
        if self.awaiting_ready || self.game_over_sent {
            return;
        }
        let round_over = driver
            .table()
            .current_round()
            .map(|r| r.is_over())
            .unwrap_or(false);
        if round_over {
            self.awaiting_ready = true;
            self.ready = [false; 4];
            self.ready_deadline = Some(Instant::now() + self.config.ready_timeout);
        }
    }

    /// 接続中の人間全員が次局進行を確認したか
    fn all_connected_humans_ready(&self) -> bool {
        (0..4)
            .filter(|&s| self.seats[s].as_ref().is_some_and(|seat| seat.tx.is_some()))
            .all(|s| self.ready[s])
    }

    /// 次の局へ進める（ゲーム終了なら GameOver を配信する）
    async fn advance_round(&mut self) {
        self.awaiting_ready = false;
        self.ready_deadline = None;

        let now = self.now_secs();
        let Some(driver) = self.driver.as_mut() else {
            return;
        };
        driver.next_round_at(now);

        if driver.is_game_over() {
            let final_scores = driver.table().scores;
            self.broadcast(ServerMessage::GameOver { final_scores })
                .await;
            self.game_over_sent = true;
            self.action_deadline = None;
            // 全員が切断したら閉じる。念のため期限も設定する
            self.close_deadline = Some(Instant::now() + self.config.abandoned_timeout);
            tracing::info!(code = self.code, "game over");
        } else {
            self.progress_game().await;
        }
    }

    /// 退出または切断を処理する
    async fn handle_departure(&mut self, seat: usize) {
        // 開始前: 座席を空ける。ホストが抜けたらルームを閉じる
        if !self.game_started() {
            self.seats[seat] = None;
            tracing::info!(code = self.code, seat, "player left");
            if seat == HOST_SEAT {
                self.broadcast_error(ErrorCode::NotInRoom, "room closed by host")
                    .await;
                self.closing = true;
                return;
            }
            if self.seats.iter().all(|s| s.is_none()) {
                self.closing = true;
                return;
            }
            self.broadcast_room_state().await;
            return;
        }

        // ゲーム終了後: 座席を空け、全員いなくなったら閉じる
        if self.game_over_sent {
            self.seats[seat] = None;
            if self.seats.iter().all(|s| s.is_none()) {
                self.closing = true;
            }
            return;
        }

        // 対局中: 座席は保持したまま切断扱いにし、CPUが代打ちする
        if let Some(s) = self.seats[seat].as_mut() {
            s.tx = None;
        }
        tracing::info!(
            code = self.code,
            seat,
            "player disconnected; CPU takes over"
        );
        let now = self.now_secs();
        if let Some(driver) = self.driver.as_mut() {
            driver.set_cpu_controlled(seat, true);
            // 切断した座席の入力待ちで止まっていたら既定アクションで進める
            driver.force_default_action_at(seat, now);
        }
        // 残りの接続者へ切断を通知する
        self.broadcast(ServerMessage::PlayerConnectionChanged {
            seat,
            connected: false,
        })
        .await;
        // 確認待ち中の切断はその座席の確認を不要にする
        if self.awaiting_ready && self.all_connected_humans_ready() {
            self.advance_round().await;
        } else {
            self.progress_game().await;
        }

        if !self.any_connected_human() {
            self.close_deadline = Some(Instant::now() + self.config.abandoned_timeout);
        }
    }

    /// 接続中の人間がいるか
    fn any_connected_human(&self) -> bool {
        self.seats
            .iter()
            .any(|s| s.as_ref().is_some_and(|seat| seat.tx.is_some()))
    }

    /// 各座席の公開情報を組み立てる
    fn seats_info(&self) -> [SeatInfo; 4] {
        std::array::from_fn(|s| match &self.seats[s] {
            Some(seat) => SeatInfo::Human {
                name: seat.name.clone(),
                connected: seat.tx.is_some(),
            },
            None => {
                if self.game_started() {
                    SeatInfo::Cpu
                } else {
                    SeatInfo::Empty
                }
            }
        })
    }

    /// 全員に RoomState を送る（your_seat は受信者ごとに変わる）
    async fn broadcast_room_state(&self) {
        let seats_info = self.seats_info();
        for seat in 0..4 {
            self.send_room_state_with(seat, &seats_info).await;
        }
    }

    /// 特定の座席へ RoomState を送る
    async fn send_room_state_to(&self, seat: usize) {
        let seats_info = self.seats_info();
        self.send_room_state_with(seat, &seats_info).await;
    }

    /// 組み立て済みの座席情報を使って特定の座席へ RoomState を送る
    async fn send_room_state_with(&self, seat: usize, seats_info: &[SeatInfo; 4]) {
        let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.clone()) else {
            return;
        };
        let msg = ServerMessage::RoomState {
            code: self.code.clone(),
            seats: seats_info.clone(),
            host_seat: HOST_SEAT,
            your_seat: seat,
        };
        let _ = tx.send(msg).await;
    }

    /// 接続中の全員にメッセージを送る
    async fn broadcast(&self, msg: ServerMessage) {
        for seat in self.seats.iter().flatten() {
            if let Some(tx) = &seat.tx {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }

    /// 接続中の全員にエラーを送る
    async fn broadcast_error(&self, code: ErrorCode, message: &str) {
        self.broadcast(ServerMessage::Error {
            code,
            message: message.to_string(),
        })
        .await;
    }

    /// 特定の座席にエラーを送る
    async fn send_error(&self, seat: usize, code: ErrorCode, message: &str) {
        if let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.clone()) {
            let _ = tx
                .send(ServerMessage::Error {
                    code,
                    message: message.to_string(),
                })
                .await;
        }
    }
}
