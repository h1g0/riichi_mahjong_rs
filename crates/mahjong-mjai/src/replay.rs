//! Replaying an imported game log through this engine.
//!
//! # The import path
//!
//! There is deliberately no Tenhou or Mahjong Soul parser here. Converters
//! from both formats into mjai already exist and are maintained by people who
//! track those sites' quirks, so a log arrives as
//!
//! ```text
//! Tenhou XML / Mahjong Soul record --(external converter)--> mjai --> MjaiReplay
//! ```
//!
//! and this crate maintains one importer instead of one per site.
//! `tenhou-to-mjai`, `mjai-gateway`, and the converter shipped with gimite/mjai
//! all produce the input this module reads.
//!
//! # What a replay is for
//!
//! Two things, both of which need every seat rather than one:
//!
//! - **Driving the engine's consumers.** [`MjaiReplay::feed`] hands back the
//!   `ServerEvent` stream each seat would have received, which is what
//!   `CpuGameState` and the client already fold over. Nothing deals a wall:
//!   the log *is* the wall.
//! - **Auditing this project against real games.** Every win in the log is
//!   scored again from the reconstructed hand and compared with the score the
//!   log reports, and every exhaustive draw's ready declarations are compared
//!   with our own shanten. A disagreement is a bug here, not there.
//!
//! # Reconstructing four seats
//!
//! [`MjaiDecoder`] rebuilds one seat. A replay-mode log conceals nothing, so
//! four decoders fed the same stream rebuild all four seats, each maintaining
//! its own `Player` exactly as the server would. No new state machine is
//! needed, and the reconstruction is exercised by the same tests as live play.
//!
//! An in-game-mode log — one addressed to a single seat, with `"?"` where that
//! seat cannot see — cannot be replayed this way, and hands from such a log are
//! counted as skipped rather than checked.
//!
//! # What cannot be compared
//!
//! mjai names yaku many-to-one (three dragon triplets all become `sangenpai`),
//! so the log's yaku list cannot be turned back into exact `Kind` values.
//! Comparison is therefore on han, minipoints (fu), and the point transfers,
//! never on yaku identity.

use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, Wind};
use mahjong_server::player::Player;
use mahjong_server::protocol::ServerEvent;
use mahjong_server::scoring;

use crate::decode::MjaiDecoder;
use crate::event::{Actor, MjaiEvent, RyukyokuReason};
use crate::tile::MjaiTile;
use crate::yaku::score_item_name;

/// Seats in a four-player game. Three-player logs are not replayed; mjai's
/// three-player dialect is a separate, weakly standardised thing.
const SEATS: usize = 4;

/// Value of one riichi deposit.
const RIICHI_STICK_VALUE: i32 = 1000;

/// The site a log came from, which fixes the rules it was played under.
///
/// mjai carries no rule set, so an importer has to be told which table it is
/// reading: red fives, open All Inside, and the limit-hand rules all change the
/// score a hand should have. These presets cover the flags that affect scoring;
/// everything else keeps this project's defaults, and anything unusual is
/// better expressed by editing the returned [`Settings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    /// Tenhou (天鳳), red fives on. Tenhou pays no double yakuman and does not
    /// round 3 han 60 fu up to a limit hand.
    Tenhou,
    /// Mahjong Soul (雀魂), red fives on. Unlike Tenhou it does pay double
    /// yakuman, and likewise does not round up to a limit hand.
    MahjongSoul,
}

impl LogSource {
    /// Returns the rules to score this source's logs under.
    pub fn settings(self) -> Settings {
        let base = Settings {
            red_fives: true,
            opened_all_inside: true,
            kiriage_mangan: false,
            counted_yakuman: true,
            nagashi_mangan: true,
            ..Settings::new()
        };
        match self {
            LogSource::Tenhou => Settings {
                double_yakuman: false,
                ..base
            },
            LogSource::MahjongSoul => Settings {
                double_yakuman: true,
                ..base
            },
        }
    }
}

/// Identifies one hand of a game, for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandId {
    /// Round wind (bakaze / 場風)
    pub round_wind: Wind,
    /// Hand number within the round wind, 1-based (East 1 = 1)
    pub kyoku: usize,
    /// Continuance counter (honba / 本場)
    pub honba: usize,
}

impl std::fmt::Display for HandId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let wind = match self.round_wind {
            Wind::East => "E",
            Wind::South => "S",
            Wind::West => "W",
            Wind::North => "N",
        };
        write!(f, "{wind}{}-{}", self.kyoku, self.honba)
    }
}

/// One disagreement between the log and this engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingKind {
    /// The log says this seat won and we say the hand does not win at all.
    NotAWin { actor: Actor },
    /// Han differ. `yaku` is our own award list, in mjai's names.
    Han {
        actor: Actor,
        logged: u32,
        ours: u32,
        yaku: Vec<(String, u32)>,
    },
    /// Minipoints (fu / 符) differ.
    Fu {
        actor: Actor,
        logged: u32,
        ours: u32,
    },
    /// The point transfers differ, in absolute seat order.
    Deltas {
        actor: Actor,
        logged: Vec<i32>,
        ours: Vec<i32>,
    },
    /// The winner's total differs, and the log carried no per-seat transfers
    /// to be more specific with.
    HoraPoints {
        actor: Actor,
        logged: i32,
        ours: i32,
    },
    /// The hand we rebuilt for a seat is not the hand the log reveals.
    Hand {
        actor: Actor,
        logged: Vec<Tile>,
        ours: Vec<Tile>,
    },
    /// The log and our shanten disagree on whether a seat was ready (tenpai).
    Ready {
        actor: Actor,
        logged: bool,
        ours: bool,
    },
}

impl FindingKind {
    /// The seat the finding is about.
    pub fn actor(&self) -> Actor {
        match self {
            FindingKind::NotAWin { actor }
            | FindingKind::Han { actor, .. }
            | FindingKind::Fu { actor, .. }
            | FindingKind::Deltas { actor, .. }
            | FindingKind::HoraPoints { actor, .. }
            | FindingKind::Hand { actor, .. }
            | FindingKind::Ready { actor, .. } => *actor,
        }
    }
}

impl std::fmt::Display for FindingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingKind::NotAWin { actor } => {
                write!(f, "seat {actor}: log records a win, we judge no win")
            }
            FindingKind::Han {
                actor,
                logged,
                ours,
                yaku,
            } => {
                let awarded = yaku
                    .iter()
                    .map(|(name, han)| format!("{name} {han}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(
                    f,
                    "seat {actor}: han {logged} in log, {ours} here [{awarded}]"
                )
            }
            FindingKind::Fu {
                actor,
                logged,
                ours,
            } => write!(f, "seat {actor}: fu {logged} in log, {ours} here"),
            FindingKind::Deltas {
                actor,
                logged,
                ours,
            } => write!(
                f,
                "seat {actor}: transfers {logged:?} in log, {ours:?} here"
            ),
            FindingKind::HoraPoints {
                actor,
                logged,
                ours,
            } => write!(f, "seat {actor}: winner takes {logged} in log, {ours} here"),
            FindingKind::Hand {
                actor,
                logged,
                ours,
            } => write!(
                f,
                "seat {actor}: log reveals [{}], we rebuilt [{}]",
                tiles_to_string(logged),
                tiles_to_string(ours),
            ),
            FindingKind::Ready {
                actor,
                logged,
                ours,
            } => write!(
                f,
                "seat {actor}: log says ready={logged}, our shanten says {ours}"
            ),
        }
    }
}

/// A finding together with the hand it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The hand in progress when the disagreement was found
    pub hand: HandId,
    /// What disagreed
    pub kind: FindingKind,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.hand, self.kind)
    }
}

/// What a replay found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayReport {
    /// Hands the log contains
    pub hands: usize,
    /// Hands that could not be audited, because the log conceals tiles or is
    /// not a four-player game
    pub hands_skipped: usize,
    /// Wins in the log
    pub wins: usize,
    /// Wins whose score was recomputed and compared. A win in a log that
    /// carries no score breakdown cannot be compared with anything.
    pub wins_checked: usize,
    /// Exhaustive draws in the log
    pub draws: usize,
    /// Exhaustive draws whose ready declarations were compared
    pub draws_checked: usize,
    /// Every disagreement found, in the order they were found
    pub findings: Vec<Finding>,
}

impl ReplayReport {
    /// Whether the log and this engine agreed everywhere they were compared.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

impl std::fmt::Display for ReplayReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} hands ({} skipped), {}/{} wins checked, {}/{} draws checked, {} {}",
            self.hands,
            self.hands_skipped,
            self.wins_checked,
            self.wins,
            self.draws_checked,
            self.draws,
            self.findings.len(),
            if self.findings.len() == 1 {
                "finding"
            } else {
                "findings"
            }
        )
    }
}

/// Replays an mjai log, rebuilding every seat and auditing every result.
pub struct MjaiReplay {
    /// One decoder per seat; together they hold the whole table.
    seats: Vec<MjaiDecoder>,
    settings: Settings,
    report: ReplayReport,
    /// Whether the hand in progress is fully revealed, and so auditable.
    hand_revealed: bool,
    /// Wins already paid this hand, which decides who takes the continuance
    /// bonus and the deposits in a multiple ron.
    wins_this_hand: usize,
    /// Seat whose quad promotion is still open to being robbed.
    open_kakan: Option<Actor>,
    /// Seat whose riichi has been declared but not yet accepted. mjai only
    /// moves the deposit at acceptance, so an unaccepted one is money the log
    /// has not shown leaving anyone's score.
    pending_reach: Option<Actor>,
}

impl Default for MjaiReplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MjaiReplay {
    /// Creates a replay under this project's default rules.
    pub fn new() -> Self {
        Self::with_settings(Settings::default())
    }

    /// Creates a replay for a log from `source`.
    pub fn from_source(source: LogSource) -> Self {
        Self::with_settings(source.settings())
    }

    /// Creates a replay under an explicit rule set.
    pub fn with_settings(settings: Settings) -> Self {
        Self {
            seats: (0..SEATS)
                .map(|seat| MjaiDecoder::with_settings(seat, settings.clone()))
                .collect(),
            settings,
            report: ReplayReport::default(),
            hand_revealed: false,
            wins_this_hand: 0,
            open_kakan: None,
            pending_reach: None,
        }
    }

    /// What has been found so far.
    pub fn report(&self) -> &ReplayReport {
        &self.report
    }

    /// One seat as currently reconstructed.
    pub fn player(&self, seat: Actor) -> Option<&Player> {
        self.seats.get(seat).map(|decoder| decoder.player())
    }

    /// The hand in progress.
    pub fn hand_id(&self) -> HandId {
        let decoder = &self.seats[0];
        HandId {
            round_wind: decoder.round_wind(),
            kyoku: decoder.round_number() % SEATS + 1,
            honba: decoder.honba(),
        }
    }

    /// Feeds one event and returns the `ServerEvent`s each seat receives,
    /// indexed by seat.
    ///
    /// Auditing happens here too: a result event is compared with this
    /// engine's own judgement before it is applied.
    pub fn feed(&mut self, event: &MjaiEvent) -> Vec<Vec<ServerEvent>> {
        self.audit(event);
        let out = self
            .seats
            .iter_mut()
            .map(|decoder| decoder.decode(event))
            .collect();
        self.open_kakan = match event {
            MjaiEvent::Kakan { actor, .. } => Some(*actor),
            _ => None,
        };
        out
    }

    /// Feeds a whole log, discarding the per-seat events.
    ///
    /// This is the auditing entry point: use [`MjaiReplay::feed`] instead when
    /// the events themselves are wanted.
    pub fn run(&mut self, log: &[MjaiEvent]) -> &ReplayReport {
        for event in log {
            self.feed(event);
        }
        &self.report
    }

    /// Compares one result event with this engine's own judgement.
    fn audit(&mut self, event: &MjaiEvent) {
        match event {
            MjaiEvent::StartKyoku { scores, tehais, .. } => {
                self.report.hands += 1;
                self.wins_this_hand = 0;
                self.pending_reach = None;
                self.hand_revealed = scores.len() == SEATS && fully_revealed(tehais);
                if !self.hand_revealed {
                    self.report.hands_skipped += 1;
                }
            }
            MjaiEvent::Reach { actor } => self.pending_reach = Some(*actor),
            MjaiEvent::ReachAccepted { actor, .. } => {
                if self.pending_reach == Some(*actor) {
                    self.pending_reach = None;
                }
            }
            MjaiEvent::Hora { .. } => self.audit_hora(event),
            MjaiEvent::Ryukyoku { .. } => self.audit_ryukyoku(event),
            _ => {}
        }
    }

    fn audit_hora(&mut self, event: &MjaiEvent) {
        let MjaiEvent::Hora {
            actor,
            target,
            pai,
            fu,
            fan,
            hora_tehais,
            uradora_markers,
            hora_points,
            deltas,
            pao,
            ..
        } = event
        else {
            return;
        };
        self.report.wins += 1;
        let rank_of_win = self.wins_this_hand;
        self.wins_this_hand += 1;

        if !self.hand_revealed || *actor >= SEATS || *target >= SEATS {
            return;
        }

        let hand = self.hand_id();
        if let Some(logged) = hora_tehais {
            self.compare_hand(hand, *actor, logged, Some(*pai));
        }

        // A log without a score breakdown (in-game mode) states no claim to
        // check: the winner is told only that it won.
        let (Some(logged_fu), Some(logged_han)) = (fu, fan) else {
            return;
        };
        self.report.wins_checked += 1;

        let is_self_draw = actor == target;
        let decoder = &self.seats[*actor];
        let is_last_tile = decoder.remaining_tiles() == 0 && !decoder.last_draw_was_dead_wall();
        let is_robbing_a_quad = !is_self_draw && self.open_kakan == Some(*target);

        let win = if is_self_draw {
            scoring::check_win_with_settings(
                decoder.player(),
                decoder.round_wind(),
                true,
                is_last_tile,
                decoder.last_draw_was_dead_wall(),
                &self.settings,
            )
        } else {
            scoring::check_ron_with_flags_and_settings(
                decoder.player(),
                *pai,
                decoder.round_wind(),
                is_last_tile,
                is_robbing_a_quad,
                &self.settings,
            )
        };
        let Some(mut score) = win.score_result.filter(|_| win.is_win) else {
            self.report.findings.push(Finding {
                hand,
                kind: FindingKind::NotAWin { actor: *actor },
            });
            return;
        };

        let player = decoder.player();
        let uradora: &[Tile] = if player.is_riichi {
            uradora_markers.as_deref().unwrap_or_default()
        } else {
            &[]
        };
        scoring::add_dora_to_score(
            &mut score,
            &player.hand,
            (!is_self_draw).then_some(*pai),
            decoder.dora_indicators(),
            uradora,
            &player.pei_tiles,
            &self.settings,
        );

        if score.han != *logged_han {
            self.report.findings.push(Finding {
                hand,
                kind: FindingKind::Han {
                    actor: *actor,
                    logged: *logged_han,
                    ours: score.han,
                    yaku: score
                        .yaku_list
                        .iter()
                        .map(|(item, han)| (score_item_name(*item).to_owned(), *han))
                        .collect(),
                },
            });
        }
        if score.fu != *logged_fu {
            self.report.findings.push(Finding {
                hand,
                kind: FindingKind::Fu {
                    actor: *actor,
                    logged: *logged_fu,
                    ours: score.fu,
                },
            });
        }

        // The continuance bonus and the deposits go to the first winner only,
        // so a second winner of a double ron is paid the bare hand.
        let is_first_win = rank_of_win == 0;
        let honba = if is_first_win { decoder.honba() } else { 0 };
        let deposits = if is_first_win {
            decoder.riichi_sticks() as i32 * RIICHI_STICK_VALUE
        } else {
            0
        };
        let dealer = decoder.round_number() % SEATS;
        let winner_is_dealer = *actor == dealer;
        // Liability (pao / 包) is not derivable from an mjai stream, which
        // never says which call locked in the yakuman, so the log's own
        // assignment is taken at face value.
        let pao_players: Vec<usize> = pao.iter().copied().filter(|seat| *seat < SEATS).collect();

        let mut ours = if is_self_draw {
            scoring::calculate_tsumo_score_deltas_with_pao(
                *actor,
                &score,
                winner_is_dealer,
                dealer,
                honba,
                SEATS,
                self.settings.tsumo_loss,
                &pao_players,
            )
        } else {
            scoring::calculate_ron_score_deltas_with_pao_players(
                *actor,
                *target,
                &pao_players,
                &score,
                winner_is_dealer,
                honba,
            )
        };
        ours[*actor] += deposits;
        // A host suppresses `reach_accepted` when the declaring discard is
        // ronned, so mjai never showed that deposit being placed — but the
        // round took it at declaration and pays it to the winner all the same.
        // It therefore has to leave the declarer's score here instead.
        if is_first_win && !is_self_draw && self.pending_reach == Some(*target) {
            ours[*target] -= RIICHI_STICK_VALUE;
        }

        match deltas {
            Some(logged) if logged.len() == SEATS => {
                if logged.as_slice() != ours.as_slice() {
                    self.report.findings.push(Finding {
                        hand,
                        kind: FindingKind::Deltas {
                            actor: *actor,
                            logged: logged.clone(),
                            ours: ours.to_vec(),
                        },
                    });
                }
            }
            // Without per-seat transfers there is only the winner's total, and
            // implementations disagree on whether that total includes the
            // continuance bonus and the deposits. Either reading is accepted.
            _ => {
                if let Some(logged) = hora_points {
                    // 100 from each of the three opponents on a self-draw, or
                    // 300 from the discarder, which come to the same total.
                    let honba_bonus = honba as i32 * 300;
                    let total = ours[*actor];
                    let bare = total - honba_bonus - deposits;
                    if *logged != total && *logged != bare {
                        self.report.findings.push(Finding {
                            hand,
                            kind: FindingKind::HoraPoints {
                                actor: *actor,
                                logged: *logged,
                                ours: total,
                            },
                        });
                    }
                }
            }
        }
    }

    fn audit_ryukyoku(&mut self, event: &MjaiEvent) {
        let MjaiEvent::Ryukyoku {
            reason,
            tenpais,
            tehais,
            ..
        } = event
        else {
            return;
        };
        // Only an exhaustive draw settles up on readiness; an abortive one ends
        // the hand before anyone is asked, and Nagashi Mangan pays regardless.
        if !matches!(reason, RyukyokuReason::Fanpai) {
            return;
        }
        self.report.draws += 1;
        if !self.hand_revealed {
            return;
        }
        let hand = self.hand_id();

        for (actor, slots) in tehais.iter().flatten().enumerate().take(SEATS) {
            let logged: Vec<Tile> = slots.iter().filter_map(|slot| slot.known()).collect();
            // A log that reveals only the ready hands leaves the rest empty.
            if logged.is_empty() || logged.len() != slots.len() {
                continue;
            }
            self.compare_hand(hand, actor, &logged, None);
        }

        let Some(tenpais) = tenpais else {
            return;
        };
        if tenpais.len() != SEATS {
            return;
        }
        self.report.draws_checked += 1;
        for (actor, logged) in tenpais.iter().enumerate() {
            let ours = scoring::is_ready(self.seats[actor].player());
            if ours != *logged {
                self.report.findings.push(Finding {
                    hand,
                    kind: FindingKind::Ready {
                        actor,
                        logged: *logged,
                        ours,
                    },
                });
            }
        }
    }

    /// Compares a revealed concealed hand with the one we rebuilt.
    ///
    /// Implementations differ over whether a winner's revealed hand includes
    /// the winning tile, so both readings are accepted rather than reported.
    fn compare_hand(&mut self, hand: HandId, actor: Actor, logged: &[Tile], winning: Option<Tile>) {
        let mut ours = self.seats[actor].player().hand.tiles().to_vec();
        ours.sort();
        let mut with_winning = ours.clone();
        if let Some(tile) = winning {
            with_winning.push(tile);
            with_winning.sort();
        }
        let mut logged_sorted = logged.to_vec();
        logged_sorted.sort();

        if logged_sorted != ours && logged_sorted != with_winning {
            self.report.findings.push(Finding {
                hand,
                kind: FindingKind::Hand {
                    actor,
                    logged: logged_sorted,
                    ours,
                },
            });
        }
    }
}

/// Whether every seat's starting hand is spelled out, which is what separates
/// a replay log from one addressed to a single seat.
fn fully_revealed(tehais: &[Vec<MjaiTile>]) -> bool {
    tehais.len() == SEATS
        && tehais
            .iter()
            .all(|hand| hand.len() == 13 && hand.iter().all(|slot| slot.known().is_some()))
}

/// Renders tiles in mjai notation, for a finding a human has to read.
fn tiles_to_string(tiles: &[Tile]) -> String {
    tiles
        .iter()
        .map(|tile| crate::tile_to_str(*tile))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Replays a log under `settings` and returns what it found.
///
/// The shortest way in: for anything that needs the reconstructed seats or the
/// per-seat `ServerEvent` streams, drive [`MjaiReplay`] directly.
pub fn audit_log(log: &[MjaiEvent], settings: Settings) -> ReplayReport {
    let mut replay = MjaiReplay::with_settings(settings);
    replay.run(log);
    replay.report
}
