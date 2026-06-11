//! 守備ロジック
//!
//! 牌の安全度を評価する。現物・筋・壁・字牌・端牌の判定に加え、
//! 他家の脅威（リーチ・副露・染め手・役満気配）を統合的に扱う。

use mahjong_core::tile::{Tile, TileType, dora_indicator_to_dora};

use super::client::{CpuConfig, CpuLevel, is_yakuhai};
use super::state::CpuGameState;

/// 他家1人分の脅威情報
#[derive(Debug, Clone, Default)]
pub struct Threat {
    /// 警戒の強さ（リーチ=1.0、副露による聴牌気配はそれより弱い）
    pub weight: f64,
    /// 染め手気配の色（0=萬子, 1=筒子, 2=索子）。その色と字牌を危険視する
    pub flush_suit: Option<usize>,
    /// 大三元気配（三元牌を2種類以上鳴いている）
    pub dragon_alert: bool,
    /// 四喜和気配（風牌を2種類以上鳴いている）
    pub wind_alert: bool,
    /// 国士無双気配（門前のまま序盤から中張牌中心に切っている）
    pub kokushi_alert: bool,
}

/// 牌の安全度を評価する（0.0=最危険, 1.0=最安全)
///
/// 全ての他家の脅威に対する安全度を評価し、最小値を返す。
/// 定石無効時はリーチ者のみを脅威として扱う（従来動作）。
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

        let safety =
            evaluate_safety_against_threat(tile, &state.all_discards[i], state, &threat, strict);
        min_safety = min_safety.min(safety);
    }

    min_safety
}

/// 他家1人の脅威を判定する
///
/// 脅威がなければ `None`。
/// - リーチ者は常に最大の脅威（重み1.0）
/// - #180（弱以上）: 3副露以上は聴牌濃厚として警戒する
/// - #181（中以上）: 2副露以上が1色+字牌に染まっていれば染め手気配
/// - #182（弱以上）: 三元牌2種以上 / 風牌2種以上の副露は役満気配
/// - #182（中以上）: 門前のまま序盤から中張牌中心に切っている相手は
///   国士無双（またはチャンタ系）気配
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

    // 定石無効時はリーチ者のみを脅威とする（従来動作）
    if !config.heuristics_enabled {
        return None;
    }

    let melds = &state.player_melds[idx];
    let mut threat = Threat::default();

    // #180: 3副露以上は聴牌濃厚
    if melds.len() >= 3 {
        threat.weight = 0.7;
    }

    // #182: 役満気配（弱以上）
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

    // #182拡張: 国士無双気配（中以上）
    // 国士無双は門前限定で、不要な中張牌を切り続ける河になる。
    // 么九牌・字牌は対子の余剰分（2枚目以降の重なり）が切られうるので、
    // 「序盤6巡で么九牌1枚以下 + 河全体で2枚以下」を気配とする。
    // 3枚以上切り出したら見切ったとみなして解除する。
    // 同じ河はチャンタ・混老頭系でもありえるが、いずれにせよ
    // 么九牌・字牌が危険という結論は変わらない。
    if config.level >= CpuLevel::Normal && melds.is_empty() {
        let discards = &state.all_discards[idx];
        if discards.len() >= 5 {
            let early_orphans = discards.iter().take(6).filter(|t| t.is_1_9_honor()).count();
            let total_orphans = discards.iter().filter(|t| t.is_1_9_honor()).count();
            if early_orphans <= 1 && total_orphans <= 2 {
                threat.kokushi_alert = true;
                threat.weight = threat.weight.max(0.5);
            }
        }
    }

    // #181: 染め手気配（中以上）: 2副露以上が全て1色+字牌
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

/// 場に公開されている枚数（自分の手牌・ツモ牌を除く）を数える
///
/// 「生牌」（他家から見えていない牌）の判定に使用する。
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

/// 特定のプレイヤーに対する牌の安全度を評価する（リーチ脅威・従来動作）
///
/// 既存呼び出し・テストとの互換用ラッパー。
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
    evaluate_safety_against_threat(tile, opponent_discards, state, &threat, false)
}

/// 特定の脅威に対する牌の安全度を評価する
///
/// `strict` は中以上の追加評価（#175 ワンチャンス, #177 生牌役牌・ドラそば）を
/// 有効にする。
fn evaluate_safety_against_threat(
    tile: Tile,
    opponent_discards: &[Tile],
    state: &CpuGameState,
    threat: &Threat,
    strict: bool,
) -> f64 {
    let tt = tile.get();

    // 1. 現物: 相手の捨て牌に同じ牌がある → 脅威の種類によらず完全安全
    if opponent_discards.iter().any(|d| d.get() == tt) {
        return 1.0;
    }

    // 2. 役満気配の生牌（#182）: 重みによらず最危険として扱う
    if tt >= 27 && publicly_visible(state, tt) == 0 {
        if threat.dragon_alert && (Tile::Z5..=Tile::Z7).contains(&tt) {
            return 0.05;
        }
        if threat.wind_alert && (Tile::Z1..=Tile::Z4).contains(&tt) {
            return 0.05;
        }
    }

    // 国士無双気配（#182拡張）: 么九牌・字牌は重みによらず危険として扱う
    // （国士は么九牌13種のどれでも待ちになりうる。生牌は特に危険）
    if threat.kokushi_alert && tile.is_1_9_honor() {
        return if publicly_visible(state, tt) == 0 {
            0.08
        } else {
            0.2
        };
    }

    let visible_counts = state.visible_tile_counts();
    let mut base: f64;

    if tt >= 27 {
        // 3. 字牌の安全度（見え枚数ベース）
        let visible = visible_counts[tt as usize];
        base = match visible {
            4 => 1.0,  // 全部見えている（ありえないが念のため）
            3 => 0.95, // 残り1枚 → ほぼ安全
            2 => 0.6,  // 残り2枚
            1 => 0.4,  // 残り3枚
            _ => 0.3,  // 1枚も見えていない
        };

        // #177（中以上）: 生牌の役牌は危険度を上げる
        if strict
            && is_yakuhai(tt, state.my_seat_wind, state.prevailing_wind)
            && publicly_visible(state, tt) == 0
        {
            base = base.min(0.22);
        }
    } else if is_suji(tt, opponent_discards) {
        // 4. 筋（suji）判定
        base = 0.75;
    } else if is_kabe(tt, &visible_counts) {
        // 5. 壁（ノーチャンス）判定
        base = 0.7;
    } else {
        // 6. 端牌 vs 中張牌
        let num = tt % 9;
        base = match num {
            0 | 8 => 0.4, // 1, 9
            1 | 7 => 0.3, // 2, 8
            2 | 6 => 0.2, // 3, 7
            _ => 0.15,    // 4, 5, 6
        };

        // #175（中以上）: ワンチャンス（順子の材料が残り1枚以下）はやや安全寄り
        if strict && is_one_chance(tt, &visible_counts) {
            base = base.max(0.5);
        }
    }

    // #177（中以上）: ドラ・ドラそばは相手の手に絡みやすく危険
    if strict && tt < 27 && is_dora_or_neighbor(tt, state) {
        base = (base - 0.08).max(0.05);
    }

    // #181: 染め手気配の色と字牌は危険度を上げる
    if let Some(suit) = threat.flush_suit
        && (tt >= 27 || (tt / 9) as usize == suit)
    {
        base *= 0.5;
    }

    // 脅威の重みでスケール（弱い脅威ほど危険度を割り引く）
    if threat.weight >= 1.0 {
        base
    } else {
        1.0 - (1.0 - base) * threat.weight
    }
}

/// ドラまたはドラの隣（同色±1）か
fn is_dora_or_neighbor(tile_type: TileType, state: &CpuGameState) -> bool {
    for indicator in &state.dora_indicators {
        let dora = dora_indicator_to_dora(indicator.get());
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

/// 筋（suji）で安全かどうか判定する
///
/// 例: 相手が4mを捨てている → 1m, 7m は筋で比較的安全
///     相手が5mを捨てている → 2m, 8m は筋
///     相手が6mを捨てている → 3m, 9m は筋
fn is_suji(tile_type: TileType, opponent_discards: &[Tile]) -> bool {
    if tile_type >= 27 {
        return false; // 字牌に筋はない
    }

    let suit_start = (tile_type / 9) * 9;
    let num = tile_type - suit_start; // 0-8

    // 筋のペア: (1,4), (2,5), (3,6), (4,7), (5,8), (6,9)
    // numは0-indexed: (0,3), (1,4), (2,5), (3,6), (4,7), (5,8)
    let suji_partner = match num {
        0 => Some(suit_start + 3), // 1 → 4
        1 => Some(suit_start + 4), // 2 → 5
        2 => Some(suit_start + 5), // 3 → 6
        3 => {
            // 4 → 1 or 7
            if opponent_discards.iter().any(|d| d.get() == suit_start)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 6)
            {
                return true;
            }
            return false;
        }
        4 => {
            // 5 → 2 or 8
            if opponent_discards.iter().any(|d| d.get() == suit_start + 1)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 7)
            {
                return true;
            }
            return false;
        }
        5 => {
            // 6 → 3 or 9
            if opponent_discards.iter().any(|d| d.get() == suit_start + 2)
                || opponent_discards.iter().any(|d| d.get() == suit_start + 8)
            {
                return true;
            }
            return false;
        }
        6 => Some(suit_start + 3), // 7 → 4
        7 => Some(suit_start + 4), // 8 → 5
        8 => Some(suit_start + 5), // 9 → 6
        _ => None,
    };

    if let Some(partner) = suji_partner {
        opponent_discards.iter().any(|d| d.get() == partner)
    } else {
        false
    }
}

/// 壁（kabe）で安全かどうか判定する
///
/// ある牌種が場に全て見えている（残り0枚）場合、
/// その牌を含む順子が成立しないため、隣接牌の危険度が下がる。
fn is_kabe(tile_type: TileType, visible_counts: &[u8; 34]) -> bool {
    is_blocked(tile_type, visible_counts, 4)
}

/// ワンチャンス（順子の材料が残り1枚以下）か（#175）
///
/// 壁ほどではないが、両面待ちで当たる可能性が低い。
fn is_one_chance(tile_type: TileType, visible_counts: &[u8; 34]) -> bool {
    is_blocked(tile_type, visible_counts, 3)
}

/// 順子の材料が min_visible 枚以上見えていて成立しにくいか（壁判定の一般化）
///
/// min_visible=4 でノーチャンス（壁）、3 でワンチャンス相当になる。
fn is_blocked(tile_type: TileType, visible_counts: &[u8; 34], min_visible: u8) -> bool {
    if tile_type >= 27 {
        return false; // 字牌に壁はない
    }

    let suit_start = (tile_type / 9) * 9;
    let num = tile_type - suit_start; // 0-8

    // この牌を含みうる順子の構成牌を確認
    // 例: 5m(num=4) → 345m, 456m, 567m の構成牌 3,4,6,7 のいずれかが壁なら安全寄り
    let mut blocked_patterns = 0;
    let total_patterns;

    match num {
        0 => {
            // 1: 123 のみ。2か3が壁なら安全
            total_patterns = 1;
            if visible_counts[(suit_start + 1) as usize] >= min_visible
                || visible_counts[(suit_start + 2) as usize] >= min_visible
            {
                blocked_patterns = 1;
            }
        }
        1 => {
            // 2: 123, 234。
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
            // 8: 789, 678
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
            // 9: 789 のみ。7か8が壁なら安全
            total_patterns = 1;
            if visible_counts[(suit_start + 6) as usize] >= min_visible
                || visible_counts[(suit_start + 7) as usize] >= min_visible
            {
                blocked_patterns = 1;
            }
        }
        _ => {
            // 3-7: 3パターン
            total_patterns = 3;
            // 前方の順子
            if num >= 2
                && (visible_counts[(suit_start + num - 2) as usize] >= min_visible
                    || visible_counts[(suit_start + num - 1) as usize] >= min_visible)
            {
                blocked_patterns += 1;
            }
            // 中央の順子
            if (1..=7).contains(&num)
                && (visible_counts[(suit_start + num - 1) as usize] >= min_visible
                    || visible_counts[(suit_start + num + 1) as usize] >= min_visible)
            {
                blocked_patterns += 1;
            }
            // 後方の順子
            if num <= 6
                && (visible_counts[(suit_start + num + 1) as usize] >= min_visible
                    || visible_counts[(suit_start + num + 2) as usize] >= min_visible)
            {
                blocked_patterns += 1;
            }
        }
    }

    // 全パターンが壁でブロックされていれば安全
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
        // 4mが捨てられている → 1m は筋で安全
        let discards = vec![Tile::new(Tile::M4)];
        assert!(is_suji(Tile::M1, &discards));
        assert!(is_suji(Tile::M7, &discards));
        assert!(!is_suji(Tile::M5, &discards));
    }

    #[test]
    fn test_suji_middle() {
        // 5mが捨てられている → 2m, 8m は筋
        let discards = vec![Tile::new(Tile::M5)];
        assert!(is_suji(Tile::M2, &discards));
        assert!(is_suji(Tile::M8, &discards));
    }

    #[test]
    fn test_honor_tile_safety() {
        let state = CpuGameState::new();
        let discards: Vec<Tile> = Vec::new();
        // 字牌で見えていない → 低い安全度
        let safety = evaluate_safety_against_player(Tile::new(Tile::Z1), &discards, &state);
        assert!(safety < 0.5);
    }

    // --- 脅威モデル（#175 #177 #180 #181 #182）---

    #[test]
    fn test_assess_threat_riichi_is_full_weight() {
        let mut state = CpuGameState::new();
        state.player_riichi[1] = true;
        let threat = assess_threat(&state, 1, &test_config()).expect("riichi is a threat");
        assert_eq!(threat.weight, 1.0);
    }

    #[test]
    fn test_assess_threat_three_melds() {
        // #180: 3副露以上は聴牌濃厚（リーチより弱い警戒）
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::M2), pon_meld(Tile::P5), pon_meld(Tile::S7)];

        let threat = assess_threat(&state, 1, &test_config()).expect("3 melds is a threat");
        assert!(threat.weight > 0.0 && threat.weight < 1.0);

        // 定石無効ならリーチ者以外は脅威としない（従来動作）
        let config = test_config().without_heuristics();
        assert!(assess_threat(&state, 1, &config).is_none());

        // 2副露（異色）では脅威としない
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::M2), pon_meld(Tile::P5)];
        assert!(assess_threat(&state, 1, &test_config()).is_none());
    }

    #[test]
    fn test_assess_threat_flush_signs() {
        // #181: 2副露が1色に染まっていれば染め手気配（中以上）
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::P2), pon_meld(Tile::P7)];

        let threat = assess_threat(&state, 1, &test_config()).expect("flush signs");
        assert_eq!(threat.flush_suit, Some(1)); // 筒子

        // 弱レベルは染め手気配を見ない
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        assert!(assess_threat(&state, 1, &config).is_none());
    }

    #[test]
    fn test_assess_threat_yakuman_signs() {
        // #182: 三元牌2種のポンは大三元気配（弱以上）
        let mut state = CpuGameState::new();
        state.player_melds[1] = vec![pon_meld(Tile::Z5), pon_meld(Tile::Z6)];

        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        let threat = assess_threat(&state, 1, &config).expect("dragon signs");
        assert!(threat.dragon_alert);

        // 風牌2種のポンは四喜和気配
        let mut state = CpuGameState::new();
        state.player_melds[2] = vec![pon_meld(Tile::Z1), pon_meld(Tile::Z2)];
        let threat = assess_threat(&state, 2, &config).expect("wind signs");
        assert!(threat.wind_alert);
    }

    #[test]
    fn test_assess_threat_kokushi_signs() {
        // #182拡張: 門前で序盤から中張牌中心の河は国士無双気配（中以上）
        let middle_discards = |n: usize| -> Vec<Tile> {
            [Tile::M5, Tile::P4, Tile::S6, Tile::M3, Tile::P7, Tile::S5]
                .iter()
                .take(n)
                .map(|&t| Tile::new(t))
                .collect()
        };

        // 中張牌6枚の河 → 気配あり
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        let threat = assess_threat(&state, 1, &test_config()).expect("kokushi signs");
        assert!(threat.kokushi_alert);

        // 么九牌の重なり（余剰）が1〜2枚混ざっていても気配は維持
        let mut state = CpuGameState::new();
        let mut discards = middle_discards(5);
        discards.push(Tile::new(Tile::M1)); // 6枚目に余剰の么九牌
        discards.extend(middle_discards(3));
        discards.push(Tile::new(Tile::Z2)); // 河全体で么九牌2枚
        state.all_discards[1] = discards;
        let threat = assess_threat(&state, 1, &test_config()).expect("kokushi signs");
        assert!(threat.kokushi_alert);

        // 序盤に么九牌を2枚以上切っている → 通常の手（気配なし）
        let mut state = CpuGameState::new();
        let mut discards = vec![Tile::new(Tile::Z3), Tile::new(Tile::M9)];
        discards.extend(middle_discards(4));
        state.all_discards[1] = discards;
        assert!(assess_threat(&state, 1, &test_config()).is_none());

        // 河全体で么九牌3枚以上 → 見切ったとみなして解除
        let mut state = CpuGameState::new();
        let mut discards = middle_discards(6);
        discards.extend([
            Tile::new(Tile::M1),
            Tile::new(Tile::P9),
            Tile::new(Tile::Z1),
        ]);
        state.all_discards[1] = discards;
        assert!(assess_threat(&state, 1, &test_config()).is_none());

        // 捨て牌4枚以下では判定しない
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(4);
        assert!(assess_threat(&state, 1, &test_config()).is_none());

        // 副露があれば国士無双ではない
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        state.player_melds[1] = vec![pon_meld(Tile::P5)];
        let threat = assess_threat(&state, 1, &test_config());
        assert!(threat.is_none_or(|t| !t.kokushi_alert));

        // 弱レベルは対象外
        let mut state = CpuGameState::new();
        state.all_discards[1] = middle_discards(6);
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        assert!(assess_threat(&state, 1, &config).is_none());
    }

    #[test]
    fn test_orphans_dangerous_against_kokushi_suspect() {
        // 国士無双気配の相手に么九牌・字牌は危険、中張牌は比較的安全
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
        let honor = evaluate_safety(Tile::new(Tile::Z1), &state, &config);
        let middle = evaluate_safety(Tile::new(Tile::S2), &state, &config);

        assert!(terminal <= 0.1, "生牌の么九牌は最危険: {terminal}");
        assert!(honor <= 0.1, "生牌の字牌は最危険: {honor}");
        assert!(
            middle > terminal && middle > honor,
            "中張牌は么九牌より安全: {middle}"
        );

        // 相手の現物（中張牌）は安全
        let genbutsu = evaluate_safety(Tile::new(Tile::M5), &state, &config);
        assert_eq!(genbutsu, 1.0);
    }

    #[test]
    fn test_live_dragon_is_deadly_against_dragon_alert() {
        // #182: 大三元気配の相手に生牌の3種類目の三元牌は切らない
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_melds[1] = vec![pon_meld(Tile::Z5), pon_meld(Tile::Z6)];

        // 生牌の中（Z7）は最危険
        let safety = evaluate_safety(Tile::new(Tile::Z7), &state, &test_config());
        assert!(
            safety <= 0.05,
            "live third dragon should be deadly: {safety}"
        );

        // 相手の現物になっている三元牌は安全
        state.all_discards[1].push(Tile::new(Tile::Z7));
        let safety = evaluate_safety(Tile::new(Tile::Z7), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_flush_suit_and_honors_are_dangerous() {
        // #181: 染め手気配の色と字牌は他の色より危険
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_melds[1] = vec![pon_meld(Tile::P2), pon_meld(Tile::P7)];
        let config = test_config();

        let in_suit = evaluate_safety(Tile::new(Tile::P5), &state, &config);
        let off_suit = evaluate_safety(Tile::new(Tile::S5), &state, &config);
        let honor = evaluate_safety(Tile::new(Tile::Z3), &state, &config);

        assert!(
            in_suit < off_suit,
            "染め色は他色より危険: {in_suit} vs {off_suit}"
        );
        assert!(honor < 1.0, "染め手相手の字牌も警戒する");
    }

    #[test]
    fn test_melded_threat_weaker_than_riichi() {
        // #180: 3副露の警戒はリーチより弱い（同じ牌でも安全度が高い）
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
        // #177: 生牌の役牌（白）は中以上では客風より危険
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.prevailing_wind = Wind::East;
        state.player_riichi[1] = true;

        let config = test_config(); // Normal → strict
        let yakuhai = evaluate_safety(Tile::new(Tile::Z5), &state, &config);
        let guest = evaluate_safety(Tile::new(Tile::Z3), &state, &config);
        assert!(
            yakuhai < guest,
            "生牌役牌({yakuhai}) < 客風({guest}) のはず"
        );

        // 弱レベルでは従来の見え枚数評価のみ
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        let yakuhai = evaluate_safety(Tile::new(Tile::Z5), &state, &config);
        let guest = evaluate_safety(Tile::new(Tile::Z3), &state, &config);
        assert_eq!(yakuhai, guest);
    }

    #[test]
    fn test_dora_neighbor_more_dangerous_when_strict() {
        // #177: ドラそばは同条件の牌より危険
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true;
        state.dora_indicators = vec![Tile::new(Tile::M4)]; // ドラは M5

        let config = test_config();
        let near_dora = evaluate_safety(Tile::new(Tile::M5), &state, &config);
        let plain = evaluate_safety(Tile::new(Tile::S5), &state, &config);
        assert!(near_dora < plain);
    }

    #[test]
    fn test_one_chance_safer_when_strict() {
        // #175: ワンチャンス（順子材料が残り1枚）は無筋中張牌より安全寄り
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true;
        // S4 が3枚見えている → S5 の 345/456 パターンが薄い…だけでは不十分なので
        // S4×3 + S6×3 を見せて S5 をワンチャンス級にする
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
        // リーチ者がいなければ常に安全度 1.0
        let state = CpuGameState::new();
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_skips_self() {
        // 自分自身のリーチはスキップされる
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[0] = true; // 東（自分）がリーチ
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_genbutsu_riichi_opponent() {
        // リーチ者の現物は安全度 1.0
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true; // 南がリーチ
        state.all_discards[1] = vec![Tile::new(Tile::M5)];
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert_eq!(safety, 1.0);
    }

    #[test]
    fn test_evaluate_safety_multiple_riichi_takes_min() {
        // 複数リーチ者がいる場合、最小の安全度を返す
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.player_riichi[1] = true; // 南がリーチ: M5は現物 → 1.0
        state.player_riichi[2] = true; // 西がリーチ: M5は非現物 → 低い安全度
        state.all_discards[1] = vec![Tile::new(Tile::M5)];
        let safety = evaluate_safety(Tile::new(Tile::M5), &state, &test_config());
        assert!(safety < 1.0);
    }

    // --- is_suji: 未カバーの num パターン ---

    #[test]
    fn test_suji_3m_when_6m_discarded() {
        // 6m捨て → 3m, 9m は筋
        let discards = vec![Tile::new(Tile::M6)];
        assert!(is_suji(Tile::M3, &discards));
        assert!(is_suji(Tile::M9, &discards));
    }

    #[test]
    fn test_suji_4m_when_1m_or_7m_discarded() {
        // 1m または 7m が捨てられている → 4m は筋
        assert!(is_suji(Tile::M4, &[Tile::new(Tile::M1)]));
        assert!(is_suji(Tile::M4, &[Tile::new(Tile::M7)]));
        assert!(!is_suji(Tile::M4, &[Tile::new(Tile::M2)]));
    }

    #[test]
    fn test_suji_5m_when_2m_or_8m_discarded() {
        // 2m または 8m が捨てられている → 5m は筋
        assert!(is_suji(Tile::M5, &[Tile::new(Tile::M2)]));
        assert!(is_suji(Tile::M5, &[Tile::new(Tile::M8)]));
        assert!(!is_suji(Tile::M5, &[Tile::new(Tile::M1)]));
    }

    #[test]
    fn test_suji_6m_when_3m_or_9m_discarded() {
        // 3m または 9m が捨てられている → 6m は筋
        assert!(is_suji(Tile::M6, &[Tile::new(Tile::M3)]));
        assert!(is_suji(Tile::M6, &[Tile::new(Tile::M9)]));
        assert!(!is_suji(Tile::M6, &[Tile::new(Tile::M2)]));
    }

    #[test]
    fn test_suji_pin_suit() {
        // 筋は別の色でも成立し、異なる色の捨て牌は無効
        let discards = vec![Tile::new(Tile::P4)];
        assert!(is_suji(Tile::P1, &discards));
        assert!(is_suji(Tile::P7, &discards));
        assert!(!is_suji(Tile::M1, &discards)); // 万子には無関係
    }

    #[test]
    fn test_suji_sou_suit() {
        // 索子でも同様
        let discards = vec![Tile::new(Tile::S5)];
        assert!(is_suji(Tile::S2, &discards));
        assert!(is_suji(Tile::S8, &discards));
    }

    #[test]
    fn test_suji_honor_tile_returns_false() {
        // 字牌に筋はない
        let discards = vec![Tile::new(Tile::M4)];
        assert!(!is_suji(Tile::Z1, &discards));
        assert!(!is_suji(Tile::Z7, &discards));
    }

    #[test]
    fn test_suji_no_partner_in_discards() {
        // 筋パートナーが捨てられていなければ false
        let discards = vec![Tile::new(Tile::M3)];
        assert!(!is_suji(Tile::M1, &discards)); // M1 のパートナーは M4
    }

    // --- evaluate_safety_against_player: 各安全度の返り値 ---

    #[test]
    fn test_suji_safety_value() {
        // 筋牌の安全度は 0.75
        let discards = vec![Tile::new(Tile::M4)];
        let state = CpuGameState::new();
        let safety = evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state);
        assert_eq!(safety, 0.75);
    }

    #[test]
    fn test_kabe_safety_value() {
        // 壁牌の安全度は 0.70
        // 2m が4枚見えている → 1m を含む唯一の順子(123m)がブロック
        let mut state = CpuGameState::new();
        state.all_discards[0] = vec![Tile::new(Tile::M2); 4];
        let discards: Vec<Tile> = vec![];
        let safety = evaluate_safety_against_player(Tile::new(Tile::M1), &discards, &state);
        assert_eq!(safety, 0.7);
    }

    #[test]
    fn test_end_tile_safety() {
        // 1m / 9m → 0.4
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
        // 2m / 8m → 0.3
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
        // 3m / 7m → 0.2
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
        // 4m / 5m / 6m → 0.15
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
    fn test_honor_tile_visible_counts() {
        // 字牌の見え枚数に応じた安全度
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
        // 2m が4枚見えている → 1m の唯一の順子(123m)がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M2 as usize] = 4;
        assert!(is_kabe(Tile::M1, &counts));
    }

    #[test]
    fn test_kabe_1m_not_blocked() {
        // 壁牌がなければ false
        let counts = [0u8; 34];
        assert!(!is_kabe(Tile::M1, &counts));
    }

    #[test]
    fn test_kabe_9m_when_8m_exhausted() {
        // 8m が4枚見えている → 9m の唯一の順子(789m)がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M8 as usize] = 4;
        assert!(is_kabe(Tile::M9, &counts));
    }

    #[test]
    fn test_kabe_2m_fully_blocked() {
        // 3m が4枚見えている → 123m と 234m の両方がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M3 as usize] = 4;
        assert!(is_kabe(Tile::M2, &counts));
    }

    #[test]
    fn test_kabe_2m_partially_blocked() {
        // 1m のみ4枚 → 123m はブロックされるが 234m はブロックされない → false
        let mut counts = [0u8; 34];
        counts[Tile::M1 as usize] = 4;
        assert!(!is_kabe(Tile::M2, &counts));
    }

    #[test]
    fn test_kabe_8m_fully_blocked() {
        // 7m が4枚見えている → 789m と 678m の両方がブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M7 as usize] = 4;
        assert!(is_kabe(Tile::M8, &counts));
    }

    #[test]
    fn test_kabe_middle_5m_fully_blocked() {
        // 4m + 6m が4枚見えている → 345m / 456m / 567m 全てブロック → 壁
        let mut counts = [0u8; 34];
        counts[Tile::M4 as usize] = 4;
        counts[Tile::M6 as usize] = 4;
        assert!(is_kabe(Tile::M5, &counts));
    }

    #[test]
    fn test_kabe_middle_5m_partially_blocked() {
        // 4m のみ4枚 → 345m と 456m はブロックされるが 567m はブロックされない → false
        let mut counts = [0u8; 34];
        counts[Tile::M4 as usize] = 4;
        assert!(!is_kabe(Tile::M5, &counts));
    }

    #[test]
    fn test_kabe_pin_suit() {
        // 別のスートでも壁判定が成立する
        let mut counts = [0u8; 34];
        counts[Tile::P2 as usize] = 4;
        assert!(is_kabe(Tile::P1, &counts));
    }

    #[test]
    fn test_kabe_honor_tile_returns_false() {
        // 字牌に壁はない
        let mut counts = [0u8; 34];
        counts[Tile::Z1 as usize] = 4;
        assert!(!is_kabe(Tile::Z1, &counts));
    }
}
