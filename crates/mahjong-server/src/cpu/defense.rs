//! Defensive logic.
//!
//! Evaluates tile safety: genbutsu, suji, walls, honours, and terminals,
//! combined with opponent threat models (riichi, melds, flush signs,
//! yakuman signs).

use mahjong_core::tile::{Tile, TileType, Wind, dora_indicator_to_dora_in};

use super::client::{CpuConfig, CpuLevel, is_yakuhai};
use super::state::CpuGameState;

/// The 13 Thirteen Orphans tile kinds (terminals and honours).
pub(crate) const ORPHAN_TYPES: [TileType; 13] = [
    Tile::M1,
    Tile::M9,
    Tile::P1,
    Tile::P9,
    Tile::S1,
    Tile::S9,
    Tile::Z1,
    Tile::Z2,
    Tile::Z3,
    Tile::Z4,
    Tile::Z5,
    Tile::Z6,
    Tile::Z7,
];

/// Threat assessment of one opponent.
#[derive(Debug, Clone, Default)]
pub struct Threat {
    /// Alarm strength: riichi = 1.0; meld-based tenpai signs are weaker
    pub weight: f64,
    /// Suspected flush suit (0 = characters, 1 = circles, 2 = bamboos);
    /// that suit and honours become dangerous
    pub flush_suit: Option<usize>,
    /// Big Dragons signs: two or more dragon kinds melded
    pub dragon_alert: bool,
    /// Big/Little Winds signs: two or more wind kinds melded
    pub wind_alert: bool,
    /// Thirteen Orphans signs: closed hand discarding mostly inside tiles
    /// from the start
    pub kokushi_alert: bool,
}

/// Evaluates a tile's safety, 0.0 (most dangerous) to 1.0 (safest).
///
/// Takes the minimum safety against every opponent threat. With
/// heuristics off, only riichi players count as threats (the legacy
/// behaviour).
pub fn evaluate_safety(tile: Tile, state: &CpuGameState, config: &CpuConfig) -> f64 {
    let my_idx = CpuGameState::wind_to_index(state.my_seat_wind);
    let strict = config.heuristics_enabled && config.level >= CpuLevel::Normal;

    let mut min_safety = 1.0f64;

    for i in 0..4 {
        if i == my_idx {
            continue;
        }

        let Some(threat) = assess_threat(state, i, config) else {
            continue;
        };

        let safety = evaluate_safety_against_threat(
            tile,
            &state.all_discards[i],
            state,
            Wind::from_index(i),
            &threat,
            strict,
        );
        min_safety = min_safety.min(safety);
    }

    min_safety
}

/// Assesses one opponent's threat; `None` when they pose none.
///
/// - A riichi player is always a full threat (weight 1.0).
/// - #180 (weak+): three or more melds means likely tenpai.
/// - #181 (normal+): two or more melds in a single suit plus honours
///   suggests a flush.
/// - #182 (weak+): two dragon kinds / two wind kinds melded suggests
///   a yakuman.
/// - #182 (normal+): a closed hand discarding mostly inside tiles from
///   the start suggests Thirteen Orphans (or a chanta-family hand).
pub(crate) fn assess_threat(
    state: &CpuGameState,
    idx: usize,
    config: &CpuConfig,
) -> Option<Threat> {
    if state.player_riichi[idx] {
        return Some(Threat {
            weight: 1.0,
            ..Threat::default()
        });
    }

    if !config.heuristics_enabled {
        return None;
    }

    let melds = &state.player_melds[idx];
    let mut threat = Threat::default();

    // #180: three or more melds means likely tenpai.
    if melds.len() >= 3 {
        threat.weight = 0.7;
    }

    // #182: yakuman signs (weak and up).
    let dragon_kinds = melds
        .iter()
        .filter(|m| {
            m.tiles
                .first()
                .is_some_and(|t| (Tile::Z5..=Tile::Z7).contains(&t.get()))
        })
        .count();
    if dragon_kinds >= 2 {
        threat.dragon_alert = true;
        threat.weight = threat.weight.max(0.6);
    }
    let wind_kinds = melds
        .iter()
        .filter(|m| {
            m.tiles
                .first()
                .is_some_and(|t| (Tile::Z1..=Tile::Z4).contains(&t.get()))
        })
        .count();
    if wind_kinds >= 2 {
        threat.wind_alert = true;
        threat.weight = threat.weight.max(0.6);
    }

    // #182 extension: Thirteen Orphans signs (normal and up).
    // The hand is closed-only and produces a discard pool of unneeded
    // inside tiles. Surplus orphan copies may still be discarded, so the
    // orphan count in the pool cannot prove the chase abandoned; the alarm
    // is lifted only when some orphan kind is publicly dead (4 visible in
    // pools, melds, and indicators). Our own concealed tiles do not count:
    // the opponent cannot see them, and any orphans we hold are exactly
    // their winning candidates. The same pool could also be a
    // chanta-family hand, but the conclusion - orphans and honours are
    // dangerous - is the same.
    if config.level >= CpuLevel::Normal && melds.is_empty() {
        let discards = &state.all_discards[idx];
        if discards.len() >= 5 {
            let early_orphans = discards
                .iter()
                .take(6)
                .filter(|t| t.is_1_9_honour())
                .count();
            let some_orphan_dead = ORPHAN_TYPES
                .iter()
                .any(|&t| publicly_visible(state, t) >= 4);
            if early_orphans <= 1 && !some_orphan_dead {
                threat.kokushi_alert = true;
                threat.weight = threat.weight.max(0.5);
            }
        }
    }

    // #181: flush signs (normal and up): two or more melds all in one
    // suit (plus honours).
    if config.level >= CpuLevel::Normal && melds.len() >= 2 {
        let mut suits_used = [false; 3];
        let mut has_number_meld = false;
        for meld in melds.iter() {
            for t in &meld.tiles {
                if t.get() < 27 {
                    suits_used[(t.get() / 9) as usize] = true;
                    has_number_meld = true;
                }
            }
        }
        let used: Vec<usize> = (0..3).filter(|&s| suits_used[s]).collect();
        if has_number_meld && used.len() == 1 {
            threat.flush_suit = Some(used[0]);
            threat.weight = threat.weight.max(0.6);
        }
    }

    if threat.weight > 0.0 {
        Some(threat)
    } else {
        None
    }
}

/// Counts publicly visible copies, excluding our own hand and drawn tile.
///
/// Used to detect live tiles (ones opponents have seen nothing of).
fn publicly_visible(state: &CpuGameState, tile_type: TileType) -> u8 {
    let total = state.visible_tile_counts()[tile_type as usize];
    let mut own = 0u8;
    for t in &state.my_hand {
        if t.get() == tile_type {
            own += 1;
        }
    }
    if state.my_drawn.is_some_and(|t| t.get() == tile_type) {
        own += 1;
    }
    total.saturating_sub(own)
}

/// Safety against a single riichi-weight threat; compatibility wrapper
/// for the older tests.
#[cfg(test)]
fn evaluate_safety_against_player(
    tile: Tile,
    opponent_discards: &[Tile],
    state: &CpuGameState,
) -> f64 {
    let threat = Threat {
        weight: 1.0,
        ..Threat::default()
    };
    evaluate_safety_against_threat(
        tile,
        opponent_discards,
        state,
        state.my_seat_wind,
        &threat,
        false,
    )
}

/// Evaluates a tile's safety against one threat.
///
/// `strict` enables the normal-and-up extras: one-chance (#175) and
/// live value honours / dora neighbours (#177).
fn evaluate_safety_against_threat(
    tile: Tile,
    opponent_discards: &[Tile],
    state: &CpuGameState,
    opponent_seat_wind: Wind,
    threat: &Threat,
    strict: bool,
) -> f64 {
    let tt = tile.get();

    // Genbutsu: a tile in the opponent's own discards can never deal in,
    // whatever the threat.
    if opponent_discards.iter().any(|d| d.get() == tt) {
        return 1.0;
    }

    // Live honours against yakuman signs (#182) are treated as deadly
    // regardless of the threat weight.
    if tt >= 27 && publicly_visible(state, tt) == 0 {
        if threat.dragon_alert && (Tile::Z5..=Tile::Z7).contains(&tt) {
            return 0.05;
        }
        if threat.wind_alert && (Tile::Z1..=Tile::Z4).contains(&tt) {
            return 0.05;
        }
    }

    // Against Thirteen Orphans signs every orphan kind is a potential
    // wait, so orphans stay dangerous regardless of weight; live ones
    // especially.
    if threat.kokushi_alert && tile.is_1_9_honour() {
        return if publicly_visible(state, tt) == 0 {
            0.08
        } else {
            0.2
        };
    }

    let visible_counts = state.visible_tile_counts();
    let mut base: f64;

    if tt >= 27 {
        // Honour safety scales with how many copies are visible.
        let visible = visible_counts[tt as usize];
        base = match visible {
            4 => 1.0,  // All visible (cannot deal in).
            3 => 0.95, // One copy left: nearly safe.
            2 => 0.6,
            1 => 0.4,
            _ => 0.3, // Fully live.
        };

        // #177 (normal+): live value honours are extra dangerous.
        if strict
            && is_yakuhai(tt, opponent_seat_wind, state.round_wind)
            && publicly_visible(state, tt) == 0
        {
            base = base.min(0.22);
        }
    } else if is_suji(tt, opponent_discards) {
        base = 0.75;
    } else if is_kabe(tt, &visible_counts) {
        base = 0.7;
    } else {
        // Terminals are safer than inside tiles.
        let num = tt % 9;
        base = match num {
            0 | 8 => 0.4, // 1, 9
            1 | 7 => 0.3, // 2, 8
            2 | 6 => 0.2, // 3, 7
            _ => 0.15,    // 4, 5, 6
        };

        // #175 (normal+): one-chance tiles lean safe.
        if strict && is_one_chance(tt, &visible_counts) {
            base = base.max(0.5);
        }
    }

    // #177 (normal+): dora and its neighbours cluster in opponents'
    // hands, so they are extra dangerous.
    if strict && tt < 27 && is_dora_or_neighbor(tt, state) {
        base = (base - 0.08).max(0.05);
    }

    // #181: the flush suit and honours get more dangerous.
    if let Some(suit) = threat.flush_suit
        && (tt >= 27 || (tt / 9) as usize == suit)
    {
        base *= 0.5;
    }

    // Scale by threat weight: weaker threats discount the danger.
    if threat.weight >= 1.0 {
        base
    } else {
        1.0 - (1.0 - base) * threat.weight
    }
}

/// Whether the tile is the dora or adjacent to it in the same suit.
fn is_dora_or_neighbor(tile_type: TileType, state: &CpuGameState) -> bool {
    for indicator in &state.dora_indicators {
        let dora = dora_indicator_to_dora_in(indicator.get(), state.three_player);
        if dora >= 27 {
            if tile_type == dora {
                return true;
            }
            continue;
        }
        if tile_type / 9 == dora / 9 {
            let diff = (tile_type % 9) as i32 - (dora % 9) as i32;
            if diff.abs() <= 1 {
                return true;
            }
        }
    }
    false
}

/// Whether the tile is suji-safe against the opponent's discards.
///
/// E.g. a discarded 4m makes 1m and 7m suji; 5m makes 2m/8m;
/// 6m makes 3m/9m.
fn is_suji(tile_type: TileType, opponent_discards: &[Tile]) -> bool {
    if tile_type >= 27 {
        return false; // Honours have no suji.
    }

    let suit_start = (tile_type / 9) * 9;
    let num = tile_type - suit_start; // 0-8

    // Suji pairs: (1,4), (2,5), (3,6), (4,7), (5,8), (6,9);
    // num is 0-indexed.
    let suji_partner = match num {
        0 => Some(suit_start + 3), // 1 -> 4
        1 => Some(suit_start + 4), // 2 -> 5
        2 => Some(suit_start + 5), // 3 -> 6
        3 => {
            // 4 -> 1 or 7
            if opponent_discards.iter().any(|d| d.get() == suit_start)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 6)
            {
                return true;
            }
            return false;
        }
        4 => {
            // 5 -> 2 or 8
            if opponent_discards.iter().any(|d| d.get() == suit_start + 1)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 7)
            {
                return true;
            }
            return false;
        }
        5 => {
            // 6 -> 3 or 9
            if opponent_discards.iter().any(|d| d.get() == suit_start + 2)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 8)
            {
                return true;
            }
            return false;
        }
        6 => Some(suit_start + 3), // 7 -> 4
        7 => Some(suit_start + 4), // 8 -> 5
        8 => Some(suit_start + 5), // 9 -> 6
        _ => None,
    };

    if let Some(partner) = suji_partner {
        opponent_discards.iter().any(|d| d.get() == partner)
    } else {
        false
    }
}

/// Whether the tile is wall-safe (no-chance): when every copy of a
/// connecting tile is visible, no sequence through it can exist, making
/// the neighbours safer.
fn is_kabe(tile_type: TileType, visible_counts: &[u8; 34]) -> bool {
    is_blocked(tile_type, visible_counts, 4)
}

/// One-chance (#175): at most one copy of the sequence material remains.
/// Weaker than a wall, but a two-sided wait is unlikely to hit.
fn is_one_chance(tile_type: TileType, visible_counts: &[u8; 34]) -> bool {
    is_blocked(tile_type, visible_counts, 3)
}

/// Generalized wall check: whether every sequence through the tile needs
/// a material tile with `min_visible`+ copies visible.
/// 4 = no-chance (wall), 3 = one-chance.
fn is_blocked(tile_type: TileType, visible_counts: &[u8; 34], min_visible: u8) -> bool {
    if tile_type >= 27 {
        return false; // Honours have no walls.
    }

    let suit_start = (tile_type / 9) * 9;
    let num = tile_type - suit_start; // 0-8

    // Check the material of every sequence containing this tile,
    // e.g. 5m needs 3/4/6/7m for 345m, 456m, 567m.
    let mut blocked_patterns = 0;
    let total_patterns;

    match num {
        0 => {
            // A 1 only fits 123.
            total_patterns = 1;
            if visible_counts[(suit_start + 1) as usize] >= min_visible
                || visible_counts[(suit_start + 2) as usize] >= min_visible
            {
                blocked_patterns = 1;
            }
        }
        1 => {
            // A 2 fits 123 and 234.
            total_patterns = 2;
            if visible_counts[suit_start as usize] >= min_visible
                || visible_counts[(suit_start + 2) as usize] >= min_visible
            {
                blocked_patterns += 1;
            }
            if visible_counts[(suit_start + 2) as usize] >= min_visible
                || visible_counts[(suit_start + 3) as usize] >= min_visible
            {
                blocked_patterns += 1;
            }
        }
        7 => {
            // An 8 fits 789 and 678.
            total_patterns = 2;
            if visible_counts[(suit_start + 8) as usize] >= min_visible
                || visible_counts[(suit_start + 6) as usize] >= min_visible
            {
                blocked_patterns += 1;
            }
            if visible_counts[(suit_start + 6) as usize] >= min_visible
                || visible_counts[(suit_start + 5) as usize] >= min_visible
            {
                blocked_patterns += 1;
            }
        }
        8 => {
            // A 9 only fits 789.
            total_patterns = 1;
            if visible_counts[(suit_start + 6) as usize] >= min_visible
                || visible_counts[(suit_start + 7) as usize] >= min_visible
            {
                blocked_patterns = 1;
            }
        }
        _ => {
            // 3-7 fit three sequence patterns.
            total_patterns = 3;
            if num >= 2
                && (visible_counts[(suit_start + num - 2) as usize] >= min_visible
                    || visible_counts[(suit_start + num - 1) as usize] >= min_visible)
            {
                blocked_patterns += 1;
            }
            if (1..=7).contains(&num)
                && (visible_counts[(suit_start + num - 1) as usize] >= min_visible
                    || visible_counts[(suit_start + num + 1) as usize] >= min_visible)
            {
                blocked_patterns += 1;
            }
            if num <= 6
                && (visible_counts[(suit_start + num + 1) as usize] >= min_visible
                    || visible_counts[(suit_start + num + 2) as usize] >= min_visible)
            {
                blocked_patterns += 1;
            }
        }
    }

    blocked_patterns > 0 && blocked_patterns >= total_patterns
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
    use mahjong_core::tile::{Tile, Wind};

    use crate::cpu::client::CpuPersonality;

    fn test_config() -> CpuConfig {
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced)
    }

    fn pon_meld(tile_type: u32) -> Meld {
        Meld {
            tiles: vec![Tile::new(tile_type); 3],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(Tile::new(tile_type)),
        }
    }

    #[test]
    fn test_genbutsu() {
        let discards = vec![Tile::new(Tile::M5)];
        let state = CpuGameState::new();
        let safety = evaluate_safety_against_player(Tile::new(Tile::M5), &discards, &state);
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_suji_basic() {
        let discards = vec![Tile::new(Tile::M4)];
        assert!(is_suji(Tile::M1, &discards));
        assert!(is_suji(Tile::M7, &discards));
        assert!(!is_suji(Tile::M5, &discards));
    }

    #[test]
    fn test_suji_middle() {
        let discards = vec![Tile::new(Tile::M5)];
        assert!(is_suji(Tile::M2, &discards));
        assert!(is_suji(Tile::M8, &discards));
    }

    #[test]
    fn test_honour_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = Vec::new();
        let safety = evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state);
        assert!(safety < 0.5);
    }

    // --- Threat model (#175 #177 #180 #181 #182) ---

    #[test]
    fn test_assess_threat_riichi_is_full_weight() {
        let mut state = CpuGameState::new();
        state.player_riichi[1] = true;
        let threat = assess_threat(&state, 1, &test_config()).expect("riichi is a threat");
        assert_eq!(threat.weight, 1.0);
    }

    #[test]
    fn test_assess_threat_three_melds() {
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::M2), pon_meld(Tile::P5), pon_meld(Tile::S7)];

        let threat = assess_threat(&state, 1, &test_config()).expect("3 melds is a threat");
        assert!(threat.weight > 0.0 && threat.weight < 1.0);

        // Without heuristics only riichi players are threats.
        let config = test_config().without_heuristics();
        assert!(assess_threat(&state, 1, &config).is_none());

        // Two melds in different suits are not yet a threat.
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::M2), pon_meld(Tile::P5)];
        assert!(assess_threat(&state, 1, &test_config()).is_none());
    }

    #[test]
    fn test_assess_threat_flush_signs() {
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::P2), pon_meld(Tile::P7)];

        let threat = assess_threat(&state, 1, &test_config()).expect("flush signs");
        assert_eq!(threat.flush_suit, Some(1)); // circles

        // The weak level ignores flush signs.
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        assert!(assess_threat(&state, 1, &config).is_none());
    }

    #[test]
    fn test_assess_threat_yakuman_signs() {
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::Z5), pon_meld(Tile::Z6)];

        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        let threat = assess_threat(&state, 1, &config).expect("dragon signs");
        assert!(threat.dragon_alert);

        let mut state = CpuGameState::new();
        state.player_melds[2] = vec![pon_meld(Tile::Z1), pon_meld(Tile::Z2)];
        let threat = assess_threat(&state, 2, &config).expect("wind signs");
        assert!(threat.wind_alert);
    }

    #[test]
    fn test_assess_threat_kokushi_signs() {
        let middle_discards = |n: usize| -> Vec<Tile> {
            [Tile::M5, Tile::P4, Tile::S6, Tile::M3, Tile::P7, Tile::S5]
                .iter()
                .take(n)
                .map(|&t| Tile::new(t))
                .collect()
        };

        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        let threat = assess_threat(&state, 1, &test_config()).expect("kokushi signs");
        assert!(threat.kokushi_alert);

        // Surplus orphan discards after the opening do not lift the
        // alarm: the chase stays alive until some orphan is 4-dead.
        let mut state = CpuGameState::new();
        let mut discards = middle_discards(5);
        discards.push(Tile::new(Tile::M1));
        discards.extend(middle_discards(3));
        discards.extend([
            Tile::new(Tile::Z2),
            Tile::new(Tile::P9),
            Tile::new(Tile::Z3),
        ]); // several surplus orphans, none 4-dead
        state.all_discards[1] = discards;
        let threat = assess_threat(&state, 1, &test_config()).expect("kokushi signs");
        assert!(threat.kokushi_alert);

        // Two or more orphans discarded early reads as a normal hand.
        let mut state = CpuGameState::new();
        let mut discards = vec![Tile::new(Tile::Z3), Tile::new(Tile::M9)];
        discards.extend(middle_discards(4));
        state.all_discards[1] = discards;
        assert!(assess_threat(&state, 1, &test_config()).is_none());

        // A publicly 4-dead orphan means the opponent must abandon the
        // chase, lifting the alarm.
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        state.all_discards[2] = vec![Tile::new(Tile::M1); 4];
        assert!(assess_threat(&state, 1, &test_config()).is_none());

        // Our own hand must not count: the opponent cannot see it and
        // keeps chasing - and our held orphans are exactly their waits.
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        state.all_discards[2] = vec![Tile::new(Tile::Z7); 2];
        state.my_hand = vec![Tile::new(Tile::Z7); 2]; // publicly only 2 visible
        let threat = assess_threat(&state, 1, &test_config()).expect("kokushi signs");
        assert!(threat.kokushi_alert, "自分の手牌を見切り判定に含めない");

        // Fewer than five discards is too early to judge.
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(4);
        assert!(assess_threat(&state, 1, &test_config()).is_none());

        // Any meld rules out Thirteen Orphans.
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        state.player_melds[1] = vec![pon_meld(Tile::P5)];
        let threat = assess_threat(&state, 1, &test_config());
        assert!(threat.is_none_or(|t| !t.kokushi_alert));

        // The weak level ignores these signs.
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        assert!(assess_threat(&state, 1, &config).is_none());
    }

    #[test]
    fn test_orphans_dangerous_against_kokushi_suspect() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.all_discards[1] = vec![
            Tile::new(Tile::M5),
            Tile::new(Tile::P4),
            Tile::new(Tile::S6),
            Tile::new(Tile::M3),
            Tile::new(Tile::P7),
        ];
        let config = test_config();

        let terminal = evaluate_safety(Tile::new(Tile::M1), &state, &config);
        let honour = evaluate_safety(Tile::new(Tile::Z1), &state, &config);
        let middle = evaluate_safety(Tile::new(Tile::S2), &state, &config);

        assert!(terminal <= 0.1, "生牌の么九牌は最危険: {terminal}");
        assert!(honour <= 0.1, "生牌の字牌は最危険: {honour}");
        assert!(
            middle > terminal && middle > honour,
            "中張牌は么九牌より安全: {middle}"
        );

        let genbutsu = evaluate_safety(Tile::new(Tile::M5), &state, &config);
        assert_eq!(genbutsu, 1.0);
    }

    #[test]
    fn test_live_dragon_is_deadly_against_dragon_alert() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_melds[1] = vec![pon_meld(Tile::Z5), pon_meld(Tile::Z6)];

        let safety = evaluate_safety(Tile::new(Tile::Z7), &state, &test_config());
        assert!(
            safety <= 0.05,
            "live third dragon should be deadly: {safety}"
        );

        state.all_discards[1].push(Tile::new(Tile::Z7));
        let safety = evaluate_safety(Tile::new(Tile::Z7), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_flush_suit_and_honours_are_dangerous() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_melds[1] = vec![pon_meld(Tile::P2), pon_meld(Tile::P7)];
        let config = test_config();

        let in_suit = evaluate_safety(Tile::new(Tile::P5), &state, &config);
        let off_suit = evaluate_safety(Tile::new(Tile::S5), &state, &config);
        let honour = evaluate_safety(Tile::new(Tile::Z3), &state, &config);

        assert!(
            in_suit < off_suit,
            "染め色は他色より危険: {in_suit} vs {off_suit}"
        );
        assert!(honour < 1.0, "染め手相手の字牌も警戒する");
    }

    #[test]
    fn test_melded_threat_weaker_than_riichi() {
        let melds = vec![pon_meld(Tile::M2), pon_meld(Tile::P5), pon_meld(Tile::S7)];

        let mut melded = CpuGameState::new();
        melded.my_seat_wind = Wind::East;
        melded.player_melds[1] = melds;

        let mut riichi = CpuGameState::new();
        riichi.my_seat_wind = Wind::East;
        riichi.player_riichi[1] = true;

        let config = test_config();
        let vs_melded = evaluate_safety(Tile::new(Tile::S5), &melded, &config);
        let vs_riichi = evaluate_safety(Tile::new(Tile::S5), &riichi, &config);
        assert!(vs_melded > vs_riichi);
        assert!(vs_melded < 1.0);
    }

    #[test]
    fn test_live_yakuhai_more_dangerous_when_strict() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.round_wind = Wind::East;
        state.player_riichi[1] = true;

        let config = test_config(); // Normal enables strict checks.
        let yakuhai = evaluate_safety(Tile::new(Tile::Z5), &state, &config);
        let guest = evaluate_safety(Tile::new(Tile::Z3), &state, &config);
        assert!(
            yakuhai < guest,
            "生牌役牌({yakuhai}) < 客風({guest}) のはず"
        );

        // The weak level keeps the visibility-only estimate.
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        let yakuhai = evaluate_safety(Tile::new(Tile::Z5), &state, &config);
        let guest = evaluate_safety(Tile::new(Tile::Z3), &state, &config);
        assert_eq!(yakuhai, guest);
    }

    #[test]
    fn test_dora_neighbor_more_dangerous_when_strict() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true;
        state.dora_indicators = vec![Tile::new(Tile::M4)]; // dora is 5m

        let config = test_config();
        let near_dora = evaluate_safety(Tile::new(Tile::M5), &state, &config);
        let plain = evaluate_safety(Tile::new(Tile::S5), &state, &config);
        assert!(near_dora < plain);
    }

    #[test]
    fn test_one_chance_safer_when_strict() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true;
        // Three visible 4s alone is not enough; show three 4s and three
        // 6s so every sequence through 5s is one-chance.
        state.all_discards[2] = vec![
            Tile::new(Tile::S4),
            Tile::new(Tile::S4),
            Tile::new(Tile::S4),
            Tile::new(Tile::S6),
            Tile::new(Tile::S6),
            Tile::new(Tile::S6),
        ];

        let config = test_config();
        let one_chance = evaluate_safety(Tile::new(Tile::S5), &state, &config);
        let plain = evaluate_safety(Tile::new(Tile::M5), &state, &config);
        assert!(one_chance > plain);
    }

    // --- evaluate_safety (public) ---

    #[test]
    fn test_evaluate_safety_no_riichi_returns_1() {
        let state = CpuGameState::new();
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_skips_self() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[0] = true; // our own riichi
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_genbutsu_riichi_opponent() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true;
        state.all_discards[1] = vec![Tile::new(Tile::M5)];
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_uses_opponent_seat_wind_for_yakuhai() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.round_wind = Wind::West;
        state.player_riichi[1] = true;

        let opponent_seat_wind = evaluate_safety(Tile::new(Tile::Z2), &state, &test_config());
        let my_seat_wind = evaluate_safety(Tile::new(Tile::Z1), &state, &test_config());

        assert!(opponent_seat_wind < my_seat_wind);
    }

    #[test]
    fn test_evaluate_safety_multiple_riichi_takes_min() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        // 5m is genbutsu against South but live against West,
        // so the minimum must win.
        state.player_riichi[1] = true;
        state.player_riichi[2] = true;
        state.all_discards[1] = vec![Tile::new(Tile::M5)];
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert!(safety < 1.0);
    }

    // --- is_suji: remaining num patterns ---

    #[test]
    fn test_suji_3m_when_6m_discarded() {
        let discards = vec![Tile::new(Tile::M6)];
        assert!(is_suji(Tile::M3, &discards));
        assert!(is_suji(Tile::M9, &discards));
    }

    #[test]
    fn test_suji_4m_when_1m_or_7m_discarded() {
        assert!(is_suji(Tile::M4, &[Tile::new(Tile::M1)]));
        assert!(is_suji(Tile::M4, &[Tile::new(Tile::M7)]));
        assert!(!is_suji(Tile::M4, &[Tile::new(Tile::M2)]));
    }

    #[test]
    fn test_suji_5m_when_2m_or_8m_discarded() {
        assert!(is_suji(Tile::M5, &[Tile::new(Tile::M2)]));
        assert!(is_suji(Tile::M5, &[Tile::new(Tile::M8)]));
        assert!(!is_suji(Tile::M5, &[Tile::new(Tile::M1)]));
    }

    #[test]
    fn test_suji_6m_when_3m_or_9m_discarded() {
        assert!(is_suji(Tile::M6, &[Tile::new(Tile::M3)]));
        assert!(is_suji(Tile::M6, &[Tile::new(Tile::M9)]));
        assert!(!is_suji(Tile::M6, &[Tile::new(Tile::M2)]));
    }

    #[test]
    fn test_suji_pin_suit() {
        let discards = vec![Tile::new(Tile::P4)];
        assert!(is_suji(Tile::P1, &discards));
        assert!(is_suji(Tile::P7, &discards));
        assert!(!is_suji(Tile::M1, &discards)); // other suits unaffected
    }

    #[test]
    fn test_suji_sou_suit() {
        let discards = vec![Tile::new(Tile::S5)];
        assert!(is_suji(Tile::S2, &discards));
        assert!(is_suji(Tile::S8, &discards));
    }

    #[test]
    fn test_suji_honour_tile_returns_false() {
        let discards = vec![Tile::new(Tile::M4)];
        assert!(!is_suji(Tile::Z1, &discards));
        assert!(!is_suji(Tile::Z7, &discards));
    }

    #[test]
    fn test_suji_no_partner_in_discards() {
        let discards = vec![Tile::new(Tile::M3)];
        assert!(!is_suji(Tile::M1, &discards)); // 1m's partner is 4m
    }

    // --- evaluate_safety_against_player: base safety values ---

    #[test]
    fn test_suji_safety_value() {
        let discards = vec![Tile::new(Tile::M4)];
        let state = CpuGameState::new();
        let safety = evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state);
        assert_eq!(safety, 0.75);
    }

    #[test]
    fn test_kabe_safety_value() {
        // With all 2m visible, the only sequence through 1m is blocked.
        let mut state = CpuGameState::new();
        state.all_discards[0] = vec![Tile::new(Tile::M2); 4];
        let discards: Vec<Tile> = vec![];
        let safety = evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state);
        assert_eq!(safety, 0.7);
    }

    #[test]
    fn test_end_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state),
            0.4
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M9), &discards, &state),
            0.4
        );
    }

    #[test]
    fn test_near_end_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M2), &discards, &state),
            0.3
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M8), &discards, &state),
            0.3
        );
    }

    #[test]
    fn test_3_7_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M3), &discards, &state),
            0.2
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M7), &discards, &state),
            0.2
        );
    }

    #[test]
    fn test_middle_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = vec![];
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M4), &discards, &state),
            0.15
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M5), &discards, &state),
            0.15
        );
        assert_eq!(
            evaluate_safety_against_player(Tile::new(Tile::M6), &discards, &state),
            0.15
        );
    }

    #[test]
    fn test_honour_tile_visible_counts() {
        let discards: Vec<Tile> = vec![];
        {
            let state = CpuGameState::new();
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.3
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1)];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.4
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1); 2];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.6
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1); 3];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                0.95
            );
        }
        {
            let mut state = CpuGameState::new();
            state.my_hand = vec![Tile::new(Tile::Z1); 4];
            assert_eq!(
                evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state),
                1.0
            );
        }
    }

    // --- is_kabe ---

    #[test]
    fn test_kabe_1m_when_2m_exhausted() {
        let mut counts = [0u8; 34];
        counts[Tile::M2 as usize] = 4;
        assert!(is_kabe(Tile::M1, &counts));
    }

    #[test]
    fn test_kabe_1m_not_blocked() {
        let counts = [0u8; 34];
        assert!(!is_kabe(Tile::M1, &counts));
    }

    #[test]
    fn test_kabe_9m_when_8m_exhausted() {
        let mut counts = [0u8; 34];
        counts[Tile::M8 as usize] = 4;
        assert!(is_kabe(Tile::M9, &counts));
    }

    #[test]
    fn test_kabe_2m_fully_blocked() {
        let mut counts = [0u8; 34];
        counts[Tile::M3 as usize] = 4;
        assert!(is_kabe(Tile::M2, &counts));
    }

    #[test]
    fn test_kabe_2m_partially_blocked() {
        // Only one of 2m's two sequences is blocked, so no wall.
        let mut counts = [0u8; 34];
        counts[Tile::M1 as usize] = 4;
        assert!(!is_kabe(Tile::M2, &counts));
    }

    #[test]
    fn test_kabe_8m_fully_blocked() {
        let mut counts = [0u8; 34];
        counts[Tile::M7 as usize] = 4;
        assert!(is_kabe(Tile::M8, &counts));
    }

    #[test]
    fn test_kabe_middle_5m_fully_blocked() {
        let mut counts = [0u8; 34];
        counts[Tile::M4 as usize] = 4;
        counts[Tile::M6 as usize] = 4;
        assert!(is_kabe(Tile::M5, &counts));
    }

    #[test]
    fn test_kabe_middle_5m_partially_blocked() {
        // 567m stays open, so no wall.
        let mut counts = [0u8; 34];
        counts[Tile::M4 as usize] = 4;
        assert!(!is_kabe(Tile::M5, &counts));
    }

    #[test]
    fn test_kabe_pin_suit() {
        let mut counts = [0u8; 34];
        counts[Tile::P2 as usize] = 4;
        assert!(is_kabe(Tile::P1, &counts));
    }

    #[test]
    fn test_kabe_honour_tile_returns_false() {
        let mut counts = [0u8; 34];
        counts[Tile::Z1 as usize] = 4;
        assert!(!is_kabe(Tile::Z1, &counts));
    }
}
