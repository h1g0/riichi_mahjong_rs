//! ルームアクター
//!
//! 1ルーム = 1 tokio タスク。ルームが `GameDriver`（卓 + CPU）を所有し、
//! 接続タスクからの `RoomMsg` を mpsc で逐次処理する。
//! 同期的な卓の操作が await をまたがないため、ゲーム状態のロックは不要。

use std::time::Duration;

use mahjong_server::cpu::personalities::default_cpu_configs;
use mahjong_server::driver::GameDriver;
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
}

impl Default for RoomConfig {
    fn default() -> Self {
        RoomConfig {
            ready_timeout: Duration::from_secs(60),
            lobby_timeout: Duration::from_secs(30 * 60),
            abandoned_timeout: Duration::from_secs(5 * 60),
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
        /// 割り当てた座席（またはエラー）の返信先
        reply: oneshot::Sender<Result<usize, ErrorCode>>,
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
    },
}

/// 着席中のプレイヤー
struct Seat {
    #[allow(dead_code)] // 再接続（フェーズ5）で照合に使う
    token: String,
    name: String,
    /// 接続への送信チャネル（None = 切断中）
    tx: Option<mpsc::Sender<ServerMessage>>,
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
    /// ルームを閉じるフラグ
    closing: bool,
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
        closing: false,
    };

    loop {
        let ready_at = deadline_or_far(room.ready_deadline);
        let close_at = deadline_or_far(room.close_deadline);

        tokio::select! {
            msg = rx.recv() => match msg {
                Some(msg) => room.handle_msg(msg).await,
                None => break,
            },
            _ = tokio::time::sleep_until(ready_at), if room.ready_deadline.is_some() => {
                tracing::debug!(code = room.code, "ready timeout; auto-advancing round");
                room.advance_round().await;
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
            } => {
                let result = self.try_join(name, token, tx);
                let _ = reply.send(result);
                if result.is_ok() {
                    self.broadcast_room_state().await;
                }
            }
            RoomMsg::FromSeat { seat, msg } => self.handle_client_message(seat, msg).await,
            // 退出と切断はフェーズ2では同じ扱い（フェーズ5で再接続対応時に分岐する）
            RoomMsg::Leave { seat } | RoomMsg::Disconnected { seat } => {
                self.handle_departure(seat).await
            }
        }
    }

    /// 空席を探して着席させる
    fn try_join(
        &mut self,
        name: String,
        token: String,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<usize, ErrorCode> {
        if self.game_started() {
            // 再接続（トークン照合）はフェーズ5で対応する
            return Err(ErrorCode::GameInProgress);
        }
        let seat = self
            .seats
            .iter()
            .position(|s| s.is_none())
            .ok_or(ErrorCode::RoomFull)?;
        self.seats[seat] = Some(Seat {
            token,
            name,
            tx: Some(tx),
        });
        tracing::info!(code = self.code, seat, "player joined");
        Ok(seat)
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
                let accepted = self
                    .driver
                    .as_mut()
                    .expect("checked above")
                    .handle_action(seat, action);
                if !accepted {
                    self.send_error(seat, ErrorCode::InvalidAction, "action rejected")
                        .await;
                }
                self.progress_game().await;
            }
            ClientMessage::ReadyNextRound => {
                if !self.awaiting_ready {
                    self.send_error(seat, ErrorCode::InvalidAction, "not awaiting next round")
                        .await;
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
        driver.start_game();
        self.driver = Some(driver);
        // 開始したのでロビーの生存期限は解除
        self.close_deadline = None;

        tracing::info!(code = self.code, "game started");
        self.broadcast_room_state().await;
        self.progress_game().await;
    }

    /// 入力待ちまでゲームを進め、イベントを配信し、局終了を確認する
    async fn progress_game(&mut self) {
        if let Some(driver) = self.driver.as_mut() {
            driver.run_until_blocked();
        }
        self.flush_events().await;
        self.check_round_end();
    }

    /// 各座席のイベントを接続へ送信する
    async fn flush_events(&mut self) {
        let Some(driver) = self.driver.as_mut() else {
            return;
        };
        for seat in 0..4 {
            // 切断中・空席のバッファも溜め込まないよう必ず drain する
            let events = driver.drain_events(seat);
            let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.clone()) else {
                continue;
            };
            for event in events {
                if tx.send(ServerMessage::Event(event)).await.is_err() {
                    // 送信失敗は切断として扱う（Disconnected が後続で届く）
                    break;
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

        let Some(driver) = self.driver.as_mut() else {
            return;
        };
        driver.next_round();

        if driver.is_game_over() {
            let final_scores = driver.table().scores;
            self.broadcast(ServerMessage::GameOver { final_scores })
                .await;
            self.game_over_sent = true;
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
        if let Some(driver) = self.driver.as_mut() {
            driver.set_cpu_controlled(seat, true);
            // 切断した座席の入力待ちで止まっていたら既定アクションで進める
            driver.force_default_action(seat);
        }
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

    /// 全員に RoomState を送る（your_seat は受信者ごとに変わる）
    async fn broadcast_room_state(&self) {
        let seats_info: [SeatInfo; 4] = std::array::from_fn(|s| match &self.seats[s] {
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
        });

        for seat in 0..4 {
            let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.clone()) else {
                continue;
            };
            let msg = ServerMessage::RoomState {
                code: self.code.clone(),
                seats: seats_info.clone(),
                host_seat: HOST_SEAT,
                your_seat: seat,
            };
            let _ = tx.send(msg).await;
        }
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
