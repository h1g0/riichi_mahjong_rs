//! The room actor.
//!
//! One room = one tokio task. The room owns the `GameDriver` (table +
//! CPUs) and processes `RoomMsg`s from the connection tasks over an
//! mpsc channel; the synchronous table operations never span an await,
//! so game state needs no lock.
//!
//! Sends to clients use `try_send` and never block: a connection that
//! cannot keep up (full buffer) counts as disconnected, so one laggard
//! cannot stall the whole room.

use std::time::Duration;

use mahjong_server::cpu::client::CpuConfig;
use mahjong_server::cpu::personalities::default_cpu_configs;
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::ServerEvent;
use mahjong_server::protocol::net::{ClientMessage, CpuSpec, ErrorCode, SeatInfo, ServerMessage};
use mahjong_server::table::GameSettings;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::lobby::Lobby;

/// Room timing configuration: `Default` in production, shortened
/// in tests.
#[derive(Debug, Clone, Copy)]
pub struct RoomConfig {
    /// Grace before auto-advancing past the result screen
    pub ready_timeout: Duration,
    /// Lifetime of a room before the game starts
    pub lobby_timeout: Duration,
    /// Grace before discarding a room after everyone disconnects mid-game
    pub abandoned_timeout: Duration,
    /// Per-turn time limit (None = unlimited); on expiry the server
    /// performs the default action (tsumogiri / pass).
    pub action_timeout: Option<Duration>,
    /// Delay between CPU actions (thinking time); 0 runs immediately.
    pub cpu_action_delay: Duration,
    /// Game-tick interval: the granularity for pacing the CPU delay.
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

/// Messages from connection tasks to the room actor.
pub enum RoomMsg {
    /// A join request
    Join {
        /// Display name
        name: String,
        /// Session token
        token: String,
        /// Send channel to this connection
        tx: mpsc::Sender<ServerMessage>,
        /// Reply channel for the assigned seat and connection
        /// generation (or an error)
        reply: oneshot::Sender<Result<(usize, u64), ErrorCode>>,
    },
    /// A client message from a seat
    FromSeat {
        /// Seat index
        seat: usize,
        /// The message
        msg: ClientMessage,
    },
    /// An explicit leave
    Leave {
        /// Seat index
        seat: usize,
    },
    /// A disconnect (socket closed)
    Disconnected {
        /// Seat index
        seat: usize,
        /// Generation of the dropped connection, so a late notice
        /// cannot clobber a reconnect
        conn_gen: u64,
    },
}

/// Join outcome: the seat info returned to the connection task plus
/// room-internal extras.
struct JoinOutcome {
    seat: usize,
    conn_gen: u64,
    /// Whether this was a reconnect to an existing seat
    reconnect: bool,
}

/// A seated player.
struct Seat {
    /// Session token, matched on reconnection
    token: String,
    name: String,
    /// Send channel to the connection; None while disconnected
    tx: Option<mpsc::Sender<ServerMessage>>,
    /// Current connection generation, bumped on every reconnect
    conn_gen: u64,
    /// Event history since the current hand's GameStarted,
    /// for reconnect resync
    history: Vec<ServerEvent>,
}

/// The host's seat index (the first joiner).
const HOST_SEAT: usize = 0;

/// Room state.
struct Room {
    code: String,
    settings: GameSettings,
    config: RoomConfig,
    seats: [Option<Seat>; 4],
    driver: Option<GameDriver>,
    /// Whether the room awaits result-screen confirmations
    awaiting_ready: bool,
    /// Per-seat next-hand confirmations
    ready: [bool; 4],
    /// Whether GameOver has been sent
    game_over_sent: bool,
    /// Deadline for auto-advancing to the next hand
    ready_deadline: Option<Instant>,
    /// Deadline for discarding the room
    close_deadline: Option<Instant>,
    /// Turn-timer deadline
    action_deadline: Option<Instant>,
    /// Seats the current timer covers, for detecting changed waits
    deadline_seats: Vec<usize>,
    /// Time base for game progress (game start), used to pace
    /// the CPU delay
    game_clock: Option<Instant>,
    /// Set to close the room
    closing: bool,
    /// Next connection generation to hand out
    next_conn_gen: u64,
    /// Seats whose sends failed (full buffer / closed) and need
    /// disconnect handling
    pending_departures: Vec<usize>,
    /// CPU configs (empty seats and shadows), set by the host at
    /// game start
    cpu_configs: [CpuConfig; 3],
}

/// The CPU config for a seat.
///
/// Seats 1-3 (the host's right, across, left) map to `configs[0..3]` in
/// order; seat 0 (the host) reuses `configs[0]` for its shadow CPU.
fn config_for_seat(configs: &[CpuConfig; 3], seat: usize) -> CpuConfig {
    let idx = seat.saturating_sub(1).min(2);
    configs[idx].clone()
}

/// The room actor's main loop.
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
        deadline_seats: Vec::new(),
        game_clock: None,
        closing: false,
        next_conn_gen: 0,
        pending_departures: Vec::new(),
        cpu_configs: default_cpu_configs(),
    };

    // The tick that paces the game against the CPU delay.
    let mut game_tick = tokio::time::interval(config.tick_interval);
    game_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        let ready_at = deadline_or_far(room.ready_deadline);
        let close_at = deadline_or_far(room.close_deadline);
        let action_at = deadline_or_far(room.action_deadline);

        tokio::select! {
            msg = rx.recv() => match msg {
                Some(msg) => room.handle_msg(msg),
                None => break,
            },
            _ = game_tick.tick(), if room.needs_game_tick() => {
                room.game_tick();
            }
            _ = tokio::time::sleep_until(ready_at), if room.ready_deadline.is_some() => {
                tracing::debug!(code = room.code, "ready timeout; auto-advancing round");
                room.advance_round();
            }
            _ = tokio::time::sleep_until(action_at), if room.action_deadline.is_some() => {
                tracing::debug!(code = room.code, "action timeout; forcing default action");
                room.on_action_timeout();
            }
            _ = tokio::time::sleep_until(close_at), if room.close_deadline.is_some() => {
                tracing::info!(code = room.code, "room expired");
                room.closing = true;
            }
        }

        // Handle every send-failure disconnect in one sweep.
        room.process_departures();

        if room.closing {
            break;
        }
    }

    lobby.remove(&code);
}

/// Turns None into a far-future instant for select!; the `if` guard
/// disables the branch, so the value is never actually used.
fn deadline_or_far(deadline: Option<Instant>) -> Instant {
    deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(365 * 24 * 3600))
}

impl Room {
    /// Whether the game has started.
    fn game_started(&self) -> bool {
        self.driver.is_some()
    }

    /// Player count (4 or 3; the three-player seat 3 stays empty).
    fn player_count(&self) -> usize {
        self.settings.rules.player_count()
    }

    fn handle_msg(&mut self, msg: RoomMsg) {
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
                        self.handle_reconnect(outcome.seat);
                    } else {
                        self.broadcast_room_state();
                    }
                }
                Err(code) => {
                    let _ = reply.send(Err(code));
                }
            },
            RoomMsg::FromSeat { seat, msg } => self.handle_client_message(seat, msg),
            RoomMsg::Leave { seat } => self.handle_departure(seat),
            RoomMsg::Disconnected { seat, conn_gen } => {
                // Ignore a stale disconnect from an old connection;
                // a reconnect has bumped the generation.
                if self.seats[seat]
                    .as_ref()
                    .is_some_and(|s| s.conn_gen == conn_gen)
                {
                    self.handle_departure(seat);
                }
            }
        }
    }

    /// Hands out the next connection generation.
    fn alloc_conn_gen(&mut self) -> u64 {
        let generation = self.next_conn_gen;
        self.next_conn_gen += 1;
        generation
    }

    /// Handles a join or reconnect: mid-game a matching token reclaims
    /// its disconnected seat; before the game a new joiner takes an
    /// empty seat.
    fn try_join(
        &mut self,
        name: String,
        token: String,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<JoinOutcome, ErrorCode> {
        if self.game_started() {
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

        // New join: an empty seat (three-player rooms use 0-2 only).
        let seat = self.seats[..self.player_count()]
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

    /// Stops the CPU substitution and resyncs a reconnected seat.
    fn handle_reconnect(&mut self, seat: usize) {
        if let Some(driver) = self.driver.as_mut() {
            driver.set_cpu_controlled(seat, false);
        }
        self.broadcast(ServerMessage::PlayerConnectionChanged {
            seat,
            connected: true,
        });
        // Send the fresh RoomState plus the current hand's replay.
        self.send_room_state_to(seat);
        let history = self.seats[seat]
            .as_ref()
            .map(|s| s.history.clone())
            .unwrap_or_default();
        self.send_to_seat(seat, ServerMessage::Resync { events: history });
        // The awaited seats may have changed; re-arm the timer.
        self.refresh_action_deadline();
    }

    fn handle_client_message(&mut self, seat: usize, msg: ClientMessage) {
        match msg {
            ClientMessage::SetCpuConfigs { cpu_configs } => {
                self.handle_set_cpu_configs(seat, cpu_configs)
            }
            ClientMessage::StartGame { cpu_configs } => self.handle_start_game(seat, cpu_configs),
            ClientMessage::Action(action) => {
                if !self.game_started() || self.awaiting_ready {
                    self.send_error(seat, ErrorCode::InvalidAction, "no action expected now");
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
                    );
                }
                self.progress_game();
            }
            ClientMessage::ReadyNextRound => {
                if !self.awaiting_ready {
                    // A late confirmation racing the auto-advance timer is
                    // harmless; ignore it silently.
                    return;
                }
                self.ready[seat] = true;
                if self.all_connected_humans_ready() {
                    self.advance_round();
                }
            }
            // Hello/CreateRoom/JoinRoom/LeaveRoom were handled by the
            // connection task.
            _ => {
                self.send_error(seat, ErrorCode::BadMessage, "unexpected message");
            }
        }
    }

    /// Stores the host's CPU configs and shares them with every lobby.
    fn handle_set_cpu_configs(&mut self, seat: usize, cpu_configs: [CpuSpec; 3]) {
        if seat != HOST_SEAT {
            self.send_error(seat, ErrorCode::NotHost, "only the host can configure CPUs");
            return;
        }
        if self.game_started() {
            self.send_error(seat, ErrorCode::GameInProgress, "game already started");
            return;
        }
        self.cpu_configs = cpu_configs.map(|spec| spec.to_config());
        self.broadcast_room_state();
    }

    fn handle_start_game(&mut self, seat: usize, cpu_configs: Option<[CpuSpec; 3]>) {
        if seat != HOST_SEAT {
            self.send_error(seat, ErrorCode::NotHost, "only the host can start");
            return;
        }
        if self.game_started() {
            self.send_error(seat, ErrorCode::GameInProgress, "game already started");
            return;
        }

        if let Some(specs) = cpu_configs {
            self.cpu_configs = specs.map(|spec| spec.to_config());
        }

        // Shuffle the participating CPUs' seats;
        // GameDriver::start_game randomizes the dealer.
        let cpu_count = self.player_count() - 1;
        mahjong_server::cpu::client::shuffle_cpu_configs(&mut self.cpu_configs[..cpu_count]);

        let mut driver = GameDriver::new(self.settings.clone());
        for s in 0..self.player_count() {
            let config = config_for_seat(&self.cpu_configs, s);
            if self.seats[s].is_some() {
                // Human seats get a resident shadow CPU for instant
                // substitution on disconnect.
                driver.set_shadow_cpu(s, config);
            } else {
                driver.set_cpu(s, config);
            }
        }
        driver.set_cpu_action_delay(self.config.cpu_action_delay.as_secs_f64());
        driver.start_game();
        self.driver = Some(driver);
        // Start the clock for the CPU delay; now_secs() feeds it
        // from here on.
        self.game_clock = Some(Instant::now());
        // The game started, so the lobby lifetime no longer applies.
        self.close_deadline = None;

        tracing::info!(code = self.code, "game started");
        self.broadcast_room_state();
        self.progress_game();
    }

    /// Delivers the last action's results, checks for hand end, and
    /// re-arms the deadlines.
    ///
    /// CPU progress is paced by [`game_tick`](Self::game_tick), so this
    /// never races ahead to the next draw - it only flushes events and
    /// updates state.
    fn progress_game(&mut self) {
        self.flush_events();
        self.check_round_end();
        self.refresh_action_deadline();
    }

    /// Advances the game one paced tick: applies due CPU actions and
    /// draws in the draw phase. Called only while `needs_game_tick`
    /// is true.
    fn game_tick(&mut self) {
        let now = self.now_secs();
        if let Some(driver) = self.driver.as_mut() {
            driver.tick_at(now);
        }
        self.flush_events();
        self.check_round_end();
        self.refresh_action_deadline();
    }

    /// Whether a tick is needed for CPU progress (draws, pending
    /// delayed actions).
    fn needs_game_tick(&self) -> bool {
        if self.awaiting_ready || self.game_over_sent {
            return false;
        }
        self.driver.as_ref().is_some_and(|d| d.needs_tick())
    }

    /// Seconds since the game time base, fed to the CPU-delay pacing.
    fn now_secs(&self) -> f64 {
        self.game_clock
            .map(|c| c.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Whether the seat is a connected human.
    fn is_connected_human(&self, seat: usize) -> bool {
        self.seats[seat].as_ref().is_some_and(|s| s.tx.is_some())
    }

    /// Re-arms the turn timer.
    ///
    /// While a connected human is being waited on, sets the deadline and
    /// notifies the seat of the remaining seconds; the deadline persists
    /// while the same wait continues (so another player's invalid action
    /// cannot extend it). Otherwise (CPU progress, confirmations, hand
    /// over) the timer is cleared.
    fn refresh_action_deadline(&mut self) {
        let Some(timeout) = self.config.action_timeout else {
            self.clear_action_deadline();
            return;
        };
        if self.awaiting_ready || self.game_over_sent {
            self.clear_action_deadline();
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
            self.clear_action_deadline();
            return;
        }

        if self.action_deadline.is_some() && seats == self.deadline_seats {
            return;
        }

        self.deadline_seats = seats.clone();
        self.action_deadline = Some(Instant::now() + timeout);
        // Round up so even short limits never show 0 seconds.
        let seconds = timeout.as_secs_f64().ceil() as u32;
        for seat in seats {
            self.send_to_seat(seat, ServerMessage::TurnTimer { seconds });
        }
    }

    /// Clears the turn timer.
    fn clear_action_deadline(&mut self) {
        self.action_deadline = None;
        self.deadline_seats.clear();
    }

    /// Turn-timer expiry: performs the default action for the awaited
    /// connected humans.
    fn on_action_timeout(&mut self) {
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
        self.clear_action_deadline();
        self.progress_game();
    }

    /// Records each seat's events into its history and sends them to
    /// connected seats.
    ///
    /// Histories reset on GameStarted and hold only the current hand;
    /// disconnected seats keep recording for the reconnect resync.
    fn flush_events(&mut self) {
        if self.driver.is_none() {
            return;
        }
        // Take every seat's events first (releasing the driver borrow);
        // drain_all_events_at paces the CPU delay while processing.
        //
        // Draining per seat was broken: pumping a later seat could
        // generate events (e.g. CallAvailable after a call resolved) into
        // an already-drained earlier buffer, where they sat undelivered
        // until the next pass. Draining all seats at once guarantees
        // delivery in the same flush regardless of generation order.
        let now = self.now_secs();
        let per_seat: [Vec<ServerEvent>; 4] = {
            let driver = self.driver.as_mut().expect("checked above");
            driver.drain_all_events_at(now)
        };

        for (seat, events) in per_seat.into_iter().enumerate() {
            for event in events {
                {
                    let Some(s) = self.seats[seat].as_mut() else {
                        break;
                    };
                    if matches!(event, ServerEvent::GameStarted { .. }) {
                        s.history.clear();
                    }
                    s.history.push(event.clone());
                }
                self.send_to_seat(seat, ServerMessage::Event(event));
            }
        }
    }

    /// Enters the next-hand confirmation wait once the hand ends.
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

    /// Whether every connected human has confirmed the next hand.
    fn all_connected_humans_ready(&self) -> bool {
        (0..4)
            .filter(|&s| self.seats[s].as_ref().is_some_and(|seat| seat.tx.is_some()))
            .all(|s| self.ready[s])
    }

    /// Advances to the next hand, or broadcasts GameOver at game end.
    fn advance_round(&mut self) {
        self.awaiting_ready = false;
        self.ready_deadline = None;

        let now = self.now_secs();
        let Some(driver) = self.driver.as_mut() else {
            return;
        };
        driver.next_round_at(now);

        if driver.is_game_over() {
            let final_scores = driver.table().scores;
            self.broadcast(ServerMessage::GameOver { final_scores });
            self.game_over_sent = true;
            self.clear_action_deadline();
            // Close once everyone is gone; set the deadline as a backstop.
            self.close_deadline = Some(Instant::now() + self.config.abandoned_timeout);
            tracing::info!(code = self.code, "game over");
        } else {
            self.progress_game();
        }
    }

    /// Sweeps the seats marked disconnected by failed sends.
    ///
    /// Sends made during the sweep (disconnect notices) can fail too, so
    /// it loops until empty; each seat leaves the send set once
    /// disconnected, so termination is guaranteed.
    fn process_departures(&mut self) {
        while let Some(seat) = self.pending_departures.pop() {
            tracing::info!(
                code = self.code,
                seat,
                "send failed; treating as disconnect"
            );
            self.handle_departure(seat);
        }
    }

    /// Handles a leave or disconnect.
    fn handle_departure(&mut self, seat: usize) {
        // Pre-game: vacate the seat; the host leaving closes the room.
        if !self.game_started() {
            self.seats[seat] = None;
            tracing::info!(code = self.code, seat, "player left");
            if seat == HOST_SEAT {
                self.broadcast_error(ErrorCode::NotInRoom, "room closed by host");
                self.closing = true;
                return;
            }
            if self.seats.iter().all(|s| s.is_none()) {
                self.closing = true;
                return;
            }
            self.broadcast_room_state();
            return;
        }

        // Post-game: vacate; close once empty.
        if self.game_over_sent {
            self.seats[seat] = None;
            if self.seats.iter().all(|s| s.is_none()) {
                self.closing = true;
            }
            return;
        }

        // Mid-game: keep the seat, mark it disconnected, let the CPU
        // substitute.
        match self.seats[seat].as_mut() {
            // Skip double handling: both a failed send and the socket
            // close can get here.
            Some(s) if s.tx.is_some() => s.tx = None,
            _ => return,
        }
        tracing::info!(
            code = self.code,
            seat,
            "player disconnected; CPU takes over"
        );
        let now = self.now_secs();
        if let Some(driver) = self.driver.as_mut() {
            driver.set_cpu_controlled(seat, true);
            // If the game was waiting on this seat, the default action
            // unblocks it.
            driver.force_default_action_at(seat, now);
        }
        self.broadcast(ServerMessage::PlayerConnectionChanged {
            seat,
            connected: false,
        });
        // A disconnect during confirmations waives that seat's
        // confirmation.
        if self.awaiting_ready && self.all_connected_humans_ready() {
            self.advance_round();
        } else {
            self.progress_game();
        }

        if !self.any_connected_human() {
            self.close_deadline = Some(Instant::now() + self.config.abandoned_timeout);
        }
    }

    /// Whether any human is connected.
    fn any_connected_human(&self) -> bool {
        self.seats
            .iter()
            .any(|s| s.as_ref().is_some_and(|seat| seat.tx.is_some()))
    }

    /// Builds each seat's public info.
    fn seats_info(&self) -> [SeatInfo; 4] {
        std::array::from_fn(|s| match &self.seats[s] {
            Some(seat) => SeatInfo::Human {
                name: seat.name.clone(),
                connected: seat.tx.is_some(),
            },
            None => {
                if self.game_started() && s < self.player_count() {
                    // Same level/personality rule as the game-start
                    // assignment.
                    let config = config_for_seat(&self.cpu_configs, s);
                    SeatInfo::Cpu {
                        level: config.level,
                        personality: config.personality,
                    }
                } else {
                    SeatInfo::Empty
                }
            }
        })
    }

    /// Sends RoomState to everyone; your_seat varies per recipient.
    fn broadcast_room_state(&mut self) {
        let seats_info = self.seats_info();
        for seat in 0..4 {
            self.send_room_state_with(seat, &seats_info);
        }
    }

    /// Sends RoomState to one seat.
    fn send_room_state_to(&mut self, seat: usize) {
        let seats_info = self.seats_info();
        self.send_room_state_with(seat, &seats_info);
    }

    /// Sends RoomState to one seat using pre-built seat info.
    fn send_room_state_with(&mut self, seat: usize, seats_info: &[SeatInfo; 4]) {
        let msg = ServerMessage::RoomState {
            code: self.code.clone(),
            seats: seats_info.clone(),
            host_seat: HOST_SEAT,
            your_seat: seat,
            rules: self.settings.rules.clone(),
            length: self.settings.length,
            cpu_configs: Some(std::array::from_fn(|i| {
                CpuSpec::from_config(&self.cpu_configs[i])
            })),
        };
        self.send_to_seat(seat, msg);
    }

    /// Broadcasts a message to every connected seat.
    fn broadcast(&mut self, msg: ServerMessage) {
        for seat in 0..4 {
            self.send_to_seat(seat, msg.clone());
        }
    }

    /// Broadcasts an error to every connected seat.
    fn broadcast_error(&mut self, code: ErrorCode, message: &str) {
        self.broadcast(ServerMessage::Error {
            code,
            message: message.to_string(),
        });
    }

    /// Sends an error to one seat.
    fn send_error(&mut self, seat: usize, code: ErrorCode, message: &str) {
        self.send_to_seat(
            seat,
            ServerMessage::Error {
                code,
                message: message.to_string(),
            },
        );
    }

    /// Sends to one seat without blocking; disconnected seats are a
    /// no-op.
    ///
    /// A full buffer (the receiver stalled) or a closed connection marks
    /// the seat for disconnect handling, performed in bulk by
    /// [`process_departures`](Self::process_departures).
    fn send_to_seat(&mut self, seat: usize, msg: ServerMessage) {
        let Some(tx) = self.seats[seat].as_ref().and_then(|s| s.tx.as_ref()) else {
            return;
        };
        if tx.try_send(msg).is_err() && !self.pending_departures.contains(&seat) {
            self.pending_departures.push(seat);
        }
    }
}
