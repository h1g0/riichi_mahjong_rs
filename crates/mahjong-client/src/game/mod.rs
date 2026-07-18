//! Client-side game state, driven by the events received from the
//! server.

use std::collections::VecDeque;

use macroquad::prelude::*;
use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::scoring::score::{ScoreItem, ScoreRank};
use mahjong_core::settings::Lang;
use mahjong_core::tile::{Tile, TileType, Wind};

use crate::i18n::{Key, Translator};
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::protocol::net::CpuSpec;
use mahjong_server::protocol::{
    AvailableCall, CallType, ClientAction, PlayerHandInfo, ServerEvent,
};

mod events;
mod input;
mod labels;
mod setup;
#[cfg(test)]
mod tests;

pub use labels::PlayerLabel;
pub use setup::{GameMode, OnlineUiState, RoomViewUi, RuleOption, SetupState};

use labels::*;

/// Seconds later events stay held after a call/riichi banner appears.
pub const CALL_HOLD_SECS: f64 = 0.9;
/// Seconds between a win/nine-terminals banner and the result screen.
pub const WIN_HOLD_SECS: f64 = 1.2;
/// Seconds a declaration banner stays visible.
pub const CALL_BANNER_SECS: f64 = 1.5;
/// Delay before the automatic riichi tsumogiri, so the drawn tile is
/// visible first.
pub const RIICHI_AUTO_DISCARD_SECS: f64 = 1.0;

/// A declaration banner (the on-screen stand-in for calling out).
#[derive(Debug, Clone, Copy)]
pub struct CallBanner {
    /// The text to show (pon, chii, kan, riichi, ron, tsumo, ...)
    pub label: Key,
    /// Display start time, on the clock passed to
    /// [`GameState::process_events`]
    pub shown_at: f64,
}

/// What a declaration event displays (returned by `declaration_for`).
struct Declaration {
    /// Banner player(s) and text; several on a multiple ron
    banners: Vec<(Wind, Key)>,
    /// Seconds to hold later events
    hold_secs: f64,
    /// Whether the event itself is also held (wins, nine terminals)
    before_apply: bool,
}

/// Value of one riichi deposit stick, in points. Mirrors the server's
/// private `RIICHI_STICK_VALUE`; needed here because `WinResult::score_points`
/// carries the merged total and the result screen displays the deposit
/// portion separately.
pub(crate) const RIICHI_STICK_VALUE: i32 = 1000;

/// Heading used by the message-style hand result panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageResultKind {
    Draw,
    NagashiMangan,
}

/// One winner's result (one page of the result screen).
#[derive(Debug, Clone)]
pub struct WinResult {
    pub win_hand: Vec<Tile>,
    pub win_melds: Vec<Meld>,
    pub win_tile: Option<Tile>,
    pub win_is_tsumo: bool,
    pub uradora_indicators: Vec<Tile>,
    pub result_message: String,
    /// The winner's display name
    pub winner_name: String,
    /// The deal-in player's display name; None on tsumo
    pub loser_name: Option<String>,
    /// Awarded yaku as (name, han) pairs
    pub yaku: Vec<(String, u32)>,
    /// Han
    pub han: u32,
    /// Fu
    pub fu: u32,
    /// Points won
    pub score_points: i32,
    /// Score rank name (mangan etc.; usually empty)
    pub rank_name: String,
    /// Structured score rank used to decide whether han/fu are displayed
    pub rank: ScoreRank,
    /// Number of awarded yakuman units; zero for non-yakuman and counted yakuman
    pub yakuman_multiplier: u32,
    /// Riichi deposits collected with this win
    pub riichi_sticks: usize,
    /// Continuance counter collected with this win
    pub honba: usize,
    /// Points collected for the continuance counter
    pub honba_points: i32,
}

impl WinResult {
    /// Riichi-deposit portion of the merged winner gain.
    pub(crate) fn riichi_points(&self) -> i32 {
        self.riichi_sticks as i32 * RIICHI_STICK_VALUE
    }

    /// Hand payment before table bonuses.
    pub(crate) fn hand_points(&self) -> i32 {
        self.score_points - self.riichi_points() - self.honba_points
    }
}

/// Display info for one discard.
#[derive(Debug, Clone)]
pub struct DiscardInfo {
    pub tile: Tile,
    pub is_tsumogiri: bool,
    /// Whether this was the riichi declaration tile (drawn sideways)
    pub is_riichi: bool,
    /// Whether another player called it (drawn dimmed)
    pub is_called: bool,
}

/// Gap-closing animation state for an opponent's hand discard.
///
/// As at a real table, where everyone sees which part of the hand a
/// discard came from, the vacated slot shows briefly before the tiles to
/// its right slide left.
#[derive(Debug, Clone, Copy)]
pub struct TedashiAnim {
    /// 0-based position the tile left, within the sorted pre-discard hand
    pub gap_index: usize,
    /// Whether a drawn tile was hanging out (it slides into the hand)
    pub had_drawn: bool,
    /// Animation start time, on the process_events clock
    pub started_at: f64,
}

/// Display info for an opponent's hand, indexed relative to us.
#[derive(Debug, Clone)]
pub struct OtherPlayerHand {
    /// Tiles; set only when revealed
    pub hand: Vec<Tile>,
    /// Melds
    pub melds: Vec<Meld>,
    /// Whether the hand is revealed (wins, tenpai at a draw)
    pub revealed: bool,
    /// Hidden-hand tile count (excluding the drawn tile), for face-down display
    pub concealed_count: usize,
    /// Whether a drawn tile hangs to the right (from draw to discard/pei)
    pub has_drawn: bool,
    /// Gap-closing animation of the latest hand discard
    pub tedashi_anim: Option<TedashiAnim>,
}

impl OtherPlayerHand {
    fn new() -> Self {
        OtherPlayerHand {
            hand: Vec::new(),
            melds: Vec::new(),
            revealed: false,
            concealed_count: 13,
            has_drawn: false,
            tedashi_anim: None,
        }
    }

    /// Updates the displayed count after n tiles leave the hand + drawn
    /// tile (discard, meld, pei).
    ///
    /// A hanging drawn tile is considered merged into the hand, matching
    /// the server's `Player::try_discard`.
    fn consume_tiles(&mut self, n: usize) {
        let total = self.concealed_count + usize::from(self.has_drawn);
        self.concealed_count = total.saturating_sub(n);
        self.has_drawn = false;
    }
}

/// The client-side game state.
pub struct GameState {
    /// Our seat wind
    pub seat_wind: Option<Wind>,
    /// Our hand
    pub hand: Vec<Tile>,
    /// The drawn tile
    pub drawn: Option<Tile>,
    /// Discards per player (0 = self, 1 = right, 2 = across, 3 = left)
    pub discards: [Vec<DiscardInfo>; 4],
    /// Scores
    pub scores: [i32; 4],
    /// Round wind
    pub round_wind: Option<Wind>,
    /// Dora indicators
    pub dora_indicators: Vec<Tile>,
    /// Ura dora indicators (revealed only on a riichi win)
    pub uradora_indicators: Vec<Tile>,
    /// The winning hand, for the result screen
    pub win_hand: Vec<Tile>,
    /// The winning hand's melds
    pub win_melds: Vec<Meld>,
    /// The winning tile
    pub win_tile: Option<Tile>,
    /// Whether the win was by tsumo
    pub win_is_tsumo: bool,
    /// Tiles left in the wall
    pub remaining_tiles: usize,
    /// Index of the selected tile
    pub selected_tile: Option<usize>,
    /// Whether the drawn tile is selected
    pub selected_drawn: bool,
    /// Whether tsumo is possible
    pub can_tsumo: bool,
    /// Whether riichi may be declared
    pub can_riichi: bool,
    /// Tiles we could declare a concealed kan on this turn
    pub self_kan_options: Vec<Tile>,
    /// Whether we are in riichi
    pub is_riichi: bool,
    /// When the automatic riichi tsumogiri fires; Some only while the
    /// drawn tile is being shown
    riichi_auto_discard_at: Option<f64>,
    /// Whether we are choosing the riichi declaration discard
    pub riichi_selection_mode: bool,
    /// Hand indices that keep tenpai for riichi
    pub riichi_selectable_tiles: Vec<usize>,
    /// Whether tsumogiri also keeps tenpai for riichi
    pub riichi_selectable_drawn: bool,
    /// Tile kinds the swap-calling rule forbids on the post-call discard;
    /// set right after a chii/pon and cleared by discarding or drawing
    pub forbidden_discards: Vec<TileType>,
    /// Whether a forbidden tile was clicked, to show the warning
    pub selected_forbidden_swap: bool,
    /// The hand's result message
    pub result_message: Option<String>,
    /// Result type for the message-style panel (draw or Nagashi Mangan)
    message_result_kind: MessageResultKind,
    /// Win results; several on a multiple ron
    pub win_results: Vec<WinResult>,
    /// Index of the win result being shown
    pub win_result_index: usize,
    /// Whether it is our turn
    pub is_my_turn: bool,
    /// Seat wind of the player to act; None between hands
    pub turn_player: Option<Wind>,
    /// Game phase
    pub phase: GamePhase,
    /// Available calls
    pub available_calls: Vec<AvailableCall>,
    /// The tile the calls are on
    pub call_target_tile: Option<Tile>,
    /// Who discarded that tile
    pub call_discarder: Option<Wind>,
    /// Our melds
    pub melds: Vec<Meld>,
    /// Hand number (0 = East 1, ...)
    pub round_number: usize,
    /// Continuance counter (honba)
    pub honba: usize,
    /// Riichi deposits on the table
    pub riichi_sticks: usize,
    /// Whether we are furiten
    pub is_furiten: bool,
    /// Whether discarding the selected tile would leave us furiten
    pub selected_would_cause_furiten: bool,
    /// Opponents' hands (0 = right, 1 = across, 2 = left)
    pub other_players: [OtherPlayerHand; 3],
    /// Wind of a player whose next discard is their riichi declaration
    /// tile (transient)
    pending_riichi_player: Option<Wind>,
    /// Wind of the last discarder, used to attribute calls
    last_discarder: Option<Wind>,
    /// Whether the chii option picker is open
    pub chi_option_selecting: bool,
    /// Chii options (the two hand tiles per option)
    pub chi_pending_options: Vec<[Tile; 2]>,
    /// Whether the pon option picker is open (red-five split)
    pub pon_option_selecting: bool,
    /// Pon options (the two hand tiles per option)
    pub pon_pending_options: Vec<[Tile; 2]>,
    /// Whether the nine-terminals choice is open
    pub nine_terminals_pending: bool,
    /// Pre-game setup state
    pub setup_state: SetupState,
    /// Rule whose description is shown on the rule-settings screen
    pub selected_rule: RuleOption,
    /// Online UI state
    pub online_state: OnlineUiState,
    /// Player types per seat, in the same order as `scores`
    pub player_labels: [PlayerLabel; 4],
    /// Our seat index (0 locally; your_seat online)
    pub my_seat: usize,
    /// The starting dealer's seat, recovered from GameStarted.
    ///
    /// The starting dealer is random, so it need not be seat 0; used to
    /// break final-standings ties (closer to the starting dealer wins).
    pub initial_dealer_seat: usize,
    /// Player count (4 or 3), set by GameStarted
    pub player_count: usize,
    /// Whether pei dora is enabled (three-player only)
    pub nuki_dora: bool,
    /// Extracted North count per player, indexed by wind
    pub pei_counts: [u8; 4],
    /// Whether pei is possible on our turn
    pub can_pei: bool,
    /// Declaration banners per player (relative order)
    pub call_banners: [Option<CallBanner>; 4],
    /// Server events not yet applied (held during declaration effects)
    pending_events: VecDeque<ServerEvent>,
    /// Later events stay held until this time
    event_hold_until: f64,
    /// Whether the front event's banner has been shown (pre-apply hold)
    head_announced: bool,
    /// The time last passed to [`process_events`](Self::process_events);
    /// used as the animation start time when applying events.
    clock: f64,
    /// Display language
    pub lang: Lang,
}

/// Where the mode/CPU screens were opened from; decides the back
/// target and the confirm action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOrigin {
    /// Local CPU play (title -> mode -> CPU setup -> start)
    Local,
    /// Online play (mode before room creation / CPU setup from the lobby)
    Online,
}

/// Game phase.
#[derive(Debug, Clone, PartialEq)]
pub enum GamePhase {
    /// Title screen
    TopMenu,
    /// Game-mode selection
    ModeSelect(MenuOrigin),
    /// Rule settings opened from game-mode selection
    RuleSettings(MenuOrigin),
    /// CPU setup (level/personality; starts locally, confirms online)
    CpuSetup(MenuOrigin),
    /// Online menu (name and room-code entry)
    OnlineMenu,
    /// Online lobby (waiting for members)
    OnlineLobby,
    /// Before the game starts
    WaitingForStart,
    /// In game
    Playing,
    /// Hand over (result showing)
    RoundResult,
    /// Game over
    GameOver,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            seat_wind: None,
            hand: Vec::new(),
            drawn: None,
            discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            scores: [25000; 4],
            round_wind: None,
            dora_indicators: Vec::new(),
            uradora_indicators: Vec::new(),
            win_hand: Vec::new(),
            win_melds: Vec::new(),
            win_tile: None,
            win_is_tsumo: false,
            remaining_tiles: 70,
            selected_tile: None,
            selected_drawn: false,
            can_tsumo: false,
            can_riichi: false,
            self_kan_options: Vec::new(),
            is_riichi: false,
            riichi_selection_mode: false,
            riichi_selectable_tiles: Vec::new(),
            riichi_selectable_drawn: false,
            forbidden_discards: Vec::new(),
            selected_forbidden_swap: false,
            result_message: None,
            message_result_kind: MessageResultKind::Draw,
            win_results: Vec::new(),
            win_result_index: 0,
            is_my_turn: false,
            turn_player: None,
            phase: GamePhase::TopMenu,
            available_calls: Vec::new(),
            call_target_tile: None,
            call_discarder: None,
            melds: Vec::new(),
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
            is_furiten: false,
            selected_would_cause_furiten: false,
            other_players: [
                OtherPlayerHand::new(),
                OtherPlayerHand::new(),
                OtherPlayerHand::new(),
            ],
            pending_riichi_player: None,
            last_discarder: None,
            chi_option_selecting: false,
            chi_pending_options: Vec::new(),
            pon_option_selecting: false,
            pon_pending_options: Vec::new(),
            nine_terminals_pending: false,
            setup_state: SetupState::new(),
            selected_rule: RuleOption::OpenAllInside,
            online_state: OnlineUiState::new(),
            player_labels: [
                PlayerLabel::Me,
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "Balanced".to_string(),
                },
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "Speedy".to_string(),
                },
                PlayerLabel::Cpu {
                    level: "Normal".to_string(),
                    personality: "HighValue".to_string(),
                },
            ],
            my_seat: 0,
            initial_dealer_seat: 0,
            player_count: 4,
            nuki_dora: false,
            pei_counts: [0; 4],
            can_pei: false,
            riichi_auto_discard_at: None,
            call_banners: [None; 4],
            pending_events: VecDeque::new(),
            event_hold_until: 0.0,
            head_announced: false,
            clock: 0.0,
            // Load the saved display language (Japanese when unsaved) so
            // the choice survives new() being rebuilt on "play again".
            lang: crate::persistence::load_lang().unwrap_or(Lang::Ja),
        }
    }

    /// Whether this is a three-player game.
    pub fn is_three_player(&self) -> bool {
        self.player_count == 3
    }

    /// The [`Translator`](crate::i18n::Translator) for the current language.
    pub fn tr(&self) -> crate::i18n::Translator {
        crate::i18n::Translator::new(self.lang)
    }

    /// Tile kind currently selected in our hand.
    ///
    /// Red fives intentionally return the same kind as normal fives so
    /// every publicly visible copy of that tile kind can be highlighted.
    pub(crate) fn selected_tile_type(&self) -> Option<TileType> {
        if self.selected_drawn {
            self.drawn.map(|tile| tile.get())
        } else {
            self.selected_tile
                .and_then(|idx| self.hand.get(idx))
                .map(|tile| tile.get())
        }
    }

    /// Localized heading for a message-style hand result.
    pub fn message_result_heading(&self) -> &'static str {
        match self.message_result_kind {
            MessageResultKind::Draw => Key::RoundDraw.text(self.lang),
            MessageResultKind::NagashiMangan => mahjong_core::winning_hand::name::get(
                mahjong_core::winning_hand::name::Kind::NagashiMangan,
                false,
                self.lang,
            ),
        }
    }

    /// Sets local-play player types: us at seat 0, CPUs at 1-3.
    pub fn set_local_players(&mut self, cpu_configs: &[CpuConfig; 3]) {
        self.my_seat = 0;
        self.player_labels = [
            PlayerLabel::Me,
            cpu_label(&cpu_configs[0]),
            cpu_label(&cpu_configs[1]),
            cpu_label(&cpu_configs[2]),
        ];
    }

    /// Sets online player types; `seats` is in seat order and
    /// `your_seat` is our own.
    pub fn set_online_players(&mut self, seats: &[PlayerLabel; 4], your_seat: usize) {
        self.my_seat = your_seat;
        self.player_labels = seats.clone();
    }

    /// Final standings as (seat, score) from first to last.
    ///
    /// Ties go to the seat closer to the starting dealer; the dummy seat
    /// is excluded in three-player games.
    pub fn final_rankings(&self) -> Vec<(usize, i32)> {
        let n = self.player_count;
        let mut rankings: Vec<(usize, i32)> = self
            .scores
            .iter()
            .enumerate()
            .take(n)
            .map(|(i, &s)| (i, s))
            .collect();
        rankings.sort_by_key(|&(seat, score)| {
            (
                std::cmp::Reverse(score),
                (seat + n - self.initial_dealer_seat) % n,
            )
        });
        rankings
    }

    pub fn my_wind_index(&self) -> usize {
        self.seat_wind.map(|w| w.to_index()).unwrap_or(0)
    }

    pub fn my_initial_wind_index(&self) -> usize {
        (self.my_seat + self.player_count - self.initial_dealer_seat) % self.player_count
    }

    fn relative_player_index(&self, wind: Wind) -> usize {
        let my_idx = self.my_wind_index();
        let their_idx = wind.to_index();
        // Wind indices cycle over 0-2 in three-player games.
        (their_idx + self.player_count - my_idx) % self.player_count
    }

    fn initial_wind_index(&self, wind: Wind) -> usize {
        (self.my_initial_wind_index() + self.relative_player_index(wind)) % self.player_count
    }

    fn call_type_to_meld_type(call_type: &CallType) -> MeldType {
        match call_type {
            CallType::Chi => MeldType::Chi,
            CallType::Pon => MeldType::Pon,
            CallType::Ankan | CallType::Daiminkan => MeldType::Kan,
            CallType::Kakan => MeldType::Kakan,
            CallType::Ron => MeldType::Pon, // Unused fallback.
        }
    }

    fn compute_meld_direction(&self, caller: Wind, discarder: Wind) -> MeldFrom {
        let (caller_idx, discarder_idx) = if self.player_count == 3 {
            (
                self.initial_wind_index(caller),
                self.initial_wind_index(discarder),
            )
        } else {
            (caller.to_index(), discarder.to_index())
        };
        let rel = (discarder_idx + 4 - caller_idx) % 4;
        match rel {
            3 => MeldFrom::Previous,
            2 => MeldFrom::Opposite,
            1 => MeldFrom::Following,
            _ => MeldFrom::Myself, // Not normally reachable.
        }
    }

    /// Queues a server event; [`process_events`](Self::process_events)
    /// applies it.
    pub fn queue_event(&mut self, event: ServerEvent) {
        self.pending_events.push_back(event);
    }

    /// Applies queued events in order; call every frame.
    ///
    /// Declaration events (calls, riichi, pei, wins, nine terminals) show
    /// a banner and hold later events for [`CALL_HOLD_SECS`]
    /// ([`WIN_HOLD_SECS`] for wins), so the "call-out" is seen before its
    /// effect and players notice the declaration. Wins and nine terminals
    /// also hold the event itself (the result-screen transition).
    pub fn process_events(&mut self, now: f64) {
        self.clock = now;

        // Retire banners past their display time.
        for slot in &mut self.call_banners {
            if slot.is_some_and(|b| now - b.shown_at >= CALL_BANNER_SECS) {
                *slot = None;
            }
        }

        loop {
            if now < self.event_hold_until || self.pending_events.is_empty() {
                return;
            }

            if !self.head_announced
                && let Some(decl) = self.declaration_for(&self.pending_events[0])
            {
                for &(wind, label) in &decl.banners {
                    let rel = self.relative_player_index(wind);
                    self.call_banners[rel] = Some(CallBanner {
                        label,
                        shown_at: now,
                    });
                }
                self.event_hold_until = now + decl.hold_secs;
                if decl.before_apply {
                    // Show the declaration now; apply the event when the
                    // hold ends. Collapse stale action UI (ron/tsumo
                    // buttons) so it cannot linger through the hold.
                    self.head_announced = true;
                    self.available_calls.clear();
                    self.can_tsumo = false;
                    self.can_riichi = false;
                } else {
                    let event = self.pending_events.pop_front().expect("front checked");
                    self.handle_event(event);
                    // Our own call is followed immediately by HandUpdated.
                    // Holding it would let a discard made during the hold
                    // be rolled back by the late HandUpdated, desyncing the
                    // hand - so apply both together.
                    if matches!(
                        self.pending_events.front(),
                        Some(ServerEvent::HandUpdated { .. })
                    ) {
                        let event = self.pending_events.pop_front().expect("front checked");
                        self.handle_event(event);
                    }
                }
                continue;
            }

            let event = self.pending_events.pop_front().expect("front checked");
            self.head_announced = false;
            self.handle_event(event);
        }
    }

    /// The declaration display for an event, when it has one.
    fn declaration_for(&self, event: &ServerEvent) -> Option<Declaration> {
        match event {
            ServerEvent::PlayerCalled {
                player, call_type, ..
            } => {
                let label = match call_type {
                    CallType::Pon => Key::Pon,
                    CallType::Chi => Key::Chi,
                    CallType::Ankan | CallType::Daiminkan | CallType::Kakan => Key::Kan,
                    // Ron is declared via RoundWon; the server does not
                    // normally send this.
                    CallType::Ron => return None,
                };
                Some(Declaration {
                    banners: vec![(*player, label)],
                    hold_secs: CALL_HOLD_SECS,
                    before_apply: false,
                })
            }
            ServerEvent::PlayerRiichi { player, .. } => Some(Declaration {
                banners: vec![(*player, Key::Riichi)],
                hold_secs: CALL_HOLD_SECS,
                before_apply: false,
            }),
            ServerEvent::PeiDeclared { player, .. } => Some(Declaration {
                banners: vec![(*player, Key::Pei)],
                hold_secs: CALL_HOLD_SECS,
                before_apply: false,
            }),
            ServerEvent::RoundWon { .. } if self.phase == GamePhase::Playing => {
                // Multiple rons arrive as consecutive RoundWon events, so
                // show every queued winner's banner at once.
                let banners: Vec<(Wind, Key)> = self
                    .pending_events
                    .iter()
                    .filter_map(|ev| match ev {
                        ServerEvent::RoundWon { winner, loser, .. } => {
                            let label = if loser.is_some() {
                                Key::Ron
                            } else {
                                Key::Tsumo
                            };
                            Some((*winner, label))
                        }
                        _ => None,
                    })
                    .collect();
                Some(Declaration {
                    banners,
                    hold_secs: WIN_HOLD_SECS,
                    before_apply: true,
                })
            }
            ServerEvent::RoundDraw {
                declarer: Some(declarer),
                ..
            } if self.phase == GamePhase::Playing => Some(Declaration {
                banners: vec![(*declarer, Key::NineTerminals)],
                hold_secs: WIN_HOLD_SECS,
                before_apply: true,
            }),
            _ => None,
        }
    }

    /// Seat name for winners and deal-in players
    /// (Japanese 「東家」, English "East").
    fn wind_to_name(&self, wind: Wind) -> String {
        match self.lang {
            Lang::Ja => format!("{}家", wind.name(Lang::Ja)),
            Lang::En => wind.name(Lang::En).to_string(),
        }
    }

    /// Player name (e.g. "CPU2") for results, instead of the seat name.
    fn player_display_name(&self, wind: Wind) -> String {
        let rel = self.relative_player_index(wind);
        let seat = (self.my_seat + rel) % self.player_count;
        self.player_labels[seat].short_name(rel, self.lang)
    }
}
