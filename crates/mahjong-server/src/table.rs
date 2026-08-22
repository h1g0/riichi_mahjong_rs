//! Table state across a whole game (East-only or hanchan):
//! creating hands, advancing them, and deciding when the game ends.

use serde::{Deserialize, Serialize};

use mahjong_core::settings::{AllLastRule, BankruptcyRule, Settings};
use mahjong_core::tile::{Tile, Wind};

use crate::protocol::{ClientAction, ServerEvent};
use crate::round::{CallResponse, Round, RoundResult, TurnPhase};

/// Game length.
///
/// A hanchan adds the South round to the East round. The total hand count
/// is [`GameLength::wind_count`] x player count: 4/8 hands in four-player
/// games, 3/6 in three-player games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GameLength {
    /// East round only (tonpuusen / 東風戦)
    #[default]
    EastOnly,
    /// East + South rounds (hanchan / 半荘)
    Hanchan,
}

impl GameLength {
    /// Number of round winds played.
    fn wind_count(self) -> usize {
        match self {
            GameLength::EastOnly => 1,
            GameLength::Hanchan => 2,
        }
    }
}

/// Game settings.
#[derive(Debug, Clone)]
pub struct GameSettings {
    /// Starting score
    pub initial_score: i32,
    /// Game length
    pub length: GameLength,
    /// Rule settings
    pub rules: Settings,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            initial_score: 25000,
            length: GameLength::EastOnly,
            rules: Settings::new(),
        }
    }
}

impl GameSettings {
    /// Builds game settings with the standard starting score for the rules
    /// (25000 four-player, 35000 three-player). Used when converting from
    /// `CreateRoom` or the local game setup.
    pub fn with_rules(length: GameLength, rules: Settings) -> Self {
        let initial_score = if rules.three_player { 35000 } else { 25000 };
        GameSettings {
            initial_score,
            length,
            rules,
        }
    }

    /// Standard three-player settings: 35000 points, East-only.
    pub fn sanma_default() -> Self {
        Self::with_rules(
            GameLength::EastOnly,
            Settings {
                three_player: true,
                ..Settings::new()
            },
        )
    }
}

/// Table state.
pub struct Table {
    /// Game settings
    pub settings: GameSettings,
    /// The hand in progress
    pub round: Option<Round>,
    /// Round wind
    pub round_wind: Wind,
    /// Hand number, 0-based (East 1 = 0, East 2 = 1, ...)
    pub round_number: usize,
    /// Continuance counter (honba / 本場)
    pub honba: usize,
    /// Riichi deposits on the table
    pub riichi_sticks: usize,
    /// Dealer's player index (0-3)
    pub dealer: usize,
    /// Scores
    pub scores: [i32; 4],
    /// Whether the game is over
    pub is_game_over: bool,
    /// Base seed for deterministic walls; `None` shuffles at random.
    seed: Option<u64>,
    /// Hands started since the seed was set, mixed into each wall seed.
    round_serial: u64,
}

/// Derives the wall seed of the `serial`-th hand from a game's base seed.
///
/// The splitmix64 finalizer scrambles the bits so consecutive hands do not
/// get correlated walls.
pub fn derive_wall_seed(base_seed: u64, serial: u64) -> u64 {
    let mut x = base_seed ^ serial.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x
}

impl Table {
    pub fn new(settings: GameSettings) -> Self {
        let initial_score = settings.initial_score;
        let player_count = settings.rules.player_count();
        // The dummy seat in three-player games always has zero points.
        let scores = std::array::from_fn(|i| if i < player_count { initial_score } else { 0 });
        Table {
            settings,
            round: None,
            round_wind: Wind::East,
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
            dealer: 0,
            scores,
            is_game_over: false,
            seed: None,
            round_serial: 0,
        }
    }

    /// Picks the starting dealer at random.
    ///
    /// Call before East 1. Drawn from the real player range so the dummy
    /// seat in three-player games can never be the dealer.
    pub fn randomize_dealer(&mut self) {
        use rand::RngExt;
        self.dealer = rand::rng().random_range(0..self.player_count());
    }

    /// Total number of hands in the game
    /// (4/8 four-player, 3/6 three-player).
    fn total_rounds(&self) -> usize {
        self.settings.length.wind_count() * self.player_count()
    }

    /// Maximum number of hands after the optional single-wind extension.
    fn maximum_rounds(&self) -> usize {
        self.total_rounds() + usize::from(self.settings.rules.round_extension) * self.player_count()
    }

    fn player_count(&self) -> usize {
        self.settings.rules.player_count()
    }

    /// Starts a new hand.
    ///
    /// Once a seed is set the wall stays deterministic for every later
    /// hand too, so a whole game is reproducible and not just its opening
    /// hand (#377).
    pub fn start_round(&mut self) {
        let round = match self.seed {
            Some(base) => {
                // The opening hand uses the base seed unchanged so a given
                // seed keeps producing the wall it always produced.
                let seed = match self.round_serial {
                    0 => base,
                    serial => derive_wall_seed(base, serial),
                };
                self.round_serial += 1;
                Round::new_with_seed(
                    seed,
                    self.round_wind,
                    self.dealer,
                    self.scores,
                    self.honba,
                    self.riichi_sticks,
                    self.round_number,
                    self.maximum_rounds(),
                    self.settings.rules.clone(),
                )
            }
            None => Round::new(
                self.round_wind,
                self.dealer,
                self.scores,
                self.honba,
                self.riichi_sticks,
                self.round_number,
                self.maximum_rounds(),
                self.settings.rules.clone(),
            ),
        };
        self.round = Some(round);
    }

    /// Starts a new hand with a seeded, deterministic wall, for
    /// simulations and reproducible tests.
    ///
    /// The seed also drives every hand started afterwards through
    /// [`Table::start_round`].
    pub fn start_round_with_seed(&mut self, seed: u64) {
        self.set_seed(seed);
        self.start_round();
    }

    /// Makes every hand from the next one on deterministic.
    ///
    /// The next hand uses `seed` itself; later hands derive their wall
    /// seeds from it.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = Some(seed);
        self.round_serial = 0;
    }

    pub fn current_round(&self) -> Option<&Round> {
        self.round.as_ref()
    }

    pub fn current_round_mut(&mut self) -> Option<&mut Round> {
        self.round.as_mut()
    }

    pub fn drain_events(&mut self) -> Vec<(usize, ServerEvent)> {
        match self.round.as_mut() {
            Some(round) => round.drain_events(),
            None => Vec::new(),
        }
    }

    /// Handles a client action; returns whether it was accepted.
    pub fn handle_action(&mut self, player_idx: usize, action: ClientAction) -> bool {
        let round = match self.round.as_mut() {
            Some(r) => r,
            None => return false,
        };
        if player_idx >= round.player_count {
            return false;
        }

        match action {
            // Turn actions: current player only.
            ClientAction::Discard { tile } => {
                let accepted = round.current_player == player_idx
                    && round.phase == TurnPhase::WaitForDiscard
                    && round.do_discard(tile);
                if !accepted {
                    // Clients apply discards locally before sending, so a
                    // silent rejection would desync their hand (#294).
                    round.resync_hand(player_idx);
                }
                accepted
            }
            ClientAction::Tsumo => {
                if round.current_player != player_idx {
                    return false;
                }
                round.do_tsumo()
            }
            ClientAction::Riichi { tile } => {
                let accepted = round.current_player == player_idx && round.do_riichi(tile);
                if !accepted {
                    // The riichi declaration discard is also applied locally
                    // by the client (#294).
                    round.resync_hand(player_idx);
                }
                accepted
            }

            // Call responses: eligible players during WaitForCalls only.
            ClientAction::Ron => round.respond_to_call(player_idx, CallResponse::Ron),
            ClientAction::Pon { tiles } => round.respond_to_call(
                player_idx,
                CallResponse::Pon {
                    hand_tile_types: tiles,
                },
            ),
            ClientAction::Chi { tiles } => round.respond_to_call(
                player_idx,
                CallResponse::Chi {
                    hand_tile_types: tiles,
                },
            ),
            ClientAction::Pass => round.respond_to_call(player_idx, CallResponse::Pass),

            ClientAction::Kan { tile_index } => {
                if round.phase == TurnPhase::WaitForCalls {
                    round.respond_to_call(player_idx, CallResponse::Daiminkan)
                } else if round.current_player == player_idx
                    && round.phase == TurnPhase::WaitForDiscard
                {
                    if tile_index >= Tile::LEN {
                        return false;
                    }
                    round.do_kan(tile_index as u32)
                } else {
                    false
                }
            }

            ClientAction::Pei => {
                if round.current_player != player_idx {
                    return false;
                }
                round.do_pei()
            }

            ClientAction::NineTerminals { declare } => round.do_nine_terminals(player_idx, declare),
        }
    }

    pub fn advance_auto_player(&mut self) -> bool {
        match self.round.as_mut() {
            Some(round) => round.advance_auto_player(),
            None => false,
        }
    }

    /// Post-hand bookkeeping: applies scores, rotates the dealer,
    /// and advances the hand counter.
    pub fn finish_round(&mut self) {
        let (result, scores, riichi_sticks) = {
            let round = match self.round.as_ref() {
                Some(r) if r.is_over() => r,
                _ => return,
            };
            (
                round.result.clone(),
                round.get_scores(),
                round.riichi_sticks,
            )
        };

        self.scores = scores;
        self.riichi_sticks = riichi_sticks;

        if self.bankruptcy_reached() || self.cold_end_reached() {
            self.is_game_over = true;
            self.round = None;
            return;
        }

        if self.round_number >= self.total_rounds() && self.has_target_score() {
            self.is_game_over = true;
            self.round = None;
            return;
        }

        if self.should_end_on_final_dealer(result.as_ref()) {
            self.is_game_over = true;
            self.round = None;
            return;
        }

        match result {
            Some(RoundResult::ExhaustiveDraw { dealer_tenpai }) => {
                self.honba += 1;
                // A tenpai dealer keeps the deal; otherwise the deal
                // rotates and the hand counter advances.
                if !dealer_tenpai || !self.settings.rules.tenpai_renchan {
                    self.dealer = (self.dealer + 1) % self.player_count();
                    self.advance_round_number();
                }
            }
            Some(RoundResult::SpecialDraw) => {
                // Abortive draws count as a continuation: honba goes up
                // and the hand does not advance.
                self.honba += 1;
            }
            Some(RoundResult::Tsumo { winner, .. }) => {
                if winner == self.dealer {
                    self.honba += 1;
                } else {
                    self.honba = 0;
                    self.dealer = (self.dealer + 1) % self.player_count();
                    self.advance_round_number();
                }
            }
            Some(RoundResult::Ron { winners, .. }) => {
                // The dealer keeps the deal if they are among the winners,
                // whether single or multiple ron.
                if winners.contains(&self.dealer) {
                    self.honba += 1;
                } else {
                    self.honba = 0;
                    self.dealer = (self.dealer + 1) % self.player_count();
                    self.advance_round_number();
                }
            }
            Some(RoundResult::NagashiMangan { winners }) => {
                // Nagashi Mangan is settled as a win. The dealer continues
                // when they are among simultaneous winners.
                if winners.contains(&self.dealer) {
                    self.honba += 1;
                } else {
                    self.honba = 0;
                    self.dealer = (self.dealer + 1) % self.player_count();
                    self.advance_round_number();
                }
            }
            None => {}
        }

        self.round = None;
    }

    fn advance_round_number(&mut self) {
        self.round_number += 1;
        let scheduled_rounds = self.total_rounds();
        if self.round_number >= scheduled_rounds
            && (!self.settings.rules.round_extension
                || self.has_target_score()
                || self.round_number >= self.maximum_rounds())
        {
            self.is_game_over = true;
        }

        // The round wind advances every player_count hands.
        self.round_wind = Wind::from_index(self.round_number / self.player_count());
    }

    fn active_scores(&self) -> &[i32] {
        &self.scores[..self.player_count()]
    }

    fn target_score(&self) -> i32 {
        if self.settings.rules.three_player {
            40000
        } else {
            30000
        }
    }

    fn has_target_score(&self) -> bool {
        self.active_scores()
            .iter()
            .any(|&score| score >= self.target_score())
    }

    fn bankruptcy_reached(&self) -> bool {
        match self.settings.rules.bankruptcy_rule {
            BankruptcyRule::None => false,
            BankruptcyRule::Negative => self.active_scores().iter().any(|&score| score < 0),
            BankruptcyRule::ZeroOrLess => self.active_scores().iter().any(|&score| score <= 0),
        }
    }

    fn cold_end_reached(&self) -> bool {
        !self.settings.rules.three_player
            && self.settings.rules.cold_end
            && self.active_scores().iter().any(|&score| score >= 55000)
    }

    fn dealer_is_leading(&self) -> bool {
        let dealer_score = self.scores[self.dealer];
        self.active_scores()
            .iter()
            .all(|&score| score <= dealer_score)
    }

    fn should_end_on_final_dealer(&self, result: Option<&RoundResult>) -> bool {
        if self.round_number + 1 != self.total_rounds() || !self.dealer_is_leading() {
            return false;
        }

        let dealer_won = match result {
            Some(RoundResult::Tsumo { winner, .. }) => *winner == self.dealer,
            Some(RoundResult::Ron { winners, .. })
            | Some(RoundResult::NagashiMangan { winners }) => winners.contains(&self.dealer),
            _ => false,
        };
        let dealer_tenpai = matches!(
            result,
            Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: true
            })
        );

        match self.settings.rules.all_last_rule {
            AllLastRule::Continue => false,
            AllLastRule::Win => dealer_won,
            AllLastRule::WinOrTenpai => dealer_won || dealer_tenpai,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use mahjong_core::hand::Hand;

    /// A rejected discard must resync the sender with HandUpdated +
    /// TileDrawn (#294): clients apply discards locally before sending,
    /// so a silent rejection left the hand desynced and the game
    /// appeared frozen.
    #[test]
    fn test_rejected_discard_resyncs_hand() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        {
            let round = table.current_round_mut().unwrap();
            let seat_wind = round.players[0].seat_wind;
            let hand = Hand::from("1m2m3m4m5m6m7m8m9m1p2p3p1s 5z");
            round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
            round.players[0].draw(hand.drawn().unwrap());
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        // Discarding a tile not in the hand (East) must be rejected.
        let accepted = table.handle_action(
            0,
            ClientAction::Discard {
                tile: Some(Tile::new(Tile::Z1)),
            },
        );
        assert!(!accepted);

        let expected_hand = {
            let round = table.current_round().unwrap();
            assert_eq!(round.phase, TurnPhase::WaitForDiscard);
            assert!(round.players[0].discards.is_empty());
            round.players[0].hand.tiles().to_vec()
        };

        // Only the sender receives HandUpdated + TileDrawn (same tile).
        let events = table.drain_events();
        assert!(
            events.iter().any(|(seat, e)| *seat == 0
                && matches!(e, ServerEvent::HandUpdated { hand } if *hand == expected_hand)),
            "HandUpdated was not sent"
        );
        assert!(
            events.iter().any(|(seat, e)| *seat == 0
                && matches!(e, ServerEvent::TileDrawn { tile, .. } if tile.get() == Tile::Z5)),
            "TileDrawn was not resent"
        );
        assert!(
            events.iter().all(|(seat, _)| *seat == 0),
            "an event was sent to another player"
        );
    }

    /// A rejection while no tile is drawn (right after a call) must send
    /// HandUpdated only.
    #[test]
    fn test_rejected_discard_without_drawn_resyncs_hand_only() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        {
            let round = table.current_round_mut().unwrap();
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        // Tsumogiri with no drawn tile must be rejected.
        let accepted = table.handle_action(0, ClientAction::Discard { tile: None });
        assert!(!accepted);

        let events = table.drain_events();
        assert!(
            events
                .iter()
                .any(|(seat, e)| *seat == 0 && matches!(e, ServerEvent::HandUpdated { .. })),
            "HandUpdated was not sent"
        );
        assert!(
            !events
                .iter()
                .any(|(_, e)| matches!(e, ServerEvent::TileDrawn { .. })),
            "TileDrawn was sent without a drawn tile"
        );
    }

    /// A rejected riichi must resync too (#294): the declaration discard
    /// is applied locally by the client just like a normal discard.
    #[test]
    fn test_rejected_riichi_resyncs_hand() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        {
            let round = table.current_round_mut().unwrap();
            let seat_wind = round.players[0].seat_wind;
            // Not tenpai, so riichi is impossible.
            let hand = Hand::from("1m4m7m2p5p8p3s6s9s1z2z3z4z 5z");
            round.players[0] = Player::new(seat_wind, hand.tiles().to_vec(), 25000);
            round.players[0].draw(hand.drawn().unwrap());
            round.current_player = 0;
            round.phase = TurnPhase::WaitForDiscard;
            round.drain_events();
        }

        let accepted = table.handle_action(0, ClientAction::Riichi { tile: None });
        assert!(!accepted);

        {
            let round = table.current_round().unwrap();
            assert!(!round.players[0].is_riichi);
            assert_eq!(round.riichi_sticks, 0);
        }

        let events = table.drain_events();
        assert!(
            events
                .iter()
                .any(|(seat, e)| *seat == 0 && matches!(e, ServerEvent::HandUpdated { .. })),
            "HandUpdated was not sent"
        );
        assert!(
            events.iter().any(|(seat, e)| *seat == 0
                && matches!(e, ServerEvent::TileDrawn { tile, .. } if tile.get() == Tile::Z5)),
            "TileDrawn was not resent"
        );
    }

    #[test]
    fn test_table_new() {
        let table = Table::new(GameSettings::default());
        assert_eq!(table.round_wind, Wind::East);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.scores, [25000; 4]);
        assert_eq!(table.riichi_sticks, 0);
        assert!(!table.is_game_over);
        assert!(table.round.is_none());
    }

    #[test]
    fn test_randomize_dealer_stays_in_player_range() {
        let mut table = Table::new(GameSettings::default());
        for _ in 0..50 {
            table.randomize_dealer();
            assert!(table.dealer < 4);
        }

        // The dummy seat must never become the starting dealer.
        let mut table = Table::new(GameSettings::sanma_default());
        for _ in 0..50 {
            table.randomize_dealer();
            assert!(table.dealer < 3);
        }
    }

    #[test]
    fn test_randomize_dealer_varies() {
        // Odds of a single dealer in 100 tries are (1/4)^99: effectively zero.
        let mut table = Table::new(GameSettings::default());
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            table.randomize_dealer();
            seen.insert(table.dealer);
        }
        assert!(seen.len() > 1);
    }

    #[test]
    fn test_start_round_with_random_dealer() {
        let mut table = Table::new(GameSettings::default());
        table.randomize_dealer();
        let dealer = table.dealer;
        table.start_round();

        let round = table.current_round().unwrap();
        assert_eq!(round.dealer, dealer);
        assert_eq!(round.current_player, dealer);
        assert_eq!(round.players[dealer].seat_wind, Wind::East);
    }

    #[test]
    fn test_table_start_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        assert!(table.round.is_some());

        let round = table.current_round().unwrap();
        assert_eq!(round.round_wind, Wind::East);
        assert_eq!(round.current_player, 0);
    }

    #[test]
    fn test_table_play_round_to_end() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.play_to_end();

        assert!(table.current_round().unwrap().is_over());

        table.finish_round();
        assert!(table.round.is_none());
        assert_eq!(table.honba, 1); // Draws increment the honba counter.
    }

    /// Collects the dealt hands of the first `count` hands of a game,
    /// ending each hand immediately as an exhaustive draw.
    fn deal_several_rounds(table: &mut Table, count: usize) -> Vec<String> {
        let mut deals = Vec::new();
        for _ in 0..count {
            table.start_round();
            {
                let round = table.current_round().unwrap();
                deals.push(
                    round
                        .players
                        .iter()
                        .map(|p| p.hand.to_short_string())
                        .collect::<Vec<_>>()
                        .join("|"),
                );
            }
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }
        deals
    }

    /// A seed must fix every hand of the game, not just the first one (#377).
    #[test]
    fn test_seed_makes_every_round_deterministic() {
        let settings = || GameSettings {
            length: GameLength::Hanchan,
            ..Default::default()
        };

        let mut first = Table::new(settings());
        first.set_seed(4242);
        let mut second = Table::new(settings());
        second.set_seed(4242);

        let deals = deal_several_rounds(&mut first, 6);
        assert_eq!(deals, deal_several_rounds(&mut second, 6));
    }

    /// Different seeds must still give different games.
    #[test]
    fn test_different_seeds_give_different_rounds() {
        let mut first = Table::new(GameSettings::default());
        first.set_seed(1);
        let mut second = Table::new(GameSettings::default());
        second.set_seed(2);

        assert_ne!(
            deal_several_rounds(&mut first, 4),
            deal_several_rounds(&mut second, 4)
        );
    }

    /// Each hand of a seeded game gets its own wall.
    #[test]
    fn test_seeded_rounds_differ_from_each_other() {
        let mut table = Table::new(GameSettings::default());
        table.set_seed(7);

        let deals = deal_several_rounds(&mut table, 4);
        let unique: std::collections::HashSet<&String> = deals.iter().collect();
        assert_eq!(unique.len(), deals.len(), "a wall was reused: {deals:?}");
    }

    /// The opening wall of a seeded game is unchanged by the per-hand
    /// derivation, so seeds used in existing tests keep their meaning (#377).
    #[test]
    fn test_seed_keeps_opening_wall() {
        let mut carried = Table::new(GameSettings::default());
        carried.start_round_with_seed(42);
        let carried_deal = carried.current_round().unwrap().players[0]
            .hand
            .to_short_string();

        let expected =
            Round::new_with_seed(42, Wind::East, 0, [25000; 4], 0, 0, 0, 4, Settings::new());
        assert_eq!(carried_deal, expected.players[0].hand.to_short_string());
    }

    #[test]
    fn test_table_carries_riichi_sticks_across_draw() {
        let mut table = Table::new(GameSettings::default());
        table.riichi_sticks = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.riichi_sticks = 3;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::ExhaustiveDraw {
            dealer_tenpai: false,
        });

        table.finish_round();
        assert_eq!(table.riichi_sticks, 3);
    }

    #[test]
    fn test_table_handle_discard() {
        let mut table = Table::new(GameSettings::default());
        // This test targets discard handling, so a fixed wall avoids
        // randomly entering the nine-terminals choice before the discard.
        table.start_round_with_seed(42);
        table.drain_events();

        {
            let round = table.current_round_mut().unwrap();
            round.do_draw();
        }
        table.drain_events();

        assert!(table.handle_action(0, ClientAction::Discard { tile: None }));

        // Pass everyone through a possible WaitForCalls phase.
        {
            let round = table.current_round_mut().unwrap();
            if round.phase == TurnPhase::WaitForCalls {
                for i in 0..4 {
                    if let Some(ref cs) = round.call_state
                        && !cs.responded[i]
                    {
                        round.respond_to_call(i, CallResponse::Pass);
                        if round.call_state.is_none() {
                            break;
                        }
                    }
                }
            }
        }

        let round = table.current_round().unwrap();
        assert_eq!(round.current_player, 1);
    }

    #[test]
    fn test_table_wrong_player_action() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.drain_events();

        {
            let round = table.current_round_mut().unwrap();
            round.do_draw();
        }

        assert!(!table.handle_action(1, ClientAction::Discard { tile: None }));
    }

    #[test]
    fn test_table_east_wind_game() {
        let mut table = Table::new(GameSettings {
            initial_score: 25000,
            length: GameLength::EastOnly,
            ..Default::default()
        });

        // Four consecutive noten draws rotate the deal through everyone
        // and must end the game.
        for _ in 0..4 {
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(table.is_game_over);
    }

    #[test]
    fn test_game_settings_with_rules() {
        let rules = Settings {
            three_player: true,
            nuki_dora: false,
            triple_ron_draw: true,
            ..Settings::new()
        };
        let settings = GameSettings::with_rules(GameLength::Hanchan, rules.clone());
        assert_eq!(settings.initial_score, 35000);
        assert_eq!(settings.length, GameLength::Hanchan);
        assert_eq!(settings.rules, rules);

        let four_player = GameSettings::with_rules(GameLength::EastOnly, Settings::new());
        assert_eq!(four_player.initial_score, 25000);
    }

    #[test]
    fn test_sanma_table_new() {
        let table = Table::new(GameSettings::sanma_default());
        assert_eq!(table.scores, [35000, 35000, 35000, 0]);
        assert_eq!(table.round_wind, Wind::East);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_sanma_east_wind_game_is_three_rounds() {
        let mut table = Table::new(GameSettings::sanma_default());

        // Three consecutive noten draws must end a three-player
        // East-only game.
        for i in 0..3 {
            assert!(!table.is_game_over, "game ended before hand {}", i + 1);
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(
            table.is_game_over,
            "three-player East-only game did not end after three hands"
        );
    }

    #[test]
    fn test_sanma_dealer_rotation_wraps_at_three() {
        let mut table = Table::new(GameSettings {
            length: GameLength::Hanchan,
            ..GameSettings::sanma_default()
        });

        let mut dealers = Vec::new();
        for _ in 0..4 {
            table.start_round();
            dealers.push(table.dealer);
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        // The deal cycles through the three players, then South begins.
        assert_eq!(dealers, vec![0, 1, 2, 0]);
        assert_eq!(table.round_wind, Wind::South);
    }

    /// Runs a table through noten draws and counts hands until game over.
    fn count_rounds_until_game_over(settings: GameSettings, max_rounds: usize) -> usize {
        let mut table = Table::new(settings);
        for i in 0..max_rounds {
            if table.is_game_over {
                return i;
            }
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }
        assert!(
            table.is_game_over,
            "game did not end after {max_rounds} hands"
        );
        max_rounds
    }

    /// A four-player hanchan is eight hands, East 1 - South 4 (#271).
    #[test]
    fn test_hanchan_game_is_eight_rounds() {
        let settings = GameSettings {
            length: GameLength::Hanchan,
            ..Default::default()
        };
        assert_eq!(count_rounds_until_game_over(settings, 8), 8);
    }

    /// A three-player hanchan is six hands, East 1 - South 3 (#271).
    #[test]
    fn test_sanma_hanchan_game_is_six_rounds() {
        let settings = GameSettings {
            length: GameLength::Hanchan,
            ..GameSettings::sanma_default()
        };
        assert_eq!(count_rounds_until_game_over(settings, 6), 6);
    }

    #[test]
    fn test_table_game_over_when_score_is_negative() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.players[0].score = -100;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Ron {
            winners: vec![1],
            loser: 0,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert!(table.is_game_over);
        assert!(table.round.is_none());
        assert_eq!(table.scores[0], -100);
    }

    #[test]
    fn test_table_methods_without_round_are_noops() {
        let mut table = Table::new(GameSettings::default());

        assert!(table.current_round().is_none());
        assert!(table.current_round_mut().is_none());
        assert!(table.drain_events().is_empty());
        assert!(!table.advance_auto_player());
        assert!(!table.handle_action(0, ClientAction::Tsumo));

        table.finish_round();
        assert!(!table.is_game_over);
        assert_eq!(table.round_number, 0);
        assert_eq!(table.honba, 0);
    }

    #[test]
    fn test_table_drain_events_delegates_to_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let events = table.drain_events();
        assert!(!events.is_empty());
        assert!(table.drain_events().is_empty());
    }

    #[test]
    fn test_table_finish_round_ignores_active_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        table.finish_round();

        assert!(table.round.is_some());
        assert_eq!(table.round_number, 0);
        assert_eq!(table.honba, 0);
    }

    #[test]
    fn test_table_finish_round_ignores_round_over_without_result() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.current_round_mut().unwrap().phase = TurnPhase::RoundOver;

        table.finish_round();

        assert!(table.round.is_none());
        assert_eq!(table.round_number, 0);
        assert_eq!(table.honba, 0);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_table_finish_round_dealer_tenpai_draw_keeps_dealer() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::ExhaustiveDraw {
            dealer_tenpai: true,
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_table_finish_round_special_draw_keeps_round_number() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::SpecialDraw);

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_table_finish_round_dealer_tsumo_continues() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Tsumo {
            winner: 0,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
    }

    #[test]
    fn test_table_finish_round_child_tsumo_advances_dealer() {
        let mut table = Table::new(GameSettings::default());
        table.honba = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Tsumo {
            winner: 1,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 0);
        assert_eq!(table.dealer, 1);
        assert_eq!(table.round_number, 1);
    }

    #[test]
    fn test_table_finish_round_ron_with_dealer_winner_continues() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Ron {
            winners: vec![2, 0],
            loser: 1,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
    }

    #[test]
    fn test_table_finish_round_ron_without_dealer_winner_advances() {
        let mut table = Table::new(GameSettings::default());
        table.honba = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Ron {
            winners: vec![1, 2],
            loser: 3,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 0);
        assert_eq!(table.dealer, 1);
        assert_eq!(table.round_number, 1);
    }

    #[test]
    fn test_table_finish_round_nagashi_with_dealer_winner_continues() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::NagashiMangan {
            winners: vec![2, 0],
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
    }

    #[test]
    fn test_table_finish_round_nagashi_without_dealer_advances() {
        let mut table = Table::new(GameSettings::default());
        table.honba = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::NagashiMangan { winners: vec![1] });

        table.finish_round();

        assert_eq!(table.honba, 0);
        assert_eq!(table.dealer, 1);
        assert_eq!(table.round_number, 1);
    }

    #[test]
    fn test_table_advance_round_updates_prevailing_wind_in_south_game() {
        let mut table = Table::new(GameSettings {
            initial_score: 25000,
            length: GameLength::Hanchan,
            ..Default::default()
        });

        for _ in 0..4 {
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(!table.is_game_over);
        assert_eq!(table.round_number, 4);
        assert_eq!(table.round_wind, Wind::South);
        assert_eq!(table.dealer, 0);
    }

    #[test]
    fn test_east_game_extension_enters_south_and_stops_at_target() {
        let mut table = Table::new(GameSettings::with_rules(
            GameLength::EastOnly,
            Settings {
                round_extension: true,
                ..Settings::new()
            },
        ));

        for _ in 0..4 {
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(!table.is_game_over);
        assert_eq!(table.round_number, 4);
        assert_eq!(table.round_wind, Wind::South);

        table.start_round();
        let round = table.current_round_mut().unwrap();
        round.players[0].score = 30000;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Tsumo {
            winner: 0,
            winning_tile: Tile::new(Tile::M1),
        });
        table.finish_round();

        assert!(table.is_game_over);
    }

    #[test]
    fn test_hanchan_extension_enters_west_and_is_capped_at_one_wind() {
        let settings = GameSettings::with_rules(
            GameLength::Hanchan,
            Settings {
                round_extension: true,
                ..Settings::new()
            },
        );
        assert_eq!(count_rounds_until_game_over(settings, 12), 12);
    }

    #[test]
    fn test_tenpai_renchan_can_be_disabled() {
        let mut table = Table::new(GameSettings::with_rules(
            GameLength::EastOnly,
            Settings {
                tenpai_renchan: false,
                ..Settings::new()
            },
        ));
        table.start_round();
        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::ExhaustiveDraw {
            dealer_tenpai: true,
        });

        table.finish_round();

        assert_eq!(table.dealer, 1);
        assert_eq!(table.round_number, 1);
        assert_eq!(table.honba, 1);
    }

    #[test]
    fn test_bankruptcy_thresholds_and_sanma_dummy_seat() {
        let mut no_bankruptcy = Table::new(GameSettings::with_rules(
            GameLength::EastOnly,
            Settings {
                bankruptcy_rule: BankruptcyRule::None,
                ..Settings::new()
            },
        ));
        no_bankruptcy.start_round();
        let round = no_bankruptcy.current_round_mut().unwrap();
        round.players[0].score = -100;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::SpecialDraw);
        no_bankruptcy.finish_round();
        assert!(!no_bankruptcy.is_game_over);

        let mut zero_or_less = Table::new(GameSettings::with_rules(
            GameLength::EastOnly,
            Settings {
                bankruptcy_rule: BankruptcyRule::ZeroOrLess,
                ..Settings::new()
            },
        ));
        zero_or_less.start_round();
        let round = zero_or_less.current_round_mut().unwrap();
        round.players[0].score = 0;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::SpecialDraw);
        zero_or_less.finish_round();
        assert!(zero_or_less.is_game_over);

        let mut sanma = Table::new(GameSettings::with_rules(
            GameLength::EastOnly,
            Settings {
                three_player: true,
                bankruptcy_rule: BankruptcyRule::ZeroOrLess,
                ..Settings::new()
            },
        ));
        sanma.start_round();
        let round = sanma.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::SpecialDraw);
        sanma.finish_round();
        assert!(!sanma.is_game_over);
    }

    #[test]
    fn test_final_dealer_can_stop_after_winning_while_leading() {
        let mut table = Table::new(GameSettings::with_rules(
            GameLength::EastOnly,
            Settings {
                all_last_rule: AllLastRule::Win,
                ..Settings::new()
            },
        ));
        table.round_number = 3;
        table.dealer = 3;
        table.scores = [24000, 25000, 25000, 26000];
        table.start_round();
        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Tsumo {
            winner: 3,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert!(table.is_game_over);
        assert_eq!(table.round_number, 3);
    }

    #[test]
    fn test_cold_end_at_fifty_five_thousand() {
        let mut table = Table::new(GameSettings::with_rules(
            GameLength::Hanchan,
            Settings {
                cold_end: true,
                ..Settings::new()
            },
        ));
        table.start_round();
        let round = table.current_round_mut().unwrap();
        round.players[0].score = 55000;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::SpecialDraw);

        table.finish_round();

        assert!(table.is_game_over);
    }

    #[test]
    fn test_table_handle_actions_reject_wrong_phase_or_player() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        assert!(!table.handle_action(0, ClientAction::Discard { tile: None }));
        assert!(!table.handle_action(0, ClientAction::Tsumo));
        assert!(!table.handle_action(0, ClientAction::Riichi { tile: None }));
        assert!(!table.handle_action(0, ClientAction::Ron));
        assert!(!table.handle_action(
            0,
            ClientAction::Pon {
                tiles: [Tile::new(Tile::M1), Tile::new(Tile::M1)],
            },
        ));
        assert!(!table.handle_action(
            0,
            ClientAction::Chi {
                tiles: [Tile::new(Tile::M1), Tile::new(Tile::M2)],
            },
        ));
        assert!(!table.handle_action(0, ClientAction::Pass));
        assert!(!table.handle_action(
            0,
            ClientAction::Kan {
                tile_index: Tile::M1 as usize,
            },
        ));
        assert!(!table.handle_action(0, ClientAction::NineTerminals { declare: true }));

        let round = table.current_round_mut().unwrap();
        round.do_draw();

        assert!(!table.handle_action(1, ClientAction::Tsumo));
        assert!(!table.handle_action(1, ClientAction::Riichi { tile: None }));
        assert!(!table.handle_action(
            1,
            ClientAction::Kan {
                tile_index: Tile::M1 as usize,
            },
        ));
        assert!(!table.handle_action(
            0,
            ClientAction::Kan {
                tile_index: Tile::LEN,
            },
        ));
    }

    #[test]
    fn test_table_handle_nine_terminals_continue_and_declare() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.current_round_mut().unwrap().phase = TurnPhase::WaitForNineTerminals;

        assert!(!table.handle_action(1, ClientAction::NineTerminals { declare: false }));
        assert!(table.handle_action(0, ClientAction::NineTerminals { declare: false }));
        assert_eq!(
            table.current_round().unwrap().phase,
            TurnPhase::WaitForDiscard
        );

        table.current_round_mut().unwrap().phase = TurnPhase::WaitForNineTerminals;
        assert!(table.handle_action(0, ClientAction::NineTerminals { declare: true }));
        assert_eq!(table.current_round().unwrap().phase, TurnPhase::RoundOver);
        assert!(matches!(
            table.current_round().unwrap().result,
            Some(RoundResult::SpecialDraw)
        ));
    }
}
