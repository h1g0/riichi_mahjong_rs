//! Translation from mjai events into `mahjong_server::protocol::ServerEvent`.
//!
//! This is the direction that lets our own CPU opponent run as an mjai bot:
//! the decoder rebuilds the `ServerEvent` stream the CPU already knows how to
//! consume, and [`MjaiDecoder::encode_action`] renders its `ClientAction` back
//! out as mjai.
//!
//! # Working out what is legal
//!
//! mjai carries the moves that were *made*, never the moves that were
//! *available*: there is no `can_riichi`, no furiten flag, and no enumeration
//! of the calls a discard opens up. An mjai player is expected to derive all of
//! that itself.
//!
//! Rather than restate those rules, the decoder maintains a real [`Player`] for
//! its own seat — drawing, discarding, and melding it exactly as the server
//! would — and then asks [`mahjong_server::legality`] the same questions the
//! server asks. The rules therefore keep exactly one definition, shared with
//! live play.
//!
//! # Limits
//!
//! Two furiten rules depend on choices rather than on the visible stream:
//! temporary furiten from declining a ron, and the permanent furiten a player
//! in riichi takes on for the same reason. Neither is observable in an mjai
//! log, so a decoder fed only a log reports hand-based furiten only. A bot that
//! declines a ron should say so through [`MjaiDecoder::declined_ron`].
//!
//! # What does not survive the trip
//!
//! mjai collapses several yaku onto one name, so decoding picks a documented
//! representative and the original distinction is gone (see [`crate::yaku`]).
//! A hand's rank, openness, and continuance payment are not on the wire and so
//! are recomputed or defaulted.

use mahjong_core::hand_info::meld::MeldFrom;
use mahjong_core::scoring::score::determine_rank;
use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, Wind};
use mahjong_server::legality::{self, TableContext};
use mahjong_server::player::Player;
use mahjong_server::protocol::{CallType, ClientAction, DrawReason, PlayerHandInfo, ServerEvent};

use crate::MjaiTile;
use crate::event::{Actor, MjaiEvent, RyukyokuReason};
use crate::yaku::score_item_from_name;

/// Live tiles left once the dead wall is set aside and the hands are dealt.
const LIVE_WALL_AT_DEAL: usize = 70;

/// Hands in a full game. mjai does not say how long the match is, so this is
/// assumed rather than known; it only affects `total_rounds` on `GameStarted`.
const ASSUMED_TOTAL_ROUNDS: usize = 8;

/// Rebuilds one seat's `ServerEvent` stream from mjai events.
pub struct MjaiDecoder {
    self_actor: Actor,
    player_count: usize,
    round_number: usize,
    round_wind: Wind,
    honba: usize,
    riichi_sticks: usize,
    /// Scores in absolute seat order.
    scores: Vec<i32>,
    dora_indicators: Vec<Tile>,
    /// This seat, maintained exactly as the server maintains its own players so
    /// that the shared legality rules apply unchanged.
    player: Player,
    settings: Settings,
    remaining_tiles: usize,
    /// Quads declared so far by everyone, which caps further quads.
    total_kan_count: usize,
    /// Whether the last draw was a quad replacement, which suppresses haitei.
    last_draw_was_dead_wall: bool,
    /// Set when a quad is declared, so the draw that follows is known to come
    /// from the dead wall.
    kan_replacement_due: Option<Actor>,
    /// The most recent discard and who made it, which a call has to name.
    last_discard: Option<(Tile, Actor)>,
    /// Seat whose riichi declaration is still waiting for its discard.
    reach_pending: Option<Actor>,
}

impl MjaiDecoder {
    /// Creates a decoder for the seat `self_actor` under the default rules.
    pub fn new(self_actor: Actor) -> Self {
        Self::with_settings(self_actor, Settings::default())
    }

    /// Creates a decoder using an explicit rule set.
    ///
    /// mjai does not carry the rules in force, so anything that depends on them
    /// — swap calling, three-player — has to be supplied by whoever knows which
    /// table this is.
    pub fn with_settings(self_actor: Actor, settings: Settings) -> Self {
        Self {
            self_actor,
            player_count: if settings.three_player { 3 } else { 4 },
            round_number: 0,
            round_wind: Wind::East,
            honba: 0,
            riichi_sticks: 0,
            scores: vec![25000; 4],
            dora_indicators: Vec::new(),
            player: Player::new(Wind::East, Vec::new(), 25000),
            settings,
            remaining_tiles: LIVE_WALL_AT_DEAL,
            total_kan_count: 0,
            last_draw_was_dead_wall: false,
            kan_replacement_due: None,
            last_discard: None,
            reach_pending: None,
        }
    }

    /// This seat as currently reconstructed.
    pub fn player(&self) -> &Player {
        &self.player
    }

    /// This seat's concealed hand, excluding any drawn tile.
    pub fn hand(&self) -> &[Tile] {
        self.player.hand.tiles()
    }

    /// The tile drawn this turn, if the seat is holding one.
    pub fn last_drawn(&self) -> Option<Tile> {
        self.player.hand.drawn()
    }

    /// The rules this decoder was told the table is using.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Round wind (bakaze / 場風) of the hand in progress.
    pub fn round_wind(&self) -> Wind {
        self.round_wind
    }

    /// Hand number, 0-based, counting from the first hand of the game.
    pub fn round_number(&self) -> usize {
        self.round_number
    }

    /// Continuance counter (honba / 本場) of the hand in progress.
    pub fn honba(&self) -> usize {
        self.honba
    }

    /// Riichi deposits on the table (kyotaku / 供託).
    pub fn riichi_sticks(&self) -> usize {
        self.riichi_sticks
    }

    /// Every seat's score, in absolute seat order.
    pub fn scores(&self) -> &[i32] {
        &self.scores
    }

    /// Dora indicators turned up so far this hand.
    pub fn dora_indicators(&self) -> &[Tile] {
        &self.dora_indicators
    }

    /// Tiles left in the live wall.
    pub fn remaining_tiles(&self) -> usize {
        self.remaining_tiles
    }

    /// Whether the most recent draw was a quad replacement, which decides
    /// between the last-tile yaku and the after-a-quad one.
    pub fn last_draw_was_dead_wall(&self) -> bool {
        self.last_draw_was_dead_wall
    }

    /// Maps an absolute seat to its seat wind for the hand in progress.
    pub fn seat_wind_of(&self, actor: Actor) -> Wind {
        self.wind_of(actor)
    }

    /// Records that this seat passed on a ron it could have declared.
    ///
    /// Declining is a choice, not something the log shows, so the resulting
    /// furiten has to be reported by whoever made the choice. Without it the
    /// decoder keeps offering a ron the rules no longer allow.
    pub fn declined_ron(&mut self) {
        if self.player.is_riichi {
            self.player.is_riichi_furiten = true;
        } else {
            self.player.is_temporary_furiten = true;
        }
    }

    /// The table facts the legality rules need.
    fn table_context(&self) -> TableContext<'_> {
        TableContext {
            round_wind: self.round_wind,
            settings: &self.settings,
            wall_remaining: self.remaining_tiles,
            last_draw_was_dead_wall: self.last_draw_was_dead_wall,
            total_kan_count: self.total_kan_count,
        }
    }

    /// Translates one mjai event. Events with no server counterpart produce
    /// nothing.
    pub fn decode(&mut self, event: &MjaiEvent) -> Vec<ServerEvent> {
        if self.self_actor >= self.player_count || event.validate().is_err() {
            return Vec::new();
        }
        match event {
            MjaiEvent::StartKyoku { .. } => self.decode_start_kyoku(event),
            MjaiEvent::Tsumo { actor, pai } => self.decode_tsumo(*actor, *pai),
            MjaiEvent::Dahai {
                actor,
                pai,
                tsumogiri,
            } => self.decode_dahai(*actor, *pai, *tsumogiri),

            MjaiEvent::Chi {
                actor,
                target,
                pai,
                consumed,
            } => self.decode_call(*actor, *target, CallType::Chi, *pai, consumed),
            MjaiEvent::Pon {
                actor,
                target,
                pai,
                consumed,
            } => self.decode_call(*actor, *target, CallType::Pon, *pai, consumed),
            MjaiEvent::Daiminkan {
                actor,
                target,
                pai,
                consumed,
            } => self.decode_call(*actor, *target, CallType::Daiminkan, *pai, consumed),
            MjaiEvent::Kakan {
                actor,
                pai,
                consumed,
            } => self.decode_call(*actor, *actor, CallType::Kakan, *pai, consumed),
            MjaiEvent::Ankan { actor, consumed } => {
                // A concealed quad names no called tile, so it is rebuilt from
                // the kind; the server uses it only to identify the quad.
                let called = consumed
                    .first()
                    .map_or(Tile::new(Tile::M1), |tile| Tile::new(tile.get()));
                self.decode_call(*actor, *actor, CallType::Ankan, called, consumed)
            }

            MjaiEvent::Dora { dora_marker } => {
                self.dora_indicators.push(*dora_marker);
                vec![ServerEvent::DoraIndicatorsUpdated {
                    dora_indicators: self.dora_indicators.clone(),
                }]
            }

            MjaiEvent::Reach { actor } => self.decode_reach(*actor),
            MjaiEvent::ReachAccepted { actor, scores, .. } => {
                self.decode_reach_accepted(*actor, scores)
            }
            MjaiEvent::Hora { .. } => self.decode_hora(event),
            MjaiEvent::Ryukyoku { .. } => self.decode_ryukyoku(event),

            // The remaining protocol events carry no server-side game state.
            MjaiEvent::StartGame { .. }
            | MjaiEvent::EndKyoku
            | MjaiEvent::EndGame
            | MjaiEvent::Hello { .. }
            | MjaiEvent::Join { .. }
            | MjaiEvent::Pass => Vec::new(),
        }
    }

    /// Renders this seat's action as the mjai event that declares it.
    ///
    /// Returns `None` for an action mjai cannot express from the action alone.
    pub fn encode_action(&self, action: &ClientAction) -> Option<MjaiEvent> {
        let actor = self.self_actor;
        match action {
            ClientAction::Discard { tile } | ClientAction::Riichi { tile } => {
                // mjai always names the tile, even for a tsumogiri, so a
                // discard of "the tile I just drew" needs the tracked draw.
                let pai = tile.or_else(|| self.last_drawn())?;
                Some(MjaiEvent::Dahai {
                    actor,
                    pai,
                    tsumogiri: tile.is_none(),
                })
            }
            ClientAction::Tsumo => Some(self.hora(actor, self.last_drawn()?)),
            ClientAction::Pass => Some(MjaiEvent::Pass),
            ClientAction::Ron => {
                let (tile, target) = self.last_discard?;
                Some(self.hora(target, tile))
            }
            ClientAction::Pon { tiles } => {
                let (tile, target) = self.last_discard?;
                Some(MjaiEvent::Pon {
                    actor,
                    target,
                    pai: tile,
                    consumed: tiles.to_vec(),
                })
            }
            ClientAction::Chi { tiles } => {
                let (tile, target) = self.last_discard?;
                Some(MjaiEvent::Chi {
                    actor,
                    target,
                    pai: tile,
                    consumed: tiles.to_vec(),
                })
            }
            ClientAction::Kan { tile_index } => self.encode_kan(*tile_index as u32),
            // Three-player only. mjai's four-player vocabulary has no North
            // extraction, so there is nothing honest to send.
            ClientAction::Pei => None,
            ClientAction::NineTerminals { declare } => Some(if *declare {
                MjaiEvent::Ryukyoku {
                    reason: RyukyokuReason::Kyushukyuhai,
                    actor: Some(actor),
                    tenpais: None,
                    tehais: None,
                    deltas: None,
                    scores: None,
                }
            } else {
                MjaiEvent::Pass
            }),
        }
    }

    /// Renders a quad declaration.
    ///
    /// `ClientAction::Kan` names only a tile kind and covers all three quads,
    /// so which one it is has to be read back off the hand: a claim on the
    /// discard just made is a called quad, four in hand is a concealed quad,
    /// and an existing triplet of the kind is a promotion.
    fn encode_kan(&self, kind: mahjong_core::tile::TileType) -> Option<MjaiEvent> {
        let actor = self.self_actor;

        if let Some((tile, target)) = self.last_discard
            && tile.get() == kind
            && self.player.can_daiminkan(tile)
        {
            return Some(MjaiEvent::Daiminkan {
                actor,
                target,
                pai: tile,
                consumed: self.tiles_of_kind(kind, 3),
            });
        }

        if self.player.ankan_options().contains(&kind) {
            return Some(MjaiEvent::Ankan {
                actor,
                consumed: self.tiles_of_kind(kind, 4),
            });
        }

        if self.player.kakan_options().contains(&kind) {
            let added = self.player.kakan_added_tile(kind)?;
            let consumed = self
                .player
                .hand
                .melds()
                .iter()
                .find(|meld| meld.tiles.first().is_some_and(|tile| tile.get() == kind))
                .map(|meld| meld.tiles.clone())?;
            return Some(MjaiEvent::Kakan {
                actor,
                pai: added,
                consumed,
            });
        }

        None
    }

    /// Returns up to `count` held tiles of `kind`, drawn tile included.
    ///
    /// Red fives sort last so a plain copy is offered up before the red one,
    /// which is what a player melding by kind would keep back.
    fn tiles_of_kind(&self, kind: mahjong_core::tile::TileType, count: usize) -> Vec<Tile> {
        let mut held: Vec<Tile> = self
            .player
            .hand
            .tiles()
            .iter()
            .chain(self.player.hand.drawn().iter())
            .copied()
            .filter(|tile| tile.get() == kind)
            .collect();
        held.sort_by_key(|tile| tile.is_red_dora());
        held.truncate(count);
        held
    }

    /// Renders a call on `tile`, discarded by `target`.
    ///
    /// `consumed` are the hand tiles used, as listed by the matching
    /// [`mahjong_server::protocol::AvailableCall`] option. A ron is rendered as
    /// the win it is.
    pub fn call_on(
        &self,
        call_type: CallType,
        tile: Tile,
        target: Actor,
        consumed: Vec<Tile>,
    ) -> MjaiEvent {
        let actor = self.self_actor;
        match call_type {
            CallType::Chi => MjaiEvent::Chi {
                actor,
                target,
                pai: tile,
                consumed,
            },
            CallType::Pon => MjaiEvent::Pon {
                actor,
                target,
                pai: tile,
                consumed,
            },
            CallType::Daiminkan => MjaiEvent::Daiminkan {
                actor,
                target,
                pai: tile,
                consumed,
            },
            CallType::Ankan => MjaiEvent::Ankan { actor, consumed },
            CallType::Kakan => MjaiEvent::Kakan {
                actor,
                pai: tile,
                consumed,
            },
            CallType::Ron => self.hora(target, tile),
        }
    }

    /// A win declaration with no score breakdown, which is all a player sends.
    fn hora(&self, target: Actor, pai: Tile) -> MjaiEvent {
        MjaiEvent::Hora {
            actor: self.self_actor,
            target,
            pai,
            fu: None,
            fan: None,
            yakus: None,
            hora_tehais: None,
            uradora_markers: None,
            hora_points: None,
            deltas: None,
            scores: None,
            pao: None,
        }
    }

    fn decode_start_kyoku(&mut self, event: &MjaiEvent) -> Vec<ServerEvent> {
        let MjaiEvent::StartKyoku {
            bakaze,
            kyoku,
            honba,
            kyotaku,
            dora_marker,
            scores,
            tehais,
            ..
        } = event
        else {
            return Vec::new();
        };

        self.player_count = scores.len().clamp(3, 4);
        self.round_wind = *bakaze;
        self.round_number =
            bakaze.to_index() * self.player_count + (*kyoku as usize).saturating_sub(1);
        self.honba = *honba as usize;
        self.riichi_sticks = *kyotaku as usize;
        self.scores = scores.clone();
        self.dora_indicators = vec![*dora_marker];
        self.remaining_tiles = LIVE_WALL_AT_DEAL;
        self.total_kan_count = 0;
        self.last_draw_was_dead_wall = false;
        self.kan_replacement_due = None;
        self.last_discard = None;
        self.reach_pending = None;

        let hand: Vec<Tile> = tehais
            .get(self.self_actor)
            .map(|hand| hand.iter().filter_map(|slot| slot.known()).collect())
            .unwrap_or_default();
        let seat_wind = self.wind_of(self.self_actor);
        self.player = Player::new(
            seat_wind,
            hand.clone(),
            self.scores.get(self.self_actor).copied().unwrap_or(25000),
        );

        vec![ServerEvent::GameStarted {
            seat_wind,
            hand,
            scores: self.scores_by_seat(),
            round_wind: self.round_wind,
            dora_indicators: self.dora_indicators.clone(),
            round_number: self.round_number,
            total_rounds: ASSUMED_TOTAL_ROUNDS,
            honba: self.honba,
            riichi_sticks: self.riichi_sticks,
            three_player: self.player_count == 3,
            nuki_dora: self.settings.three_player && self.settings.nuki_dora,
        }]
    }

    fn decode_tsumo(&mut self, actor: Actor, pai: MjaiTile) -> Vec<ServerEvent> {
        self.remaining_tiles = self.remaining_tiles.saturating_sub(1);
        self.last_draw_was_dead_wall = self.kan_replacement_due == Some(actor);
        self.kan_replacement_due = None;

        if actor != self.self_actor {
            return vec![ServerEvent::OtherPlayerDrew {
                player: self.wind_of(actor),
                remaining_tiles: self.remaining_tiles,
            }];
        }

        let Some(tile) = pai.known() else {
            // Our own draw must be visible; a concealed one means the stream
            // was addressed to a different seat.
            return Vec::new();
        };
        self.player.draw(tile);
        // Reaching one's own draw clears the temporary furiten taken on by
        // passing a ron.
        self.player.is_temporary_furiten = false;

        let ctx = self.table_context();
        let mut out = vec![ServerEvent::TileDrawn {
            tile,
            remaining_tiles: self.remaining_tiles,
            can_tsumo: legality::can_tsumo(&self.player, &ctx),
            can_riichi: legality::can_riichi(&self.player, &ctx),
            is_furiten: self.player.is_furiten(),
        }];
        if self.settings.nine_terminals_draw && legality::can_nine_terminals(&self.player) {
            out.push(ServerEvent::NineTerminalsAvailable);
        }
        out
    }

    fn decode_dahai(&mut self, actor: Actor, pai: Tile, tsumogiri: bool) -> Vec<ServerEvent> {
        let is_reach_discard = self.reach_pending == Some(actor);
        self.reach_pending = None;
        if actor == self.self_actor {
            // A tsumogiri is recorded as such so the discard pool, and with it
            // the furiten check, matches what the server would have stored.
            let target = if tsumogiri { None } else { Some(pai) };
            self.player.try_discard(target);
            if is_reach_discard {
                // try_discard() clears ippatsu on every discard, but the
                // declaring discard is the one that must keep it: the window
                // runs until the *next* discard. The round restores it the
                // same way.
                self.player.is_ippatsu = true;
                if let Some(last) = self.player.discards.last_mut() {
                    last.is_riichi_declaration = true;
                }
            }
        }

        self.last_discard = Some((pai, actor));

        let mut out = vec![ServerEvent::TileDiscarded {
            player: self.wind_of(actor),
            tile: pai,
            is_tsumogiri: tsumogiri,
            // A display hint for our own client, absent from mjai.
            hand_index: None,
        }];

        if actor != self.self_actor {
            let ctx = self.table_context();
            let is_left_of_discarder = (actor + 1) % self.player_count == self.self_actor;
            let calls =
                legality::available_calls_for(&self.player, pai, is_left_of_discarder, &ctx);
            if !calls.is_empty() {
                out.push(ServerEvent::CallAvailable {
                    tile: pai,
                    discarder: self.wind_of(actor),
                    calls,
                });
            }
        }
        out
    }

    fn decode_reach(&mut self, actor: Actor) -> Vec<ServerEvent> {
        // The server takes the deposit when riichi is announced, before the
        // declaring discard, so it is applied here rather than at acceptance.
        if let Some(score) = self.scores.get_mut(actor) {
            *score -= 1000;
        }
        self.riichi_sticks += 1;
        self.reach_pending = Some(actor);
        if actor == self.self_actor {
            let is_double = self.player.is_first_turn && !self.player.first_turn_interrupted;
            self.player.declare_riichi(is_double);
            self.player.score = self.scores[actor];
        }
        vec![ServerEvent::PlayerRiichi {
            player: self.wind_of(actor),
            scores: self.scores_by_seat(),
            riichi_sticks: self.riichi_sticks,
        }]
    }

    fn decode_call(
        &mut self,
        actor: Actor,
        target: Actor,
        call_type: CallType,
        called: Tile,
        consumed: &[Tile],
    ) -> Vec<ServerEvent> {
        if matches!(
            call_type,
            CallType::Ankan | CallType::Daiminkan | CallType::Kakan
        ) {
            self.total_kan_count += 1;
            self.kan_replacement_due = Some(actor);
        }
        // Any call breaks the uninterrupted first go-around that double riichi
        // and the blessings require, and takes ippatsu off the table for
        // everyone, not only for the caller.
        self.player.first_turn_interrupted = true;
        self.player.is_ippatsu = false;

        if actor == self.self_actor {
            self.apply_own_call(target, call_type.clone(), called, consumed);
        }

        // The called tile goes first so that stripping it back out recovers
        // `consumed` in its original order even when the meld holds a red five
        // alongside its plain twin.
        let tiles = match call_type {
            CallType::Ankan => consumed.to_vec(),
            _ => {
                let mut tiles = vec![called];
                tiles.extend_from_slice(consumed);
                tiles
            }
        };

        let mut out = vec![ServerEvent::PlayerCalled {
            player: self.wind_of(actor),
            call_type,
            called_tile: called,
            tiles,
        }];
        if actor == self.self_actor {
            out.push(ServerEvent::HandUpdated {
                hand: self.player.hand.tiles().to_vec(),
            });
        }
        out
    }

    /// Applies a call this seat made to the tracked player.
    ///
    /// Mirrors what the round does, so the resulting melds and hand feed the
    /// shared legality rules unchanged. Each arm is guarded by the precondition
    /// its `do_*` requires: those index straight into the hand and panic on
    /// anything they cannot find, and a peer can send anything at all.
    fn apply_own_call(
        &mut self,
        target: Actor,
        call_type: CallType,
        called: Tile,
        consumed: &[Tile],
    ) {
        match call_type {
            CallType::Pon => {
                if let Some(pair) = two_tiles(consumed)
                    && self.holds_all(consumed)
                {
                    let from = self.meld_from(target);
                    self.player.do_pon(called, pair, from);
                }
            }
            CallType::Chi => {
                if let Some(pair) = two_tiles(consumed)
                    && self.holds_all(consumed)
                {
                    self.player.do_chi(called, pair);
                }
            }
            CallType::Daiminkan => {
                if self.player.can_daiminkan(called) {
                    let from = self.meld_from(target);
                    self.player.do_daiminkan(called, from);
                }
            }
            CallType::Ankan => {
                if self.player.ankan_options().contains(&called.get()) {
                    self.player.do_ankan(called.get());
                }
            }
            CallType::Kakan => {
                // The consumed tiles of a promotion sit in an existing meld,
                // not in the hand, so the hand check does not apply here.
                if self.player.kakan_options().contains(&called.get()) {
                    self.player.do_kakan(called.get());
                }
            }
            // Ron ends the hand and is reported as a win, not a call.
            CallType::Ron => {}
        }
    }

    /// Where a called tile came from, relative to this seat.
    ///
    /// Only meaningful for the calls that take a tile from someone else; a
    /// concealed or promoted quad has no discarder, and asking would trip the
    /// assertion inside `meld_from_relative`.
    fn meld_from(&self, target: Actor) -> MeldFrom {
        if target == self.self_actor {
            return MeldFrom::Unknown;
        }
        Player::meld_from_relative(self.self_actor, target, self.player_count)
    }

    fn decode_hora(&mut self, event: &MjaiEvent) -> Vec<ServerEvent> {
        let MjaiEvent::Hora {
            actor,
            target,
            pai,
            fu,
            fan,
            yakus,
            hora_tehais,
            uradora_markers,
            hora_points,
            scores,
            ..
        } = event
        else {
            return Vec::new();
        };
        if let Some(scores) = scores {
            self.scores = scores.clone();
        }
        let han = fan.unwrap_or(0);
        let fu = fu.unwrap_or(0);
        let winner = self.wind_of(*actor);

        vec![ServerEvent::RoundWon {
            winner,
            // mjai marks a self-draw by naming the winner as its own target.
            loser: (target != actor).then(|| self.wind_of(*target)),
            winning_tile: *pai,
            scores: self.scores_by_seat(),
            yaku_list: yakus
                .iter()
                .flatten()
                .filter_map(|(name, han)| score_item_from_name(name).map(|item| (item, *han)))
                .collect(),
            han,
            fu,
            score_points: hora_points.unwrap_or(0),
            // Not on the wire, so recomputed. A yakuman is indistinguishable
            // from a large han count here, which only affects the label.
            rank: determine_rank(han, fu, false),
            has_opened: false,
            uradora_indicators: uradora_markers.clone().unwrap_or_default(),
            riichi_sticks: self.riichi_sticks,
            honba: self.honba,
            honba_points: 0,
            player_hands: hora_tehais
                .as_ref()
                .map(|hand| {
                    vec![PlayerHandInfo {
                        wind: winner,
                        hand: hand.clone(),
                        melds: Vec::new(),
                        pei: Vec::new(),
                    }]
                })
                .unwrap_or_default(),
        }]
    }

    fn decode_ryukyoku(&mut self, event: &MjaiEvent) -> Vec<ServerEvent> {
        let MjaiEvent::Ryukyoku {
            reason,
            actor,
            tenpais,
            tehais,
            scores,
            ..
        } = event
        else {
            return Vec::new();
        };
        if let Some(scores) = scores {
            self.scores = scores.clone();
        }
        let player_hands = self.revealed_hands(tehais);
        // Nagashi Mangan is a payout, not a draw, and the server models it as
        // its own event.
        if matches!(reason, RyukyokuReason::Nagashimangan) {
            return vec![ServerEvent::RoundNagashiMangan {
                winners: Vec::new(),
                scores: self.scores_by_seat(),
                riichi_sticks: self.riichi_sticks,
                player_hands,
            }];
        }
        vec![ServerEvent::RoundDraw {
            scores: self.scores_by_seat(),
            reason: match reason {
                RyukyokuReason::Kyushukyuhai => DrawReason::NineTerminals,
                RyukyokuReason::Sufonrenta => DrawReason::FourWinds,
                RyukyokuReason::Suchareach => DrawReason::FourRiichi,
                RyukyokuReason::Sukaikan => DrawReason::FourKans,
                RyukyokuReason::Sanchaho => DrawReason::TripleRon,
                // An exhaustive draw, and the safest reading of a reason this
                // crate does not recognise.
                RyukyokuReason::Fanpai | RyukyokuReason::Other(_) => DrawReason::Exhaustive,
                RyukyokuReason::Nagashimangan => unreachable!("handled above"),
            },
            tenpai: tenpais
                .iter()
                .flatten()
                .enumerate()
                .filter(|(_, ready)| **ready)
                .map(|(actor, _)| self.wind_of(actor))
                .collect(),
            riichi_sticks: self.riichi_sticks,
            player_hands,
            declarer: actor.map(|actor| self.wind_of(actor)),
        }]
    }

    fn decode_reach_accepted(
        &mut self,
        actor: Actor,
        scores: &Option<Vec<i32>>,
    ) -> Vec<ServerEvent> {
        if let Some(scores) = scores {
            self.scores = scores.clone();
            if actor == self.self_actor
                && let Some(score) = self.scores.get(actor)
            {
                self.player.score = *score;
            }
        }
        Vec::new()
    }

    fn revealed_hands(&self, tehais: &Option<Vec<Vec<MjaiTile>>>) -> Vec<PlayerHandInfo> {
        tehais
            .iter()
            .flatten()
            .enumerate()
            .filter_map(|(actor, slots)| {
                let hand: Vec<Tile> = slots.iter().filter_map(|slot| slot.known()).collect();
                (!hand.is_empty()).then(|| PlayerHandInfo {
                    wind: self.wind_of(actor),
                    hand,
                    melds: Vec::new(),
                    pei: Vec::new(),
                })
            })
            .collect()
    }

    /// Maps an absolute seat to its seat wind for the current hand.
    ///
    /// Inverse of the encoder's rule: the dealer of hand `n` is seat
    /// `n % player_count`, and seat winds run outward from there.
    fn wind_of(&self, actor: Actor) -> Wind {
        let dealer = self.round_number % self.player_count;
        Wind::from_index((actor + self.player_count - dealer) % self.player_count)
    }

    /// The scores in the order the server protocol uses.
    ///
    /// `ServerEvent` indexes scores by seat rather than by seat wind, and this
    /// project's seats already line up with mjai's actors, so this is a copy
    /// rather than a permutation.
    fn scores_by_seat(&self) -> [i32; 4] {
        let mut out = [0; 4];
        for (actor, score) in self.scores.iter().enumerate().take(self.player_count) {
            out[actor] = *score;
        }
        out
    }
}

impl MjaiDecoder {
    /// Whether the tracked hand holds every one of `tiles`, counting repeats.
    fn holds_all(&self, tiles: &[Tile]) -> bool {
        let mut held = self.player.hand.tiles().to_vec();
        if let Some(drawn) = self.player.hand.drawn() {
            held.push(drawn);
        }
        tiles.iter().all(|tile| {
            match held
                .iter()
                .position(|candidate| candidate == tile)
                .or_else(|| {
                    held.iter()
                        .position(|candidate| candidate.get() == tile.get())
                }) {
                Some(index) => {
                    held.remove(index);
                    true
                }
                None => false,
            }
        })
    }
}

/// Returns the two hand tiles a sequence or triplet call uses.
///
/// A peer sending the wrong number is a protocol error, not a panic: the call
/// is skipped and the hand tracking simply misses that meld.
fn two_tiles(consumed: &[Tile]) -> Option<[Tile; 2]> {
    match consumed {
        [first, second] => Some([*first, *second]),
        _ => None,
    }
}
