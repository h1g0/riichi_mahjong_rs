//! Game driver.
//!
//! Synchronous glue that owns a `Table` plus CPU clients and runs the
//! event pump. It performs no I/O, so both local play (embedded in the
//! client) and online play (a network-server room) reuse it.
//!
//! CPUs speak the same protocol as humans (ServerEvent / ClientAction);
//! events for seats not driven by a CPU accumulate in per-seat buffers.
//!
//! To simulate CPU "thinking time", enable a delay via
//! [`set_cpu_action_delay`](GameDriver::set_cpu_action_delay) and pass the
//! current time (seconds) to the `*_at` methods. Time is injected so this
//! crate stays independent of any clock implementation (macroquad etc.).
//! Without a delay (e.g. on the network server) everything runs
//! immediately.

use std::collections::VecDeque;

use crate::cpu::client::{CpuClient, CpuConfig};
use crate::protocol::{ClientAction, ServerEvent};
use crate::round::TurnPhase;
use crate::table::{GameSettings, Table};

/// CPU actions awaiting their "thinking" delay.
///
/// Responses produced by the same event pass are applied together, since
/// call resolution needs them all at once.
struct PendingCpuActionBatch {
    actions: Vec<(usize, ClientAction)>,
    ready_at: f64,
}

/// A CPU client bound to a seat.
struct CpuSeat {
    client: CpuClient,
    /// Whether the CPU drives this seat.
    ///
    /// When false this is a "shadow CPU": it receives events to keep its
    /// internal state current but emits no actions (a human drives the
    /// seat). Flipping to true on disconnect lets the CPU take over
    /// instantly.
    controlled: bool,
    /// Whether this seat's events are always buffered.
    ///
    /// True for shadow CPUs (human seats): events keep buffering even
    /// while the CPU substitutes (controlled = true), so the room can
    /// record a history for reconnection. False for pure CPU seats.
    mirror: bool,
}

/// Owns the table and CPU clients and advances the game.
pub struct GameDriver {
    table: Table,
    /// Per-seat CPU clients; None means a purely human seat
    cpus: [Option<CpuSeat>; 4],
    /// Event buffers for seats not driven by a CPU
    event_buffers: [Vec<ServerEvent>; 4],
    /// Delay between CPU actions in seconds; None applies immediately
    action_delay: Option<f64>,
    /// Pending CPU action batches, FIFO per event pass
    pending_cpu_batches: VecDeque<PendingCpuActionBatch>,
}

impl GameDriver {
    /// Creates a driver with every seat human.
    pub fn new(settings: GameSettings) -> Self {
        GameDriver {
            table: Table::new(settings),
            cpus: [None, None, None, None],
            event_buffers: [const { Vec::new() }; 4],
            action_delay: None,
            pending_cpu_batches: VecDeque::new(),
        }
    }

    /// Assigns a CPU client that drives the seat.
    pub fn set_cpu(&mut self, seat: usize, config: CpuConfig) {
        if seat < 4 {
            self.cpus[seat] = Some(CpuSeat {
                client: CpuClient::new_with_rules(config, &self.table.settings.rules),
                controlled: true,
                mirror: false,
            });
        }
    }

    /// Assigns a shadow CPU to the seat.
    ///
    /// It receives events to track state but emits no actions. Assign one
    /// to a human seat so [`set_cpu_controlled`](Self::set_cpu_controlled)
    /// can substitute instantly on disconnect. Shadow seats keep buffering
    /// events during substitution, so [`drain_events`](Self::drain_events)
    /// still yields a reconnection history.
    pub fn set_shadow_cpu(&mut self, seat: usize, config: CpuConfig) {
        if seat < 4 {
            self.cpus[seat] = Some(CpuSeat {
                client: CpuClient::new_with_rules(config, &self.table.settings.rules),
                controlled: false,
                mirror: true,
            });
        }
    }

    /// Toggles CPU control of a seat.
    ///
    /// Returns false when the seat has no CPU client to toggle.
    pub fn set_cpu_controlled(&mut self, seat: usize, controlled: bool) -> bool {
        match self.cpus.get_mut(seat) {
            Some(Some(cpu)) => {
                cpu.controlled = controlled;
                if !controlled {
                    // Releasing control drops unapplied CPU actions so a
                    // reconnecting human's play is not hijacked by the
                    // substitute.
                    for batch in &mut self.pending_cpu_batches {
                        batch.actions.retain(|(s, _)| *s != seat);
                    }
                    self.pending_cpu_batches.retain(|b| !b.actions.is_empty());
                }
                true
            }
            _ => false,
        }
    }

    /// Whether a CPU currently drives the seat.
    pub fn is_cpu_controlled(&self, seat: usize) -> bool {
        matches!(self.cpus.get(seat), Some(Some(cpu)) if cpu.controlled)
    }

    /// Enables a delay (seconds) before CPU actions apply.
    ///
    /// Once enabled, drive the game through `tick_at` / `handle_action_at` /
    /// `drain_events_at` / `next_round_at`, passing the current time.
    pub fn set_cpu_action_delay(&mut self, seconds: f64) {
        self.action_delay = Some(seconds);
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    /// Starts the game: picks a random starting dealer and deals the
    /// first hand.
    pub fn start_game(&mut self) {
        self.pending_cpu_batches.clear();
        self.table.randomize_dealer();
        self.table.start_round();
        self.pump(None);
    }

    /// Starts the game with a seeded wall, for tests and reproduction.
    ///
    /// The dealer stays at seat 0 to keep runs reproducible.
    pub fn start_game_with_seed(&mut self, seed: u64) {
        self.pending_cpu_batches.clear();
        self.table.start_round_with_seed(seed);
        self.pump(None);
    }

    /// Drains a seat's buffered events.
    ///
    /// Yields events for human and shadow-CPU seats; shadow seats keep
    /// buffering during substitution (reconnection history). Pure CPU
    /// seats are always empty.
    pub fn drain_events(&mut self, seat: usize) -> Vec<ServerEvent> {
        self.drain_events_impl(seat, None)
    }

    /// Time-passing variant of `drain_events`, for when the CPU delay
    /// is enabled.
    pub fn drain_events_at(&mut self, seat: usize, now: f64) -> Vec<ServerEvent> {
        self.drain_events_impl(seat, Some(now))
    }

    fn drain_events_impl(&mut self, seat: usize, now: Option<f64>) -> Vec<ServerEvent> {
        self.pump(now);

        match self.event_buffers.get_mut(seat) {
            Some(buffer) => std::mem::take(buffer),
            None => Vec::new(),
        }
    }

    /// Drains every seat's events in one flush (for the online server).
    ///
    /// Calling [`drain_events_at`](Self::drain_events_at) per seat is
    /// broken: pumping while processing a later seat can generate new
    /// events (e.g. `CallAvailable` after a call resolves) that land in an
    /// already-drained earlier seat's buffer, and sit undelivered until
    /// the next pass. That left players without their `CallAvailable`
    /// while a response was expected, so the game appeared stuck until
    /// the action timeout force-discarded. Pumping once per seat and then
    /// draining all buffers together guarantees delivery in the same
    /// flush regardless of generation order (the pump count matches the
    /// old per-seat calls, keeping the CPU delay pacing unchanged).
    pub fn drain_all_events_at(&mut self, now: f64) -> [Vec<ServerEvent>; 4] {
        for _ in 0..self.event_buffers.len() {
            self.pump(Some(now));
        }
        std::array::from_fn(|seat| std::mem::take(&mut self.event_buffers[seat]))
    }

    /// Handles a seat's action; returns false for invalid actions
    /// (wrong turn, wrong phase, ...).
    pub fn handle_action(&mut self, seat: usize, action: ClientAction) -> bool {
        self.handle_action_impl(seat, action, None)
    }

    /// Time-passing variant of `handle_action`.
    pub fn handle_action_at(&mut self, seat: usize, action: ClientAction, now: f64) -> bool {
        self.handle_action_impl(seat, action, Some(now))
    }

    fn handle_action_impl(&mut self, seat: usize, action: ClientAction, now: Option<f64>) -> bool {
        let accepted = self.table.handle_action(seat, action);
        self.pump(now);
        accepted
    }

    /// Advances the game one tick: executes the draw phase, waits for UI
    /// input on a human's turn, and lets event delivery drive CPU turns.
    pub fn tick(&mut self) {
        self.tick_impl(None);
    }

    /// Time-passing variant of `tick`.
    pub fn tick_at(&mut self, now: f64) {
        self.tick_impl(Some(now));
    }

    fn tick_impl(&mut self, now: Option<f64>) {
        // When a CPU acted this tick, hold off the next draw so the
        // action order stays visible on screen.
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
                round.do_draw();
            }
            TurnPhase::WaitForDiscard
            | TurnPhase::WaitForCalls
            | TurnPhase::WaitForNineTerminals
            | TurnPhase::RoundOver => {
                // Waiting for human input, or already handled.
            }
        }

        self.pump(now);
    }

    /// Runs the game until human input is needed or the hand ends.
    ///
    /// Repeats the draw phase while the event pump handles CPU actions.
    /// For the network server, which has no frame loop; runs immediately
    /// regardless of the CPU delay. The finite wall guarantees termination.
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

    /// Performs the seat's default action (tsumogiri / pass / continue)
    /// if it is being waited on.
    ///
    /// Unblocks a game stalled on input right after a CPU takeover or an
    /// action timeout. Returns true when an action was performed.
    pub fn force_default_action(&mut self, seat: usize) -> bool {
        self.force_default_action_impl(seat, None)
    }

    /// Time-passing variant of `force_default_action`.
    pub fn force_default_action_at(&mut self, seat: usize, now: f64) -> bool {
        self.force_default_action_impl(seat, Some(now))
    }

    fn force_default_action_impl(&mut self, seat: usize, now: Option<f64>) -> bool {
        let Some(action) = self.default_action_for(seat) else {
            return false;
        };
        self.handle_action_impl(seat, action, now)
    }

    /// The seat's default action (tsumogiri / pass / decline nine
    /// terminals), or None when the seat is not being waited on.
    fn default_action_for(&self, seat: usize) -> Option<ClientAction> {
        let round = self.table.current_round()?;
        if round.is_over() || seat >= round.player_count {
            return None;
        }
        match round.phase {
            TurnPhase::WaitForDiscard if round.current_player == seat => {
                // Right after a call there is no drawn tile, so tsumogiri
                // (None) would fail; pick a hand tile that the swap-calling
                // rule allows instead.
                let player = &round.players[seat];
                let tile = if player.hand.drawn().is_some() {
                    None
                } else {
                    player
                        .hand
                        .tiles()
                        .iter()
                        .rev()
                        .copied()
                        .find(|t| !player.is_swap_call_forbidden(*t))
                };
                Some(ClientAction::Discard { tile })
            }
            TurnPhase::WaitForCalls => {
                let pending = round
                    .call_state
                    .as_ref()
                    .map(|cs| !cs.responded[seat])
                    .unwrap_or(false);
                pending.then_some(ClientAction::Pass)
            }
            TurnPhase::WaitForNineTerminals if round.current_player == seat => {
                Some(ClientAction::NineTerminals { declare: false })
            }
            _ => None,
        }
    }

    /// Applies a CPU action, falling back to the default action when the
    /// server rejects it.
    ///
    /// A rejected CPU is never re-consulted until a new event arrives, so
    /// leaving the rejection alone would stall the hand forever waiting on
    /// that seat (e.g. a pei declaration on the last draw is rejected
    /// because no replacement tile exists; #296). The fallback keeps the
    /// hand moving even when the CPU's judgement and the server's
    /// validation disagree.
    fn apply_cpu_action(&mut self, seat: usize, action: ClientAction) {
        if self.table.handle_action(seat, action) {
            return;
        }
        if let Some(fallback) = self.default_action_for(seat) {
            self.table.handle_action(seat, fallback);
        }
    }

    /// Whether a tick is needed for CPU progress or a draw
    /// (false while waiting on human input).
    ///
    /// True with pending CPU actions, in the draw phase, or on a
    /// CPU-driven seat's turn. The network server uses this to decide
    /// whether to call `tick_at` while pacing the CPU delay, avoiding
    /// useless ticks during human waits.
    pub fn needs_tick(&self) -> bool {
        if !self.pending_cpu_batches.is_empty() {
            return true;
        }
        let Some(round) = self.table.current_round() else {
            return false;
        };
        if round.is_over() {
            return false;
        }
        match round.phase {
            TurnPhase::Draw => true,
            TurnPhase::WaitForDiscard | TurnPhase::WaitForNineTerminals => {
                self.is_cpu_controlled(round.current_player)
            }
            TurnPhase::WaitForCalls => round
                .call_state
                .as_ref()
                .is_some_and(|cs| (0..4).any(|i| !cs.responded[i] && self.is_cpu_controlled(i))),
            TurnPhase::RoundOver => false,
        }
    }

    /// Whether the current hand is over.
    pub fn is_round_over(&self) -> bool {
        match self.table.current_round() {
            Some(r) => r.is_over(),
            None => true,
        }
    }

    /// Seats whose action is currently awaited.
    ///
    /// The current player during discard / nine-terminals waits, the
    /// unresponded players during call waits, and empty otherwise. Used to
    /// pick the targets of the action timeout.
    pub fn pending_action_seats(&self) -> Vec<usize> {
        let Some(round) = self.table.current_round() else {
            return Vec::new();
        };
        if round.is_over() {
            return Vec::new();
        }
        match round.phase {
            TurnPhase::WaitForDiscard | TurnPhase::WaitForNineTerminals => {
                vec![round.current_player]
            }
            TurnPhase::WaitForCalls => match &round.call_state {
                Some(cs) => (0..4).filter(|&i| !cs.responded[i]).collect(),
                None => Vec::new(),
            },
            TurnPhase::Draw | TurnPhase::RoundOver => Vec::new(),
        }
    }

    /// Starts the next hand.
    pub fn next_round(&mut self) {
        self.next_round_impl(None);
    }

    /// Time-passing variant of `next_round`.
    pub fn next_round_at(&mut self, now: f64) {
        self.next_round_impl(Some(now));
    }

    fn next_round_impl(&mut self, now: Option<f64>) {
        self.pending_cpu_batches.clear();
        self.table.finish_round();
        if !self.table.is_game_over {
            self.table.start_round();
            self.pump(now);
        }
    }

    /// Whether the game is over.
    pub fn is_game_over(&self) -> bool {
        self.table.is_game_over
    }

    /// Runs the event pump; returns true when a CPU applied an action.
    ///
    /// With a delay configured and `now` provided, processes one paced
    /// step; otherwise loops immediately until quiescent.
    fn pump(&mut self, now: Option<f64>) -> bool {
        match (self.action_delay, now) {
            (Some(delay), Some(now)) => self.pump_paced(delay, now),
            _ => {
                self.pump_immediate();
                false
            }
        }
    }

    /// Drains server events, delivering to CPUs and buffering for humans.
    ///
    /// Loops because CPU actions can generate further events.
    fn pump_immediate(&mut self) {
        // Apply any leftover pending batches first (always empty when
        // the delay is unused).
        while let Some(batch) = self.pending_cpu_batches.pop_front() {
            for (seat, action) in batch.actions {
                self.apply_cpu_action(seat, action);
            }
        }

        loop {
            let all_events = self.table.drain_events();
            if all_events.is_empty() {
                break;
            }

            self.buffer_human_events(&all_events);

            let cpu_actions = self.collect_cpu_actions(&all_events);

            if cpu_actions.is_empty() {
                break;
            }

            for (seat, action) in cpu_actions {
                self.apply_cpu_action(seat, action);
            }
        }
    }

    /// Processes one paced step.
    ///
    /// CPU actions are queued instead of applied; at most one batch whose
    /// `ready_at` has passed is applied per tick.
    fn pump_paced(&mut self, delay: f64, now: f64) -> bool {
        let cpu_acted = self.apply_ready_cpu_batch(now);

        let all_events = self.table.drain_events();
        if all_events.is_empty() {
            return cpu_acted;
        }

        self.buffer_human_events(&all_events);

        let cpu_actions = self.collect_cpu_actions(&all_events);
        self.schedule_cpu_actions(cpu_actions, delay, now);

        cpu_acted
    }

    /// Buffers events for human-associated seats: everything except pure
    /// CPU seats ([`set_cpu`](Self::set_cpu)). Shadow seats keep buffering
    /// during substitution.
    fn buffer_human_events(&mut self, events: &[(usize, ServerEvent)]) {
        for (seat, event) in events {
            if self.should_buffer(*seat) {
                self.event_buffers[*seat].push(event.clone());
            }
        }
    }

    fn should_buffer(&self, seat: usize) -> bool {
        match self.cpus.get(seat) {
            // Human seat with no CPU.
            Some(None) => true,
            // Shadow seats buffer; pure CPU seats do not.
            Some(Some(cpu)) => cpu.mirror,
            None => false,
        }
    }

    /// Queues a batch of CPU actions.
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

    /// Applies at most the front batch whose time has come.
    fn apply_ready_cpu_batch(&mut self, now: f64) -> bool {
        let Some(pending) = self.pending_cpu_batches.front() else {
            return false;
        };
        if pending.ready_at > now {
            return false;
        }

        let pending = self.pending_cpu_batches.pop_front().unwrap();
        for (seat, action) in pending.actions {
            self.apply_cpu_action(seat, action);
        }

        true
    }

    /// Delivers events to CPU clients and collects actions from driven
    /// seats. Shadow CPUs (controlled = false) still receive events, but
    /// their actions are discarded.
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
                // Later events in the same server batch carry the final
                // phase. In particular, NineTerminalsAvailable follows
                // TileDrawn and must supersede the speculative draw action.
                if let Some((_, queued)) = actions
                    .iter_mut()
                    .find(|(queued_seat, _)| seat == queued_seat)
                {
                    *queued = action;
                } else {
                    actions.push((*seat, action));
                }
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

    #[test]
    fn test_cpu_receives_open_tanyao_rule() {
        let mut settings = GameSettings::default();
        settings.rules.opened_all_inside = false;
        let mut driver = GameDriver::new(settings);

        driver.set_cpu(0, default_cpu_configs()[0].clone());

        assert!(
            !driver.cpus[0]
                .as_ref()
                .unwrap()
                .client
                .state
                .opened_all_inside
        );
    }

    /// Driver with seat 0 human and the rest CPUs.
    fn driver_with_three_cpus() -> GameDriver {
        let mut driver = GameDriver::new(GameSettings::default());
        let configs = default_cpu_configs();
        for (i, config) in configs.into_iter().enumerate() {
            driver.set_cpu(i + 1, config);
        }
        driver
    }

    #[test]
    fn test_start_game_randomizes_dealer() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..64 {
            let mut driver = driver_with_three_cpus();
            driver.start_game();
            let dealer = driver.table().dealer;
            assert!(dealer < 4);
            seen.insert(dealer);
        }
        // Odds of one dealer in 64 runs are (1/4)^63: effectively zero.
        assert!(seen.len() > 1, "起家がランダム化されていない");
    }

    /// Seeded starts keep the dealer at seat 0 for reproducibility.
    #[test]
    fn test_start_game_with_seed_keeps_dealer_zero() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);
        assert_eq!(driver.table().dealer, 0);
    }

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

        // Pure CPU seats never buffer.
        for seat in 1..4 {
            assert!(driver.drain_events(seat).is_empty());
        }
    }

    #[test]
    fn test_nine_terminals_event_supersedes_draw_action_in_same_batch() {
        let mut driver = GameDriver::new(GameSettings::default());
        driver.set_cpu(0, default_cpu_configs()[0].clone());
        let cpu = driver.cpus[0].as_mut().unwrap();
        cpu.client.state.my_hand = mahjong_core::hand::Hand::from("1m9m1p9p1s9s1z2z3z4z5z6z5m")
            .tiles()
            .to_vec();

        let actions = driver.collect_cpu_actions(&[
            (
                0,
                ServerEvent::TileDrawn {
                    tile: Tile::new(Tile::Z7),
                    remaining_tiles: 69,
                    can_tsumo: false,
                    can_riichi: false,
                    is_furiten: false,
                },
            ),
            (0, ServerEvent::NineTerminalsAvailable),
        ]);

        assert_eq!(
            actions,
            vec![(0, ClientAction::NineTerminals { declare: false })]
        );
    }

    /// The hand must run to completion with a tsumogiri-only human.
    #[test]
    fn test_seeded_round_runs_to_completion_with_tsumogiri_human() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);
        let _ = driver.drain_events(0);

        // Generous iteration cap to avoid an infinite loop on failure.
        for _ in 0..1000 {
            if driver.is_round_over() {
                break;
            }

            driver.tick();
            let events = driver.drain_events(0);

            // Decline any nine-terminals offer; the server then re-sends
            // TileDrawn to prompt the discard.
            let nine_terminals = events
                .iter()
                .any(|e| matches!(e, ServerEvent::NineTerminalsAvailable));
            if nine_terminals {
                driver.handle_action(0, ClientAction::NineTerminals { declare: false });
            }

            for event in &events {
                match event {
                    ServerEvent::TileDrawn { can_tsumo, .. } if !nine_terminals => {
                        if *can_tsumo {
                            driver.handle_action(0, ClientAction::Tsumo);
                        } else {
                            driver.handle_action(0, ClientAction::Discard { tile: None });
                        }
                    }
                    ServerEvent::CallAvailable { .. } => {
                        driver.handle_action(0, ClientAction::Pass);
                    }
                    _ => {}
                }
            }
        }

        assert!(driver.is_round_over(), "局が終了しなかった");

        let round = driver.table().current_round().unwrap();
        assert!(round.result.is_some(), "局結果が設定されていない");
    }

    /// A shadow CPU receives events but never acts.
    #[test]
    fn test_shadow_cpu_does_not_act() {
        let mut driver = driver_with_three_cpus();
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);
        assert!(!driver.is_cpu_controlled(0));

        driver.start_game_with_seed(42);
        driver.run_until_blocked();

        // Stopped waiting on seat 0: the shadow CPU did not discard.
        let round = driver.table().current_round().unwrap();
        assert!(!round.is_over());
        assert_eq!(round.current_player, 0);
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);

        let events = driver.drain_events(0);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::TileDrawn { .. })),
            "シャドーCPU座席にTileDrawnが届いていない"
        );
    }

    /// Switching a shadow CPU to controlled auto-plays the hand.
    #[test]
    fn test_shadow_cpu_takeover_completes_round() {
        let mut driver = driver_with_three_cpus();
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);

        driver.start_game_with_seed(42);
        driver.run_until_blocked();
        assert!(!driver.is_round_over());

        assert!(driver.set_cpu_controlled(0, true));
        assert!(driver.is_cpu_controlled(0));

        // Seat 0 is mid-discard at takeover, so kick it with the default
        // action; with every seat now CPU-driven the hand auto-completes.
        assert!(driver.force_default_action(0));
        driver.run_until_blocked();

        assert!(driver.is_round_over(), "代打ち後に局が進行しなかった");
    }

    /// Shadow seats must keep buffering during substitution, so the room
    /// can record the hand's progress for post-disconnect reconnection.
    #[test]
    fn test_mirror_seat_buffers_events_during_takeover() {
        let mut driver = driver_with_three_cpus();
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);

        driver.start_game_with_seed(42);
        driver.run_until_blocked();
        let _ = driver.drain_events(0);

        // Simulate a disconnect: switch seat 0 to CPU control.
        assert!(driver.set_cpu_controlled(0, true));
        assert!(driver.force_default_action(0));

        for _ in 0..20 {
            if driver.is_round_over() {
                break;
            }
            driver.run_until_blocked();
            if !driver.is_round_over() {
                driver.force_default_action(driver.table().current_round().unwrap().current_player);
            }
        }

        let events = driver.drain_events(0);
        assert!(
            !events.is_empty(),
            "代打ち中のシャドーCPU席にイベントが記録されていない"
        );

        assert!(
            driver.drain_events(1).is_empty(),
            "純粋なCPU席にイベントがバッファされている"
        );
    }

    #[test]
    fn test_needs_tick_distinguishes_human_wait_from_cpu_progress() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);
        driver.run_until_blocked();

        let round = driver.table().current_round().unwrap();
        assert_eq!(round.current_player, 0);
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert!(!driver.needs_tick(), "人間の打牌待ちでは tick 不要");

        driver.handle_action(0, ClientAction::Discard { tile: None });
        // Immediate mode races through the CPUs, so use a paced driver
        // to leave a pending batch observable.
        let mut paced = driver_with_three_cpus();
        paced.set_cpu_action_delay(1.0);
        paced.start_game_with_seed(42);
        for _ in 0..50 {
            if !paced.needs_tick() {
                break;
            }
            paced.tick_at(0.0);
        }
        assert!(!paced.needs_tick(), "人間の打牌待ちで停止するはず");
        paced.handle_action_at(0, ClientAction::Discard { tile: None }, 0.0);
        assert!(paced.needs_tick(), "打牌後は CPU 進行のため tick が必要");
    }

    #[test]
    fn test_set_cpu_controlled_requires_cpu_client() {
        let mut driver = GameDriver::new(GameSettings::default());
        assert!(!driver.set_cpu_controlled(0, true));
        assert!(!driver.is_cpu_controlled(0));
    }

    /// Releasing CPU control must discard unapplied CPU actions, so a
    /// discard queued during substitution cannot hijack the reconnected
    /// human's play.
    #[test]
    fn test_releasing_cpu_control_discards_pending_actions() {
        let mut driver = driver_with_three_cpus();
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);
        driver.set_cpu_action_delay(1.0);
        driver.table_mut().start_round();

        {
            let round = driver.table_mut().current_round_mut().unwrap();
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.players[0].draw(Tile::new(Tile::Z7));
            round.drain_events();
        }

        // Simulate a disconnect: take over and queue a CPU discard.
        assert!(driver.set_cpu_controlled(0, true));
        driver.schedule_cpu_actions(vec![(0, ClientAction::Discard { tile: None })], 1.0, 10.0);

        // Reconnect: releasing control drops the queued action.
        assert!(driver.set_cpu_controlled(0, false));
        assert!(driver.pending_cpu_batches.is_empty());

        // Past the scheduled time, still no discard: waiting on the human.
        driver.tick_at(20.0);
        let round = driver.table().current_round().unwrap();
        assert_eq!(round.phase, TurnPhase::WaitForDiscard);
        assert_eq!(round.current_player, 0);
    }

    /// Releasing one seat's control must not drop other seats'
    /// queued actions.
    #[test]
    fn test_releasing_cpu_control_keeps_other_seats_actions() {
        let mut driver = driver_with_three_cpus();
        let config = default_cpu_configs()[0].clone();
        driver.set_shadow_cpu(0, config);
        driver.set_cpu_action_delay(1.0);
        driver.table_mut().start_round();

        driver.set_cpu_controlled(0, true);
        driver.schedule_cpu_actions(
            vec![
                (0, ClientAction::Pass),
                (1, ClientAction::Discard { tile: None }),
            ],
            1.0,
            10.0,
        );

        driver.set_cpu_controlled(0, false);
        assert_eq!(driver.pending_cpu_batches.len(), 1);
        assert_eq!(
            driver.pending_cpu_batches.front().unwrap().actions,
            vec![(1, ClientAction::Discard { tile: None })]
        );
    }

    /// Right after a call there is no drawn tile, so the default action
    /// must discard from the hand: tsumogiri (None) fails there, which
    /// used to make timeout/CPU fallbacks whiff and stall the hand. It
    /// must also avoid swap-calling-forbidden tiles.
    #[test]
    fn test_force_default_action_discards_from_hand_without_drawn() {
        let mut driver = driver_with_three_cpus();
        driver.table_mut().start_round();
        {
            let round = driver.table_mut().current_round_mut().unwrap();
            let seat_wind = round.players[0].seat_wind;
            let hand = Hand::from("1m2m3m4m5m6m7m8m9m1p2p3p 4p");
            round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
            // Post-call state: no drawn tile, and the trailing 3p is
            // forbidden by the swap-calling rule.
            round.players[0].set_forbidden_discards(vec![Tile::P3]);
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        assert!(driver.force_default_action(0));
        let round = driver.table().current_round().unwrap();
        assert_eq!(round.players[0].discards.len(), 1, "既定打牌が空振りした");
        // Skips the forbidden 3p and discards the next-from-last 2p.
        assert_eq!(round.players[0].discards[0].tile.get(), Tile::P2);
    }

    /// A rejected CPU action must fall back to the default action (#296):
    /// when CPU judgement and server validation disagreed (e.g. a pei
    /// declaration on the last draw), the CPU was never re-consulted and
    /// the hand stalled forever.
    #[test]
    fn test_rejected_cpu_action_falls_back_to_default() {
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

        // Pei is always invalid in four-player games.
        driver.schedule_cpu_actions(vec![(1, ClientAction::Pei)], 1.0, 10.0);
        driver.tick_at(20.0);

        // After the rejection it fell back to tsumogiri and the hand moved on.
        let round = driver.table().current_round().unwrap();
        assert_eq!(
            round.players[1].discards.len(),
            1,
            "却下されたCPUアクションがフォールバックされず局が停止している"
        );
    }

    #[test]
    fn test_run_until_blocked_stops_at_human_turn() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(123);
        driver.run_until_blocked();

        let round = driver.table().current_round().unwrap();
        if !round.is_over() {
            // Must stop outside the draw phase: human input or hand over.
            assert_ne!(round.phase, TurnPhase::Draw);
        }
    }

    #[test]
    fn test_kan_advances_game() {
        let mut driver = driver_with_three_cpus();

        driver.table_mut().start_round();

        {
            let round = driver.table_mut().current_round_mut().unwrap();
            let seat_wind = round.players[0].seat_wind;
            // Three 1m in hand plus a drawn 1m enables a concealed kan.
            let hand = Hand::from("2p3p4p5s6s7s7m8m9m1m1m1m 1m");
            round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
            round.players[0].draw(hand.drawn().unwrap());
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        driver.pump(None);
        let _ = driver.drain_events(0);

        let kan_result = driver.handle_action(
            0,
            ClientAction::Kan {
                tile_index: Tile::M1 as usize,
            },
        );
        assert!(kan_result, "カンが失敗した");

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

        let discard_result = driver.handle_action(0, ClientAction::Discard { tile: None });
        assert!(discard_result, "カン後の打牌が失敗した");
    }

    #[test]
    fn test_cpu_kan_advances_game() {
        let mut driver = driver_with_three_cpus();

        driver.table_mut().start_round();

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

        // The CPU decides on its own whether to kan or discard.
        driver.pump(None);

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

        // Run ten more turns to prove the game does not freeze.
        for _i in 0..10 {
            driver.tick();
            let _ = driver.drain_events(0);
            {
                let round = driver.table().current_round().unwrap();
                if round.is_over() {
                    break;
                }
            }
            {
                let (phase, current_player) = {
                    let round = driver.table().current_round().unwrap();
                    (round.phase.clone(), round.current_player)
                };
                if phase == TurnPhase::WaitForDiscard && current_player == 0 {
                    driver.handle_action(0, ClientAction::Discard { tile: None });
                }
            }
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

    #[test]
    fn test_no_delay_keeps_immediate_progression() {
        let mut driver = driver_with_three_cpus();
        driver.start_game_with_seed(42);
        driver.tick();
        assert!(driver.pending_cpu_batches.is_empty());
    }

    /// Regression: with per-seat `drain_events_at`, a `CallAvailable`
    /// generated while pumping a later seat only landed in an
    /// already-drained earlier seat's buffer and missed the flush — in
    /// online play the notification never arrived and the game appeared
    /// stuck until the action-timeout force discard.
    ///
    /// Builds the chain: seat 2 discards -> seat 3 pons -> post-pon
    /// discard -> seat 0's call chance, deliberately split into two
    /// pending batches (one `pump` only reaches the pon), and resolves it
    /// within a single `drain_all_events_at` call. Seat 0 — the lowest
    /// index, drained first — must still receive `CallAvailable` in the
    /// same flush.
    ///
    /// No CPU clients are assigned; every action is injected manually so
    /// the test does not depend on actual CPU decision-making.
    #[test]
    fn test_drain_all_events_at_delivers_chained_call_available_in_same_flush() {
        let mut driver = GameDriver::new(GameSettings::default());
        driver.set_cpu_action_delay(0.0);
        driver.table_mut().start_round();

        let pon_tiles = [Tile::new(Tile::M5), Tile::new(Tile::M5)];

        {
            let round = driver.table_mut().current_round_mut().unwrap();

            // Seat 0: 13 tiles that can pon a 9s.
            let seat0_wind = round.players[0].seat_wind;
            let seat0_tiles = vec![
                Tile::new(Tile::S9),
                Tile::new(Tile::S9),
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::M4),
                Tile::new(Tile::M6),
                Tile::new(Tile::M7),
                Tile::new(Tile::P1),
                Tile::new(Tile::P2),
                Tile::new(Tile::P3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P6),
            ];
            round.players[0] = Player::new(seat0_wind, seat0_tiles, 25000);

            // Seat 3: two 5m for the pon plus a 9s to discard afterwards.
            let seat3_wind = round.players[3].seat_wind;
            let seat3_tiles = vec![
                Tile::new(Tile::M5),
                Tile::new(Tile::M5),
                Tile::new(Tile::S9),
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P1),
                Tile::new(Tile::P2),
                Tile::new(Tile::P3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::P7),
            ];
            round.players[3] = Player::new(seat3_wind, seat3_tiles, 25000);

            // Seat 2 will discard the 5m.
            let seat2_wind = round.players[2].seat_wind;
            let hand = Hand::from("1p2p3p4p6p7p8p1s2s3s4s6s 5m");
            round.players[2] = Player::new(seat2_wind, hand.tiles().to_vec(), 25000);
            round.players[2].draw(hand.drawn().unwrap());
            round.current_player = 2;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }
        let _ = driver.drain_all_events_at(0.0);

        // Seat 2 discards the drawn 5m, giving seat 3 a pon chance.
        driver.handle_action_at(2, ClientAction::Discard { tile: None }, 0.0);

        // Inject the pon and the post-pon discard as separate batches:
        // one pump then only reaches the pon, and the post-pon discard
        // (with the CallAvailable it generates for seat 0) only appears
        // on the next pump - reproducing the bug's setup.
        driver.schedule_cpu_actions(vec![(3, ClientAction::Pon { tiles: pon_tiles })], 0.0, 0.0);
        driver.schedule_cpu_actions(
            vec![(
                3,
                ClientAction::Discard {
                    tile: Some(Tile::new(Tile::S9)),
                },
            )],
            0.0,
            0.0,
        );

        // One call must resolve the whole chain.
        let per_seat = driver.drain_all_events_at(0.0);

        assert!(
            per_seat[0]
                .iter()
                .any(|e| matches!(e, ServerEvent::CallAvailable { .. })),
            "座席0にCallAvailableが同一フラッシュで届いていない: {:?}",
            per_seat[0]
        );
    }
}
