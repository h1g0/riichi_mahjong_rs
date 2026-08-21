//! Translation from `mahjong_server::protocol::ServerEvent` into mjai events.
//!
//! [`MjaiEncoder`] consumes the event stream of a single seat and produces the
//! mjai equivalent in *in-game mode*: tiles that seat is not entitled to see
//! stay concealed. That is exactly the view an mjai bot expects, so the output
//! can be fed straight to one.
//!
//! The translation is stateful for four separate reasons, none of which are
//! avoidable from a single event:
//!
//! * mjai numbers seats absolutely (0-3, fixed for the game) while
//!   `ServerEvent` names them by seat wind, which rotates every hand. Mapping
//!   between them needs the current hand number.
//! * `DoraIndicatorsUpdated` carries the whole list; mjai announces one new
//!   indicator at a time, so the encoder tracks how many it has already sent.
//! * mjai brackets the declaring discard with `reach` and `reach_accepted`,
//!   whereas the server announces riichi once, before the discard.
//! * mjai reports score *changes*, which need the scores from before the
//!   payment.

use std::collections::VecDeque;

use mahjong_core::tile::{Tile, Wind};
use mahjong_server::protocol::{CallType, DrawReason, PlayerHandInfo, ServerEvent};

use crate::event::{Actor, MjaiEvent, RyukyokuReason};
use crate::tile::MjaiTile;
use crate::yaku::score_item_name;

/// Number of tiles in a starting hand, used to size the concealed hands of
/// opponents in `start_kyoku`.
const STARTING_HAND_LEN: usize = 13;

/// Hidden information for replay mode.
///
/// A single seat's stream cannot see the other players' starting hands or
/// draws, so replay mode supplies them from the other seats' streams. Only
/// these two things are missing: every discard, call, dora, and result is
/// already public, and the final hands ride along on the result event.
#[derive(Debug, Clone, Default)]
pub struct Reveal {
    /// Starting hand per absolute seat.
    pub tehais: Vec<Vec<Tile>>,
    /// Draws still to come, per absolute seat, in order.
    pub draws: Vec<VecDeque<Tile>>,
}

/// A riichi deposit waiting for the declaring discard to survive.
struct PendingReach {
    actor: Actor,
    deltas: Vec<i32>,
    scores: Vec<i32>,
}

/// Translates one seat's `ServerEvent` stream into mjai events.
pub struct MjaiEncoder {
    /// Absolute seat of the player this stream belongs to. Known once the
    /// first hand starts.
    self_actor: Option<Actor>,
    /// Hand number, 0-based, as reported by the server.
    round_number: usize,
    player_count: usize,
    /// How many dora indicators have already been announced this hand.
    dora_announced: usize,
    /// Scores before the current payment, in absolute seat order.
    scores: Vec<i32>,
    /// Seat of the most recent discard, which is the `target` of any call.
    last_discarder: Option<Actor>,
    /// Riichi declared but whose declaring discard has not been seen yet.
    awaiting_reach_discard: Option<PendingReach>,
    /// Riichi whose discard has been seen; `reach_accepted` is held back one
    /// event so that a ron on the declaring discard can suppress it.
    pending_reach_accepted: Option<PendingReach>,
    /// Whether the current hand has already been closed with `end_kyoku`.
    /// A double ron reports one `RoundWon` per winner, but the hand ends once.
    hand_ended: bool,
    /// A win may be followed by another win on the same discard, so closing
    /// the hand is delayed until the next non-win event.
    pending_end_kyoku: bool,
    /// Whether `start_game` has been emitted.
    game_started: bool,
    /// Player names used for `start_game`.
    names: Vec<String>,
    /// Hidden information for the current hand; `None` means in-game mode.
    reveal: Option<Reveal>,
}

impl MjaiEncoder {
    /// Creates an encoder that will announce the players under `names`.
    pub fn new(names: Vec<String>) -> Self {
        Self {
            self_actor: None,
            round_number: 0,
            player_count: 4,
            dora_announced: 0,
            scores: Vec::new(),
            last_discarder: None,
            hand_ended: false,
            pending_end_kyoku: false,
            awaiting_reach_discard: None,
            pending_reach_accepted: None,
            game_started: false,
            names,
            reveal: None,
        }
    }

    /// Supplies the hidden information for the hand about to start, switching
    /// this encoder into replay mode until the next call.
    ///
    /// Must be set before the hand's `GameStarted` is encoded, because that is
    /// where the revealed starting hands are written.
    pub fn set_reveal(&mut self, reveal: Option<Reveal>) {
        self.reveal = reveal;
    }

    /// Returns the absolute seat of the player this stream belongs to, once a
    /// hand has started.
    pub fn self_actor(&self) -> Option<Actor> {
        self.self_actor
    }

    /// Translates one server event. May produce zero, one, or several mjai
    /// events; events with no mjai counterpart produce none.
    pub fn encode(&mut self, event: &ServerEvent) -> Vec<MjaiEvent> {
        let mut out = self.release_pending_end_kyoku(event);
        out.extend(self.release_pending_reach(event));
        match event {
            ServerEvent::GameStarted { .. } => self.encode_start_kyoku(event, &mut out),
            ServerEvent::TileDrawn { tile, .. } => {
                let actor = self.expect_self();
                out.push(MjaiEvent::Tsumo {
                    actor,
                    pai: MjaiTile::Known(*tile),
                });
            }
            ServerEvent::OtherPlayerDrew { player, .. } => {
                let actor = self.actor_of(*player);
                out.push(MjaiEvent::Tsumo {
                    actor,
                    pai: self.next_revealed_draw(actor),
                });
            }
            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
                ..
            } => {
                let actor = self.actor_of(*player);
                out.push(MjaiEvent::Dahai {
                    actor,
                    pai: *tile,
                    tsumogiri: *is_tsumogiri,
                });
                self.last_discarder = Some(actor);
                if self
                    .awaiting_reach_discard
                    .as_ref()
                    .is_some_and(|reach| reach.actor == actor)
                {
                    self.pending_reach_accepted = self.awaiting_reach_discard.take();
                }
            }
            ServerEvent::PlayerCalled {
                player,
                call_type,
                called_tile,
                tiles,
            } => {
                out.push(self.encode_call(*player, call_type, *called_tile, tiles));
            }
            ServerEvent::PlayerRiichi {
                player,
                scores,
                riichi_sticks: _,
            } => {
                let actor = self.actor_of(*player);
                out.push(MjaiEvent::Reach { actor });
                let after = scores[..self.player_count].to_vec();
                let deltas = after
                    .iter()
                    .zip(self.scores.iter())
                    .map(|(new, old)| new - old)
                    .collect();
                self.awaiting_reach_discard = Some(PendingReach {
                    actor,
                    deltas,
                    scores: after,
                });
            }
            ServerEvent::DoraIndicatorsUpdated { dora_indicators } => {
                for indicator in dora_indicators.iter().skip(self.dora_announced) {
                    out.push(MjaiEvent::Dora {
                        dora_marker: *indicator,
                    });
                }
                self.dora_announced = dora_indicators.len();
            }
            ServerEvent::RoundWon { .. } => self.encode_hora(event, &mut out),
            ServerEvent::RoundDraw {
                scores,
                reason,
                tenpai,
                player_hands,
                declarer,
                ..
            } => {
                let mjai_reason = match reason {
                    DrawReason::Exhaustive => RyukyokuReason::Fanpai,
                    DrawReason::NineTerminals => RyukyokuReason::Kyushukyuhai,
                    DrawReason::FourWinds => RyukyokuReason::Sufonrenta,
                    DrawReason::FourRiichi => RyukyokuReason::Suchareach,
                    DrawReason::FourKans => RyukyokuReason::Sukaikan,
                    DrawReason::TripleRon => RyukyokuReason::Sanchaho,
                };
                out.push(self.ryukyoku(mjai_reason, scores, tenpai, player_hands, *declarer));
                self.end_kyoku(&mut out);
            }
            ServerEvent::RoundNagashiMangan {
                scores,
                player_hands,
                ..
            } => {
                out.push(self.ryukyoku(
                    RyukyokuReason::Nagashimangan,
                    scores,
                    &[],
                    player_hands,
                    None,
                ));
                self.end_kyoku(&mut out);
            }
            // No mjai counterpart. HandUpdated and CallAvailable are resync
            // and legality hints for our own client; an mjai player derives
            // both for itself. PeiDeclared is three-player only, which this
            // encoder does not claim to support.
            ServerEvent::HandUpdated { .. }
            | ServerEvent::CallAvailable { .. }
            | ServerEvent::NineTerminalsAvailable
            | ServerEvent::PeiDeclared { .. } => {}
        }
        out
    }

    /// Emits `end_game`. The server has no event for it, so the caller signals
    /// the end of the match itself.
    pub fn end_game(&self) -> Vec<MjaiEvent> {
        let mut out = Vec::new();
        if self.pending_end_kyoku {
            out.push(MjaiEvent::EndKyoku);
        }
        out.push(MjaiEvent::EndGame);
        out
    }

    fn encode_start_kyoku(&mut self, event: &ServerEvent, out: &mut Vec<MjaiEvent>) {
        let ServerEvent::GameStarted {
            seat_wind,
            hand,
            scores,
            round_wind,
            dora_indicators,
            round_number,
            honba,
            riichi_sticks,
            three_player,
            ..
        } = event
        else {
            return;
        };

        self.player_count = if *three_player { 3 } else { 4 };
        self.round_number = *round_number;
        self.dora_announced = dora_indicators.len();
        self.scores = scores.to_vec();
        self.awaiting_reach_discard = None;
        self.pending_reach_accepted = None;
        self.last_discarder = None;
        self.hand_ended = false;
        self.pending_end_kyoku = false;

        let actor = (*round_number + seat_wind.to_index()) % self.player_count;
        self.self_actor = Some(actor);

        if !self.game_started {
            self.game_started = true;
            out.push(MjaiEvent::StartGame {
                // A replay has no single recipient, so it names no seat.
                // Keeping the backbone seat's id here would claim the log was
                // recorded from that seat's point of view, which it is not.
                id: self.reveal.is_none().then_some(actor),
                names: self.names.clone(),
            });
        }

        let mut tehais = vec![vec![MjaiTile::Hidden; STARTING_HAND_LEN]; self.player_count];
        if let Some(reveal) = &self.reveal {
            for (seat, revealed) in reveal.tehais.iter().enumerate().take(self.player_count) {
                if !revealed.is_empty() {
                    tehais[seat] = revealed.iter().map(|tile| MjaiTile::Known(*tile)).collect();
                }
            }
        }
        tehais[actor] = hand.iter().map(|tile| MjaiTile::Known(*tile)).collect();

        out.push(MjaiEvent::StartKyoku {
            bakaze: *round_wind,
            kyoku: (*round_number % self.player_count) as u8 + 1,
            honba: *honba as u32,
            kyotaku: *riichi_sticks as u32,
            oya: *round_number % self.player_count,
            // The opening indicator is always present; falling back to a
            // placeholder would produce a log that silently lies, so an
            // indicator-less hand is left for the caller to notice.
            dora_marker: dora_indicators
                .first()
                .copied()
                .unwrap_or_else(|| Tile::new(Tile::M1)),
            scores: scores[..self.player_count].to_vec(),
            tehais,
        });
    }

    fn encode_call(
        &self,
        player: Wind,
        call_type: &CallType,
        called_tile: Tile,
        tiles: &[Tile],
    ) -> MjaiEvent {
        let actor = self.actor_of(player);
        // The server reports the finished meld, which includes the called
        // tile; mjai lists only what came out of the hand.
        let consumed = consumed_without(tiles, called_tile);
        // The event does not name the discarder, so the encoder remembers who
        // discarded last. Falling back to the left-hand seat would be wrong for
        // every pon and open quad taken across the table.
        let target = self
            .last_discarder
            .unwrap_or((actor + self.player_count - 1) % self.player_count);
        match call_type {
            CallType::Chi => MjaiEvent::Chi {
                actor,
                target,
                pai: called_tile,
                consumed,
            },
            CallType::Pon => MjaiEvent::Pon {
                actor,
                target,
                pai: called_tile,
                consumed,
            },
            CallType::Daiminkan => MjaiEvent::Daiminkan {
                actor,
                target,
                pai: called_tile,
                consumed,
            },
            CallType::Ankan => MjaiEvent::Ankan {
                actor,
                // A concealed quad consumes all four tiles and names none.
                consumed: tiles.to_vec(),
            },
            CallType::Kakan => MjaiEvent::Kakan {
                actor,
                pai: called_tile,
                consumed,
            },
            // Ron is announced by RoundWon, not by PlayerCalled; if one ever
            // arrives here, a pass is the only harmless thing to emit.
            CallType::Ron => MjaiEvent::Pass,
        }
    }

    fn encode_hora(&mut self, event: &ServerEvent, out: &mut Vec<MjaiEvent>) {
        let ServerEvent::RoundWon {
            winner,
            loser,
            winning_tile,
            scores,
            yaku_list,
            han,
            fu,
            score_points,
            uradora_indicators,
            player_hands,
            ..
        } = event
        else {
            return;
        };
        let actor = self.actor_of(*winner);
        out.push(MjaiEvent::Hora {
            actor,
            // On a self-draw mjai names the winner as its own target.
            target: loser.map_or(actor, |wind| self.actor_of(wind)),
            pai: *winning_tile,
            fu: Some(*fu),
            fan: Some(*han),
            yakus: Some(
                yaku_list
                    .iter()
                    .map(|(item, han)| (score_item_name(*item).to_owned(), *han))
                    .collect(),
            ),
            hora_tehais: player_hands
                .iter()
                .find(|info| info.wind == *winner)
                .map(|info| info.hand.clone()),
            // Only a riichi win turns these up, so an empty list means "none
            // revealed" rather than "not known".
            uradora_markers: (!uradora_indicators.is_empty()).then(|| uradora_indicators.clone()),
            hora_points: Some(*score_points),
            deltas: Some(self.deltas(scores)),
            scores: Some(scores[..self.player_count].to_vec()),
            pao: None,
        });
        self.scores = scores.to_vec();
        self.pending_end_kyoku = true;
    }

    /// Closes the hand, at most once.
    ///
    /// A double ron produces a `RoundWon` for each winner; mjai wants both wins
    /// but only one `end_kyoku`, so the second call here is a no-op.
    fn end_kyoku(&mut self, out: &mut Vec<MjaiEvent>) {
        if !self.hand_ended {
            self.hand_ended = true;
            self.pending_end_kyoku = false;
            out.push(MjaiEvent::EndKyoku);
        }
    }

    /// Releases a delayed hand ending before the first event that cannot be
    /// another winner on the same discard.
    fn release_pending_end_kyoku(&mut self, next: &ServerEvent) -> Vec<MjaiEvent> {
        if self.pending_end_kyoku && !matches!(next, ServerEvent::RoundWon { .. }) {
            self.pending_end_kyoku = false;
            self.hand_ended = true;
            vec![MjaiEvent::EndKyoku]
        } else {
            Vec::new()
        }
    }

    fn ryukyoku(
        &mut self,
        reason: RyukyokuReason,
        scores: &[i32],
        tenpai_winds: &[Wind],
        player_hands: &[PlayerHandInfo],
        declarer: Option<Wind>,
    ) -> MjaiEvent {
        let mut tenpais = vec![false; self.player_count];
        for wind in tenpai_winds {
            tenpais[self.actor_of(*wind)] = true;
        }
        // Four-riichi reveals all four locked hands in the reference host.
        if matches!(reason, RyukyokuReason::Suchareach) {
            tenpais.fill(true);
        }

        let mut tehais = vec![Vec::new(); self.player_count];
        for info in player_hands {
            let actor = self.actor_of(info.wind);
            let visible = self.reveal.is_some() || tenpais[actor] || declarer == Some(info.wind);
            tehais[actor] = info
                .hand
                .iter()
                .copied()
                .map(|tile| {
                    if visible {
                        MjaiTile::Known(tile)
                    } else {
                        MjaiTile::Hidden
                    }
                })
                .collect();
        }
        let has_hands = !player_hands.is_empty();
        let event = MjaiEvent::Ryukyoku {
            reason,
            actor: declarer.map(|wind| self.actor_of(wind)),
            tenpais: has_hands.then_some(tenpais),
            tehais: has_hands.then_some(tehais),
            deltas: Some(self.deltas(scores)),
            scores: Some(scores[..self.player_count].to_vec()),
        };
        self.scores = scores.to_vec();
        event
    }

    /// Emits a withheld `reach_accepted`, unless `next` shows the declaring
    /// discard was ronned — in which case the declaration never stood.
    fn release_pending_reach(&mut self, next: &ServerEvent) -> Vec<MjaiEvent> {
        let Some(reach) = self.pending_reach_accepted.take() else {
            return Vec::new();
        };
        if let ServerEvent::RoundWon {
            loser: Some(loser), ..
        } = next
            && self.actor_of(*loser) == reach.actor
        {
            return Vec::new();
        }
        self.scores = reach.scores.clone();
        vec![MjaiEvent::ReachAccepted {
            actor: reach.actor,
            deltas: Some(reach.deltas),
            scores: Some(reach.scores),
        }]
    }

    /// Maps a seat wind to its absolute seat.
    ///
    /// The dealer of hand `n` is always seat `n % player_count`, and seat winds
    /// are assigned outward from the dealer, so the two differ by the hand
    /// number. A dealer repeat leaves the hand number alone, which is why this
    /// stays correct across renchan.
    fn actor_of(&self, wind: Wind) -> Actor {
        (self.round_number + wind.to_index()) % self.player_count
    }

    /// Returns the next draw of `actor`, revealed if replay mode supplied it.
    ///
    /// Running out mid-hand conceals the rest rather than panicking: a partial
    /// recording should degrade to an in-game-mode log, not lose the game.
    fn next_revealed_draw(&mut self, actor: Actor) -> MjaiTile {
        self.reveal
            .as_mut()
            .and_then(|reveal| reveal.draws.get_mut(actor))
            .and_then(|queue| queue.pop_front())
            .map_or(MjaiTile::Hidden, MjaiTile::Known)
    }

    fn expect_self(&self) -> Actor {
        self.self_actor.unwrap_or(0)
    }

    /// Score changes per seat.
    ///
    /// `ServerEvent` scores are indexed by seat, not by seat wind, and this
    /// project's seat numbering already matches mjai's: seat `n % player_count`
    /// deals hand `n`, which is exactly how mjai numbers its actors. So no
    /// reordering is needed in either direction — only the subtraction.
    fn deltas(&self, after: &[i32]) -> Vec<i32> {
        after
            .iter()
            .zip(self.scores.iter())
            .take(self.player_count)
            .map(|(new, old)| new - old)
            .collect()
    }
}

/// Removes one instance of `called` from `tiles`.
///
/// Prefers an exact match so a red five is not swapped for a plain one, then
/// falls back to the same tile kind: a promoted quad reports a plain tile as
/// the called tile even when the tile added to the triplet is the red five.
fn consumed_without(tiles: &[Tile], called: Tile) -> Vec<Tile> {
    let mut rest = tiles.to_vec();
    let found = rest
        .iter()
        .position(|tile| *tile == called)
        .or_else(|| rest.iter().position(|tile| tile.get() == called.get()));
    if let Some(index) = found {
        rest.remove(index);
    }
    rest
}
