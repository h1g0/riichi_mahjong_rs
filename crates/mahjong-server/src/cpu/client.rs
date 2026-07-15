//! CPU client: receives ServerEvents and returns ClientActions, speaking
//! exactly the same protocol as a human player.

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::{calc_shanten_number, calc_shanten_number_by_form};
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::settings::Settings;
use mahjong_core::tile::Tile;
use mahjong_core::winning_hand::name::Form;
use serde::{Deserialize, Serialize};

use crate::protocol::{AvailableCall, ClientAction, ServerEvent};

use super::evaluator;
use super::heuristics;
use super::state::CpuGameState;

/// CPU skill level.
///
/// Ordered `Weak < Normal < Strong`; heuristics use the ordering for
/// their "weak and up" / "normal and up" activation thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CpuLevel {
    /// Beginner: shanten only, no defense, makes mistakes
    Weak,
    /// Intermediate: tile acceptance, basic defense
    Normal,
    /// Advanced: value estimation and suji/wall/genbutsu defense
    Strong,
}

impl CpuLevel {
    pub fn display_name(&self) -> &'static str {
        match self {
            CpuLevel::Weak => "Weak",
            CpuLevel::Normal => "Normal",
            CpuLevel::Strong => "Strong",
        }
    }

    /// Whether tile acceptance is considered.
    pub fn uses_acceptance_count(&self) -> bool {
        matches!(self, CpuLevel::Normal | CpuLevel::Strong)
    }

    /// Whether hand value estimation is used.
    pub fn uses_value_estimation(&self) -> bool {
        matches!(self, CpuLevel::Strong)
    }

    /// Whether defensive play is used.
    pub fn uses_defense(&self) -> bool {
        matches!(self, CpuLevel::Normal | CpuLevel::Strong)
    }

    /// Whether deliberate mistakes (non-best discards) may occur.
    pub fn should_make_mistake(&self) -> bool {
        matches!(self, CpuLevel::Weak)
    }
}

/// CPU personality (playing style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuPersonality {
    Balanced,
    /// Rushes cheap fast wins (tanyao, pinfu, ...)
    Speedy,
    /// Chases value: closed riichi, value honours, dora
    HighValue,
    /// Prioritizes safe tiles and avoiding deal-ins
    Defensive,
}

impl CpuPersonality {
    pub fn display_name(&self) -> &'static str {
        match self {
            CpuPersonality::Balanced => "Balanced",
            CpuPersonality::Speedy => "Speedy",
            CpuPersonality::HighValue => "HighValue",
            CpuPersonality::Defensive => "Defensive",
        }
    }
}

/// Per-personality tuning parameters.
#[derive(Debug, Clone)]
pub struct PersonalityParams {
    /// Call eagerness, 0.0 (never calls) to 1.0 (calls eagerly)
    pub call_aggressiveness: f64,
    /// Weight on hand value
    pub value_weight: f64,
    /// Weight on winning fast
    pub speed_weight: f64,
    /// Fold threshold; higher folds earlier
    pub retreat_threshold: f64,
    /// Riichi eagerness, 0.0 (never) to 1.0 (eager)
    pub riichi_aggressiveness: f64,
}

/// CPU configuration: level plus personality.
#[derive(Debug, Clone)]
pub struct CpuConfig {
    pub level: CpuLevel,
    pub personality: CpuPersonality,
    pub params: PersonalityParams,
    /// Whether heuristics apply.
    ///
    /// Normally true; false restores the pre-heuristics behaviour for
    /// A/B comparison in simulations.
    pub heuristics_enabled: bool,
}

impl CpuConfig {
    pub fn new(level: CpuLevel, personality: CpuPersonality) -> Self {
        let params = PersonalityParams::from_personality(personality);
        CpuConfig {
            level,
            personality,
            params,
            heuristics_enabled: true,
        }
    }

    /// Returns a copy with heuristics disabled, for A/B simulations.
    pub fn without_heuristics(mut self) -> Self {
        self.heuristics_enabled = false;
        self
    }
}

/// Shuffles CPU configs (i.e. their seat order) in place.
///
/// Pass only the configs that actually play (e.g. the first two in a
/// three-player game).
pub fn shuffle_cpu_configs(configs: &mut [CpuConfig]) {
    use rand::seq::SliceRandom;
    configs.shuffle(&mut rand::rng());
}

/// CPU client: consumes ServerEvents and produces ClientActions.
pub struct CpuClient {
    pub config: CpuConfig,
    /// Game state, reconstructed purely from events
    pub state: CpuGameState,
}

impl CpuClient {
    pub fn new(config: CpuConfig) -> Self {
        Self::new_with_rules(config, &Settings::new())
    }

    /// Creates a CPU with immutable table rules that are not repeated in
    /// every gameplay event.
    pub fn new_with_rules(config: CpuConfig, rules: &Settings) -> Self {
        let mut state = CpuGameState::new();
        state.three_player = rules.three_player;
        state.nuki_dora = rules.three_player && rules.nuki_dora;
        state.opened_all_inside = rules.opened_all_inside;
        state.double_yakuman = rules.double_yakuman;
        CpuClient { config, state }
    }

    /// Handles a ServerEvent, returning a ClientAction when one is due.
    ///
    /// This is the CPU's gameplay interface to the server: like a human
    /// watching the screen, it learns changing state only from events.
    pub fn handle_event(&mut self, event: &ServerEvent) -> Option<ClientAction> {
        self.state.update(event);

        match event {
            ServerEvent::TileDrawn { .. } => Some(self.decide_on_draw()),
            ServerEvent::CallAvailable { .. } => Some(self.decide_call()),
            ServerEvent::HandUpdated { .. } => {
                if self.state.need_discard_after_call {
                    self.state.need_discard_after_call = false;
                    Some(self.decide_discard_after_call())
                } else {
                    None
                }
            }
            ServerEvent::NineTerminalsAvailable => Some(self.decide_nine_terminals()),
            _ => None,
        }
    }

    /// Post-draw decision: tsumo / riichi / pei / kan / discard.
    fn decide_on_draw(&self) -> ClientAction {
        if self.state.can_tsumo {
            return ClientAction::Tsumo;
        }

        if let Some(pei_action) = self.consider_pei() {
            return pei_action;
        }

        // Under riichi the hand is locked; only tsumogiri remains.
        if self.state.is_riichi {
            return ClientAction::Discard { tile: None };
        }

        // Only declare riichi with a tenpai-preserving discard in hand:
        // an invalid declaration is rejected by the server and would
        // stall the hand.
        if self.state.can_riichi
            && let Some(tile) = self.select_riichi_tile()
        {
            // Heuristic judgement (#168-#172); Neutral falls back to the
            // aggressiveness-based decision.
            let declare = if self.config.heuristics_enabled {
                let ctx = heuristics::CallContext {
                    state: &self.state,
                    config: &self.config,
                };
                match heuristics::judge_riichi(&ctx, tile) {
                    heuristics::RiichiJudgement::Declare => true,
                    heuristics::RiichiJudgement::Damaten => false,
                    heuristics::RiichiJudgement::Neutral => self.should_riichi(),
                }
            } else {
                self.should_riichi()
            };
            if declare {
                return ClientAction::Riichi { tile };
            }
        }

        if let Some(kan_action) = self.consider_ankan() {
            return kan_action;
        }

        self.decide_discard()
    }

    /// Picks a discard.
    fn decide_discard(&self) -> ClientAction {
        let candidates = evaluator::evaluate_discards(&self.state, &self.config);

        let attacking = self.should_attack();
        if let Some(tile) =
            evaluator::select_best_discard(&candidates, &self.config, attacking, &self.state)
        {
            if self.state.my_drawn == Some(tile) {
                ClientAction::Discard { tile: None }
            } else {
                ClientAction::Discard { tile: Some(tile) }
            }
        } else {
            ClientAction::Discard { tile: None }
        }
    }

    /// Picks the discard right after a call.
    fn decide_discard_after_call(&self) -> ClientAction {
        // Exclude swap-calling-forbidden tiles: the server would reject
        // them.
        let forbidden = self
            .state
            .my_melds()
            .last()
            .map(|meld| meld.forbidden_swap_tiles())
            .unwrap_or_default();

        let candidates: Vec<_> = evaluator::evaluate_discards(&self.state, &self.config)
            .into_iter()
            .filter(|c| !forbidden.contains(&c.tile.get()))
            .collect();
        let attacking = self.should_attack();

        if let Some(tile) =
            evaluator::select_best_discard(&candidates, &self.config, attacking, &self.state)
        {
            ClientAction::Discard { tile: Some(tile) }
        } else if let Some(&tile) = self
            .state
            .my_hand
            .iter()
            .rev()
            .find(|t| !forbidden.contains(&t.get()))
        {
            ClientAction::Discard { tile: Some(tile) }
        } else {
            ClientAction::Discard { tile: None }
        }
    }

    /// Call decision: ron / pon / chii / pass.
    fn decide_call(&self) -> ClientAction {
        let calls = &self.state.pending_calls;

        if calls.iter().any(|c| matches!(c, AvailableCall::Ron)) {
            return ClientAction::Ron;
        }

        for call in calls {
            if let AvailableCall::Pon { options } = call {
                if self.should_pon() {
                    // Prefer the option that spends the red five.
                    let tiles = options
                        .iter()
                        .find(|o| o[0].is_red_dora() || o[1].is_red_dora())
                        .copied()
                        .unwrap_or(options[0]);
                    return ClientAction::Pon { tiles };
                }
                break;
            }
        }

        // Called quads are always passed: they risk revealing new dora
        // for little value gain.
        for call in calls {
            if let AvailableCall::Chi { options } = call
                && let Some(tiles) = self.select_chi_option(options)
            {
                return ClientAction::Chi { tiles };
            }
        }

        ClientAction::Pass
    }

    /// Baseline riichi decision from the aggressiveness parameter,
    /// hedged when opponents have already declared.
    fn should_riichi(&self) -> bool {
        let params = &self.config.params;

        let riichi_count = self.state.player_riichi.iter().filter(|&&r| r).count();

        if riichi_count >= 2 && params.riichi_aggressiveness < 0.8 {
            return false;
        }

        if riichi_count >= 1 && params.riichi_aggressiveness < 0.4 {
            return false;
        }

        if self.state.remaining_tiles < 10 && params.riichi_aggressiveness < 0.9 {
            return false;
        }

        params.riichi_aggressiveness >= 0.4
    }

    /// Picks the riichi declaration discard.
    ///
    /// With heuristics on, maximizes the wait count and breaks ties on
    /// safety; with heuristics off, safety alone decides.
    ///
    /// Returns `Some(Some(tile))` for a hand discard, `Some(None)` for
    /// tsumogiri, and `None` when no discard preserves tenpai.
    fn select_riichi_tile(&self) -> Option<Option<Tile>> {
        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }

        // Include melds so hands with a concealed kan are judged correctly.
        let melds = self.state.my_melds_for_analysis();
        let visible = self.state.visible_tile_counts();
        let mut best: Option<(Tile, u32, f64)> = None;

        for (i, &tile) in all_tiles.iter().enumerate() {
            let mut remaining: Vec<Tile> = all_tiles.clone();
            remaining.remove(i);

            let hand = Hand::new_with_melds(remaining.clone(), melds.clone(), None);
            let shanten = calc_shanten_number(&hand);

            if shanten.is_ready() {
                let waits = if self.config.heuristics_enabled {
                    heuristics::remaining_wait_count(&remaining, &melds, &visible)
                } else {
                    0
                };
                let safety = super::defense::evaluate_safety(tile, &self.state, &self.config);
                let is_better = match best {
                    Some((_, best_waits, best_safety)) => {
                        waits > best_waits || (waits == best_waits && safety > best_safety)
                    }
                    None => true,
                };
                if is_better {
                    best = Some((tile, waits, safety));
                }
            }
        }

        best.map(|(tile, _, _)| {
            if self.state.my_drawn == Some(tile) {
                None
            } else {
                Some(tile)
            }
        })
    }

    /// Considers extracting a North tile (three-player with pei dora only).
    ///
    /// A North is almost always extracted: each is a guaranteed han plus a
    /// replacement draw. Exception: a hand chasing Thirteen Orphans (best
    /// form and within 3 shanten) keeps its Norths. Under riichi only a
    /// drawn North may be extracted.
    fn consider_pei(&self) -> Option<ClientAction> {
        if !(self.state.three_player && self.state.nuki_dora) {
            return None;
        }

        // With the live wall empty (post-haitei) no replacement draw
        // exists, so the server rejects pei - and a rejected CPU is never
        // re-consulted, stalling the hand (#296).
        if self.state.remaining_tiles == 0 {
            return None;
        }

        let drawn_is_north = self.state.my_drawn.is_some_and(|t| t.get() == Tile::Z4);

        if self.state.is_riichi {
            return drawn_is_north.then_some(ClientAction::Pei);
        }

        let hand_has_north = self.state.my_hand.iter().any(|t| t.get() == Tile::Z4);
        if !drawn_is_north && !hand_has_north {
            return None;
        }

        // Keep Norths for a Thirteen Orphans chase (closed hands only).
        let melds = self.state.my_melds_for_analysis();
        if melds.is_empty() {
            let hand = Hand::new_with_melds(self.state.my_hand.clone(), melds, self.state.my_drawn);
            let overall = calc_shanten_number(&hand);
            let kokushi = calc_shanten_number_by_form(&hand, Form::ThirteenOrphans);
            if kokushi <= overall && kokushi.as_i32() <= 3 {
                return None;
            }
        }

        Some(ClientAction::Pei)
    }

    /// Considers a concealed kan.
    fn consider_ankan(&self) -> Option<ClientAction> {
        if self.state.remaining_tiles == 0 {
            return None;
        }

        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }

        let mut counts = [0u8; 34];
        for tile in &all_tiles {
            counts[tile.get() as usize] += 1;
        }

        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };

        for (tile_type, &count) in counts.iter().enumerate() {
            if count == 4 {
                // Heuristics (normal and up) veto kans that break the hand
                // or follow an opponent's riichi.
                if heuristics::judge_ankan(&ctx, tile_type as u32)
                    == heuristics::CallJudgement::Forbid
                {
                    continue;
                }

                // Legacy behaviour with heuristics off: only Strong checks
                // that the kan preserves tenpai.
                if !self.config.heuristics_enabled && self.config.level == CpuLevel::Strong {
                    let remaining: Vec<Tile> = all_tiles
                        .iter()
                        .filter(|t| t.get() != tile_type as u32)
                        .copied()
                        .collect();
                    // Post-kan shape: existing melds plus the new quad
                    // (stored as three tiles for analysis).
                    let mut melds = self.state.my_melds_for_analysis();
                    melds.push(Meld {
                        tiles: vec![Tile::new(tile_type as u32); 3],
                        category: MeldType::Kan,
                        from: MeldFrom::Myself,
                        called_tile: None,
                    });
                    let hand = Hand::new_with_melds(remaining, melds, None);
                    if !calc_shanten_number(&hand).is_ready_or_won() {
                        continue; // The kan would break tenpai.
                    }
                }

                return Some(ClientAction::Kan {
                    tile_index: tile_type,
                });
            }
        }

        None
    }

    /// Push or fold decision.
    fn should_attack(&self) -> bool {
        let params = &self.config.params;

        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }
        let hand = Hand::new_with_melds(all_tiles, self.state.my_melds_for_analysis(), None);
        let shanten = calc_shanten_number(&hand);

        // Threats: riichi players, plus (with heuristics) opponents with
        // three or more melds, who are very likely tenpai (#180).
        let riichi_count = self.state.player_riichi.iter().filter(|&&r| r).count();
        let threat_count = if self.config.heuristics_enabled {
            let my_idx = CpuGameState::wind_to_index(self.state.my_seat_wind);
            let melded_threats = (0..4)
                .filter(|&i| {
                    i != my_idx
                        && !self.state.player_riichi[i]
                        && self.state.player_melds[i].len() >= 3
                })
                .count();
            riichi_count + melded_threats
        } else {
            riichi_count
        };

        // Fold far-from-tenpai hands late in the round (#183, weak and
        // up): with few draws left, a 2+ shanten hand never pushes,
        // regardless of threats or value.
        if self.config.heuristics_enabled && self.state.remaining_tiles <= 12 && shanten >= 2 {
            return false;
        }

        // Push/fold heuristics (#178, normal and up): push good-shape,
        // high-value, or dealer tenpai; fold bad-shape cheap tenpai.
        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };
        match heuristics::judge_push(&ctx, threat_count) {
            heuristics::PushJudgement::Push => return true,
            heuristics::PushJudgement::Fold => return false,
            heuristics::PushJudgement::Neutral => {}
        }

        if shanten.is_ready_or_won() {
            return true;
        }

        // Levels without defense always push - except that heuristics
        // give even the weak level a fold decision (#173).
        if !self.config.level.uses_defense() && !self.config.heuristics_enabled {
            return true;
        }

        if threat_count >= 2 && shanten >= 2 {
            return params.retreat_threshold < 0.3;
        }

        if threat_count >= 1 && shanten >= 2 {
            return params.retreat_threshold < 0.5;
        }

        if self.state.remaining_tiles < 15 && shanten >= 2 {
            return params.retreat_threshold < 0.4;
        }

        true
    }

    /// Whether to declare a nine-terminals abortive draw.
    ///
    /// With heuristics, judged by Thirteen Orphans potential
    /// (#158/#159/#160): 10+ orphan kinds always continue; 9 kinds
    /// continue for the HighValue personality or when far behind;
    /// otherwise declare the draw. Without heuristics only HighValue
    /// continues.
    fn decide_nine_terminals(&self) -> ClientAction {
        if self.config.heuristics_enabled {
            let mut counts = [0u8; 34];
            for t in &self.state.my_hand {
                counts[t.get() as usize] += 1;
            }
            if let Some(drawn) = self.state.my_drawn {
                counts[drawn.get() as usize] += 1;
            }
            let kinds = super::defense::ORPHAN_TYPES
                .iter()
                .filter(|&&t| counts[t as usize] > 0)
                .count();

            let continue_kokushi = kinds >= 10
                || (kinds >= 9
                    && (self.config.personality == CpuPersonality::HighValue
                        || heuristics::is_far_behind(&self.state)));
            return ClientAction::NineTerminals {
                declare: !continue_kokushi,
            };
        }

        let declare = self.config.personality != CpuPersonality::HighValue;
        ClientAction::NineTerminals { declare }
    }

    fn should_pon(&self) -> bool {
        let params = &self.config.params;
        let called_tile = match self.state.pending_call_tile {
            Some(t) => t,
            None => return false,
        };

        // Never pon without lowering the shanten (all levels).
        if !self.call_reduces_shanten_pon(called_tile) {
            return false;
        }

        // Heuristics: no yakuless calls, avoid going down to a bare pair,
        // pon value honours early. (Neutral with heuristics off.)
        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };
        match heuristics::judge_pon(&ctx, called_tile) {
            heuristics::CallJudgement::Forbid => return false,
            heuristics::CallJudgement::Encourage => return true,
            heuristics::CallJudgement::Neutral => {}
        }

        if self.config.level == CpuLevel::Weak {
            return true;
        }

        if params.call_aggressiveness < 0.3 {
            return false;
        }

        let tt = called_tile.get();

        // A value-honour pon secures a yaku, so take it.
        if is_yakuhai(tt, self.state.my_seat_wind, self.state.round_wind) {
            return true;
        }

        if self.config.personality == CpuPersonality::Speedy && is_tanyao_tile(tt) {
            return params.call_aggressiveness >= 0.5;
        }

        // HighValue protects the closed hand, so it declines pons.
        if self.config.personality == CpuPersonality::HighValue {
            return false;
        }

        params.call_aggressiveness >= 0.5
    }

    /// Picks the best chii option, or None to pass.
    fn select_chi_option(&self, options: &[[Tile; 2]]) -> Option<[Tile; 2]> {
        let params = &self.config.params;

        if self.config.personality == CpuPersonality::HighValue {
            return None;
        }

        if params.call_aggressiveness < 0.4 {
            return None;
        }

        let called_tile = self.state.pending_call_tile?;
        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };

        for &opt in options {
            if self.call_reduces_shanten_chi(called_tile, opt) {
                // Heuristics: no yakuless calls, avoid a bare pair.
                match heuristics::judge_chi(&ctx, called_tile, opt) {
                    heuristics::CallJudgement::Forbid => continue,
                    heuristics::CallJudgement::Encourage => return Some(opt),
                    heuristics::CallJudgement::Neutral => {}
                }

                if self.config.personality == CpuPersonality::Speedy {
                    return Some(opt);
                }
                if params.call_aggressiveness >= 0.5 {
                    return Some(opt);
                }
            }
        }

        None
    }

    /// Whether a pon would lower the shanten.
    fn call_reduces_shanten_pon(&self, called_tile: Tile) -> bool {
        // Existing melds must count on both sides, or the comparison
        // is skewed.
        let melds = self.state.my_melds_for_analysis();
        let current_hand = Hand::new_with_melds(self.state.my_hand.clone(), melds.clone(), None);
        let current_shanten = calc_shanten_number(&current_hand);

        let tt = called_tile.get();
        let mut remaining = self.state.my_hand.clone();
        let mut removed = 0;
        remaining.retain(|t| {
            if t.get() == tt && removed < 2 {
                removed += 1;
                false
            } else {
                true
            }
        });

        if removed < 2 {
            return false;
        }

        let mut melds = melds;
        melds.push(Meld {
            tiles: vec![called_tile, called_tile, called_tile],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(called_tile),
        });

        let new_hand = Hand::new_with_melds(remaining, melds, None);
        calc_shanten_number(&new_hand) < current_shanten
    }

    /// Whether a chii would lower the shanten.
    fn call_reduces_shanten_chi(&self, called_tile: Tile, hand_tiles: [Tile; 2]) -> bool {
        // Existing melds must count on both sides, or the comparison
        // is skewed.
        let melds = self.state.my_melds_for_analysis();
        let current_hand = Hand::new_with_melds(self.state.my_hand.clone(), melds.clone(), None);
        let current_shanten = calc_shanten_number(&current_hand);

        // Remove the two hand tiles, matching red fives exactly.
        let mut remaining = self.state.my_hand.clone();
        let mut chi_tiles_for_meld = Vec::new();
        for &target in &hand_tiles {
            if let Some(pos) = remaining.iter().position(|t| *t == target) {
                chi_tiles_for_meld.push(remaining.remove(pos));
            } else {
                return false;
            }
        }

        let mut melds = melds;
        melds.push(Meld {
            tiles: vec![called_tile, chi_tiles_for_meld[0], chi_tiles_for_meld[1]],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(called_tile),
        });

        let new_hand = Hand::new_with_melds(remaining, melds, None);
        calc_shanten_number(&new_hand) < current_shanten
    }
}

/// Whether the tile kind is a value honour (yakuhai).
pub(crate) fn is_yakuhai(
    tile_type: u32,
    seat_wind: mahjong_core::tile::Wind,
    round_wind: mahjong_core::tile::Wind,
) -> bool {
    use mahjong_core::tile::Tile as T;
    // Dragons, round wind, or seat wind; Wind discriminants equal the
    // corresponding tile kinds.
    matches!(tile_type, T::Z5..=T::Z7)
        || tile_type == round_wind as u32
        || tile_type == seat_wind as u32
}

/// Whether the tile kind is an inside tile (2-8), usable for tanyao.
fn is_tanyao_tile(tile_type: u32) -> bool {
    if tile_type >= 27 {
        return false;
    }
    let num = tile_type % 9;
    (1..=7).contains(&num)
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
