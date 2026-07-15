use anyhow::Result;

use crate::hand::Hand;
use crate::hand_info::block::BlockProperty;
use crate::hand_info::hand_analyzer::*;
use crate::hand_info::status::*;
use crate::settings::*;
use crate::tile::{Dragon, Tile, Wind};
use crate::winning_hand::name::*;

fn double_yakuman_han(settings: &Settings) -> u32 {
    if settings.double_yakuman { 26 } else { 13 }
}

fn is_thirteen_orphans_thirteen_wait(hand: &Hand) -> bool {
    if !hand.melds().is_empty() || hand.tiles().len() != 13 {
        return false;
    }

    let mut seen = [false; Tile::LEN];
    for tile in hand.tiles() {
        let index = tile.get() as usize;
        if !tile.is_1_9_honour() || seen[index] {
            return false;
        }
        seen[index] = true;
    }

    hand.drawn()
        .is_some_and(|tile| tile.is_1_9_honour() && seen[tile.get() as usize])
}

/// Thirteen Orphans (Kokushi Musō / 国士無双)
pub fn check_thirteen_orphans(
    hand_analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ThirteenOrphans,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if hand_analyzer.form == Form::ThirteenOrphans
        && !(settings.double_yakuman && is_thirteen_orphans_thirteen_wait(hand))
    {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}

/// Thirteen Orphans on a 13-sided wait (Kokushi Musō jūsanmen machi /
/// 国士無双十三面待ち)
pub fn check_thirteen_orphans_thirteen_wait(
    hand_analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::ThirteenOrphansThirteenWait,
        status.has_claimed_open,
        settings.display_lang,
    );
    if settings.double_yakuman
        && hand_analyzer.shanten.has_won()
        && hand_analyzer.form == Form::ThirteenOrphans
        && is_thirteen_orphans_thirteen_wait(hand)
    {
        Ok((name, true, 26))
    } else {
        Ok((name, false, 0))
    }
}
fn is_four_concealed_triplets_pair_wait(hand_analyzer: &HandAnalyzer, hand: &Hand) -> bool {
    if let Some(placement) = hand_analyzer.winning_tile_placement {
        return placement == WinningTilePlacement::Pair;
    }

    hand.drawn().is_some_and(|winning_tile| {
        hand_analyzer
            .same2
            .iter()
            .any(|pair| pair.get()[0] == winning_tile.get())
    })
}

/// Four Concealed Triplets (Sūankō / 四暗刻)
pub fn check_four_concealed_triplets(
    hand_analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::FourConcealedTriplets,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open
        || hand_analyzer.same3.len() != 4
        || is_four_concealed_triplets_pair_wait(hand_analyzer, hand)
    {
        return Ok((name, false, 0));
    }

    if status.is_self_drawn {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}

/// Four Concealed Triplets with a pair wait (Sūankō tanki / 四暗刻単騎待ち)
pub fn check_four_concealed_triplets_pair_wait(
    hand_analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::FourConcealedTripletsPairWait,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.has_claimed_open || hand_analyzer.same3.len() != 4 {
        return Ok((name, false, 0));
    }

    if is_four_concealed_triplets_pair_wait(hand_analyzer, hand) {
        Ok((name, true, double_yakuman_han(settings)))
    } else {
        Ok((name, false, 0))
    }
}
/// Big Dragons (Daisangen / 大三元)
pub fn check_big_dragons(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::BigDragons,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    let mut dragon_count = 0;
    for same in &hand_analyzer.same3 {
        if same.has_dragon(Dragon::White)?
            || same.has_dragon(Dragon::Green)?
            || same.has_dragon(Dragon::Red)?
        {
            dragon_count += 1;
        }
    }
    if dragon_count == 3 {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// Little Winds (Shōsūshii / 小四喜)
pub fn check_little_winds(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::LittleWinds,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Three wind triplets plus a wind pair.
    let mut wind_triplet_count = 0;
    let mut wind_pair = false;
    for same in &hand_analyzer.same3 {
        if same.has_wind(Wind::East)?
            || same.has_wind(Wind::South)?
            || same.has_wind(Wind::West)?
            || same.has_wind(Wind::North)?
        {
            wind_triplet_count += 1;
        }
    }
    for head in &hand_analyzer.same2 {
        if head.has_wind(Wind::East)?
            || head.has_wind(Wind::South)?
            || head.has_wind(Wind::West)?
            || head.has_wind(Wind::North)?
        {
            wind_pair = true;
        }
    }
    if wind_triplet_count == 3 && wind_pair {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// Big Winds (Daisūshii / 大四喜)
pub fn check_big_winds(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::BigWinds,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    let mut wind_triplet_count = 0;
    for same in &hand_analyzer.same3 {
        if same.has_wind(Wind::East)?
            || same.has_wind(Wind::South)?
            || same.has_wind(Wind::West)?
            || same.has_wind(Wind::North)?
        {
            wind_triplet_count += 1;
        }
    }
    if wind_triplet_count == 4 {
        Ok((name, true, double_yakuman_han(settings)))
    } else {
        Ok((name, false, 0))
    }
}
/// All Honours (Tsūiisō / 字一色)
pub fn check_all_honours(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllHonours,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Thirteen Orphans intentionally has no block decomposition. Without
    // this guard, the loops below are empty and every tile appears honour.
    if hand_analyzer.form == Form::ThirteenOrphans {
        return Ok((name, false, 0));
    }
    for same in &hand_analyzer.same3 {
        if !same.has_honour()? {
            return Ok((name, false, 0));
        }
    }
    for head in &hand_analyzer.same2 {
        if !head.has_honour()? {
            return Ok((name, false, 0));
        }
    }
    if !hand_analyzer.sequential3.is_empty() {
        return Ok((name, false, 0));
    }
    Ok((name, true, 13))
}
/// Perfect Terminals (Chinrōtō / 清老頭)
pub fn check_perfect_terminals(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::PerfectTerminals,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Thirteen Orphans has no blocks, so the universal checks below would
    // otherwise pass without inspecting any tile.
    if hand_analyzer.form == Form::ThirteenOrphans {
        return Ok((name, false, 0));
    }
    if !hand_analyzer.sequential3.is_empty() {
        return Ok((name, false, 0));
    }
    for same in &hand_analyzer.same3 {
        if !same.has_1_or_9()? || same.has_honour()? {
            return Ok((name, false, 0));
        }
    }
    for head in &hand_analyzer.same2 {
        if !head.has_1_or_9()? || head.has_honour()? {
            return Ok((name, false, 0));
        }
    }
    Ok((name, true, 13))
}
/// All Green (Ryūiisō / 緑一色)
pub fn check_all_green(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::AllGreen,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Thirteen Orphans has no blocks, so the universal checks below would
    // otherwise pass without inspecting any tile.
    if hand_analyzer.form == Form::ThirteenOrphans {
        return Ok((name, false, 0));
    }
    // Only tiles drawn entirely in green qualify:
    // 2s, 3s, 4s, 6s, 8s, and the Green dragon (發).
    let is_green_tile = |t: u32| -> bool {
        matches!(
            t,
            Tile::S2 | Tile::S3 | Tile::S4 | Tile::S6 | Tile::S8 | Tile::Z6
        )
    };
    for same in &hand_analyzer.same3 {
        if !is_green_tile(same.get()[0]) {
            return Ok((name, false, 0));
        }
    }
    for seq in &hand_analyzer.sequential3 {
        let tiles = seq.get();
        for t in &tiles {
            if !is_green_tile(*t) {
                return Ok((name, false, 0));
            }
        }
    }
    for head in &hand_analyzer.same2 {
        if !is_green_tile(head.get()[0]) {
            return Ok((name, false, 0));
        }
    }
    Ok((name, true, 13))
}
/// Nine Gates (Chūren Pōto / 九蓮宝燈)
pub fn check_nine_gates(
    hand_analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::NineGates,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // Closed hands only.
    if status.has_claimed_open {
        return Ok((name, false, 0));
    }
    // Every block must be from a single suit with no honours.
    let mut has_character = false;
    let mut has_circle = false;
    let mut has_bamboo = false;
    let mut has_honour = false;

    for same in &hand_analyzer.same3 {
        if same.is_character()? {
            has_character = true;
        }
        if same.is_circle()? {
            has_circle = true;
        }
        if same.is_bamboo()? {
            has_bamboo = true;
        }
        if same.has_honour()? {
            has_honour = true;
        }
    }
    for seq in &hand_analyzer.sequential3 {
        if seq.is_character()? {
            has_character = true;
        }
        if seq.is_circle()? {
            has_circle = true;
        }
        if seq.is_bamboo()? {
            has_bamboo = true;
        }
    }
    for head in &hand_analyzer.same2 {
        if head.is_character()? {
            has_character = true;
        }
        if head.is_circle()? {
            has_circle = true;
        }
        if head.is_bamboo()? {
            has_bamboo = true;
        }
        if head.has_honour()? {
            has_honour = true;
        }
    }

    if has_honour {
        return Ok((name, false, 0));
    }
    let suit_count = [has_character, has_circle, has_bamboo]
        .iter()
        .filter(|&&x| x)
        .count();
    if suit_count != 1 {
        return Ok((name, false, 0));
    }

    // Tally tile counts and match them against the Nine Gates pattern.
    let offset = if has_character {
        0
    } else if has_circle {
        9
    } else {
        18
    };
    let mut counts = [0u32; 9];
    for same in &hand_analyzer.same3 {
        let t = same.get()[0];
        counts[(t - offset) as usize] += 3;
    }
    for seq in &hand_analyzer.sequential3 {
        let tiles = seq.get();
        for t in &tiles {
            counts[(*t - offset) as usize] += 1;
        }
    }
    for head in &hand_analyzer.same2 {
        let t = head.get()[0];
        counts[(t - offset) as usize] += 2;
    }
    for single in &hand_analyzer.single {
        if *single >= offset && *single < offset + 9 {
            counts[(*single - offset) as usize] += 1;
        }
    }

    // 1112345678999 + one more tile of the same suit: at least three 1s,
    // at least three 9s, at least one each of 2-8, and 14 tiles in total.
    if counts[0] >= 3
        && counts[8] >= 3
        && counts[1] >= 1
        && counts[2] >= 1
        && counts[3] >= 1
        && counts[4] >= 1
        && counts[5] >= 1
        && counts[6] >= 1
        && counts[7] >= 1
    {
        let total: u32 = counts.iter().sum();
        if total == 14 {
            if settings.double_yakuman && is_pure_nine_gates(hand) {
                return Ok((name, false, 0));
            }
            return Ok((name, true, 13));
        }
    }
    Ok((name, false, 0))
}

fn is_pure_nine_gates(hand: &Hand) -> bool {
    if !hand.melds().is_empty() || hand.tiles().len() != 13 {
        return false;
    }

    let Some(winning_tile) = hand.drawn() else {
        return false;
    };
    let offset = match winning_tile.get() {
        Tile::M1..=Tile::M9 => Tile::M1,
        Tile::P1..=Tile::P9 => Tile::P1,
        Tile::S1..=Tile::S9 => Tile::S1,
        _ => return false,
    };

    let mut counts = [0u32; 9];
    for tile in hand.tiles() {
        let tile_type = tile.get();
        if tile_type < offset || tile_type > offset + 8 {
            return false;
        }
        counts[(tile_type - offset) as usize] += 1;
    }
    counts == [3, 1, 1, 1, 1, 1, 1, 1, 3]
}

/// Pure Nine Gates (Junsei Chūren Pōto / 純正九蓮宝燈)
pub fn check_pure_nine_gates(
    hand_analyzer: &HandAnalyzer,
    hand: &Hand,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::PureNineGates,
        status.has_claimed_open,
        settings.display_lang,
    );
    if settings.double_yakuman
        && hand_analyzer.shanten.has_won()
        && !status.has_claimed_open
        && is_pure_nine_gates(hand)
    {
        Ok((name, true, 26))
    } else {
        Ok((name, false, 0))
    }
}
/// Four Quads (Sūkantsu / 四槓子)
pub fn check_four_quads(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::FourQuads,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    if status.kan_count == 4 {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// Blessing of Heaven (Tenhō / 天和)
pub fn check_blessing_of_heaven(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::BlessingOfHeaven,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // The dealer's starting hand is already complete.
    if status.is_dealer && status.is_first_turn && status.is_self_drawn && !status.has_claimed_open
    {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}
/// Blessing of Earth (Chihō / 地和)
pub fn check_blessing_of_earth(
    hand_analyzer: &HandAnalyzer,
    status: &Status,
    settings: &Settings,
) -> Result<(&'static str, bool, u32)> {
    let name = get(
        Kind::BlessingOfEarth,
        status.has_claimed_open,
        settings.display_lang,
    );
    if !hand_analyzer.shanten.has_won() {
        return Ok((name, false, 0));
    }
    // A non-dealer wins on their very first draw.
    if !status.is_dealer && status.is_first_turn && status.is_self_drawn && !status.has_claimed_open
    {
        Ok((name, true, 13))
    } else {
        Ok((name, false, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::Hand;
    use rstest::rstest;

    #[test]
    fn test_win_by_thirteen_orphans() {
        let test_str = "19m19p19s1123456z 7z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_thirteen_orphans(&test_analyzer, &test, &status, &settings).unwrap(),
            ("国士無双", true, 13)
        );
    }

    #[test]
    fn test_thirteen_orphans_thirteen_wait_double_yakuman_toggle() {
        let thirteen_wait = Hand::from("19m19p19s1234567z 1m");
        let analyzer = HandAnalyzer::new(&thirteen_wait).unwrap();
        let status = Status::new();
        let mut settings = Settings::new();

        assert_eq!(
            check_thirteen_orphans(&analyzer, &thirteen_wait, &status, &settings).unwrap(),
            ("国士無双", false, 0)
        );
        assert_eq!(
            check_thirteen_orphans_thirteen_wait(&analyzer, &thirteen_wait, &status, &settings)
                .unwrap(),
            ("国士無双十三面待ち", true, 26)
        );

        let single_wait = Hand::from("19m19p19s1123456z 7z");
        let analyzer = HandAnalyzer::new(&single_wait).unwrap();
        assert_eq!(
            check_thirteen_orphans(&analyzer, &single_wait, &status, &settings).unwrap(),
            ("国士無双", true, 13)
        );
        assert_eq!(
            check_thirteen_orphans_thirteen_wait(&analyzer, &single_wait, &status, &settings)
                .unwrap(),
            ("国士無双十三面待ち", false, 0)
        );

        settings.double_yakuman = false;
        let analyzer = HandAnalyzer::new(&thirteen_wait).unwrap();
        assert_eq!(
            check_thirteen_orphans(&analyzer, &thirteen_wait, &status, &settings).unwrap(),
            ("国士無双", true, 13)
        );
        assert_eq!(
            check_thirteen_orphans_thirteen_wait(&analyzer, &thirteen_wait, &status, &settings)
                .unwrap(),
            ("国士無双十三面待ち", false, 0)
        );
    }

    #[rstest]
    #[case::tanki_tsumo("111333m444s1777z 1z", true, ("四暗刻単騎待ち", true, 26), ("四暗刻", false, 0), false)]
    #[case::tanki_ron("111333m444s1777z 1z", false, ("四暗刻単騎待ち", true, 26), ("四暗刻", false, 0), false)]
    #[case::shanpon_tsumo("111333m444s55s77z 5s", true, ("四暗刻単騎待ち", false, 0), ("四暗刻", true, 13), false)]
    #[case::shanpon_ron("111333m444s55s77z 5s", false, ("四暗刻単騎待ち", false, 0), ("四暗刻", false, 0), false)]
    #[case::open_tanki_tsumo("111333m444s1777z 1z", true, ("四暗刻単騎待ち", false, 0), ("四暗刻", false, 0), true)]
    /// The pair-wait variant and the plain yakuman must never both apply.
    fn test_four_concealed_triplets(
        #[case] test_str: &str,
        #[case] is_self_drawn: bool,
        #[case] expected_single_wait: (&'static str, bool, u32),
        #[case] expected_four_concealed_triplets: (&'static str, bool, u32),
        #[case] has_claimed_open: bool,
    ) {
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_self_drawn = is_self_drawn;
        status.has_claimed_open = has_claimed_open;
        assert!(test_analyzer.shanten.has_won());
        assert_eq!(
            check_four_concealed_triplets_pair_wait(&test_analyzer, &test, &status, &settings)
                .unwrap(),
            expected_single_wait
        );
        assert_eq!(
            check_four_concealed_triplets(&test_analyzer, &test, &status, &settings).unwrap(),
            expected_four_concealed_triplets
        );
    }

    #[test]
    fn test_four_concealed_triplets_pair_wait_double_yakuman_toggle() {
        let hand = Hand::from("111333m444s1777z 1z");
        let analyzer = HandAnalyzer::new(&hand).unwrap();
        let status = Status::new();
        let mut settings = Settings::new();

        assert_eq!(
            check_four_concealed_triplets_pair_wait(&analyzer, &hand, &status, &settings).unwrap(),
            ("四暗刻単騎待ち", true, 26)
        );

        settings.double_yakuman = false;
        assert_eq!(
            check_four_concealed_triplets_pair_wait(&analyzer, &hand, &status, &settings).unwrap(),
            ("四暗刻単騎待ち", true, 13)
        );
    }
    #[test]
    fn test_win_by_big_dragons() {
        let test_str = "555666777z234m1p 1p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_big_dragons(&test_analyzer, &status, &settings).unwrap(),
            ("大三元", true, 13)
        );
    }
    #[test]
    fn test_win_by_little_winds() {
        let test_str = "11122233344z23m 4m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_little_winds(&test_analyzer, &status, &settings).unwrap(),
            ("小四喜", true, 13)
        );
    }
    #[test]
    fn test_win_by_big_winds() {
        let test_str = "5m 111z 222z 333z 444z 5m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_big_winds(&test_analyzer, &status, &settings).unwrap(),
            ("大四喜", true, 26)
        );

        let settings = Settings {
            double_yakuman: false,
            ..Settings::new()
        };
        assert_eq!(
            check_big_winds(&test_analyzer, &status, &settings).unwrap(),
            ("大四喜", true, 13)
        );
    }
    #[test]
    fn test_win_by_all_honours() {
        let test_str = "111222333z5z 777z 5z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_honours(&test_analyzer, &status, &settings).unwrap(),
            ("字一色", true, 13)
        );
    }
    #[test]
    fn test_win_by_perfect_terminals() {
        let test_str = "111999m1p 111s 999p 1p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_perfect_terminals(&test_analyzer, &status, &settings).unwrap(),
            ("清老頭", true, 13)
        );
    }
    #[test]
    fn test_win_by_all_green() {
        let test_str = "22233344s66z 888s 6z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let status = Status::new();
        let settings = Settings::new();
        assert_eq!(
            check_all_green(&test_analyzer, &status, &settings).unwrap(),
            ("緑一色", true, 13)
        );
    }
    #[test]
    fn test_win_by_nine_gates() {
        let test_str = "1112355678999m 4m";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.has_claimed_open = false;
        assert_eq!(
            check_nine_gates(&test_analyzer, &test, &status, &settings).unwrap(),
            ("九蓮宝燈", true, 13)
        );
    }

    #[test]
    fn test_pure_nine_gates_double_yakuman_toggle() {
        let pure = Hand::from("1112345678999m 5m");
        let analyzer = HandAnalyzer::new(&pure).unwrap();
        let status = Status::new();
        let mut settings = Settings::new();

        assert_eq!(
            check_nine_gates(&analyzer, &pure, &status, &settings).unwrap(),
            ("九蓮宝燈", false, 0)
        );
        assert_eq!(
            check_pure_nine_gates(&analyzer, &pure, &status, &settings).unwrap(),
            ("純正九蓮宝燈", true, 26)
        );

        let ordinary = Hand::from("1112355678999m 4m");
        let analyzer = HandAnalyzer::new(&ordinary).unwrap();
        assert_eq!(
            check_nine_gates(&analyzer, &ordinary, &status, &settings).unwrap(),
            ("九蓮宝燈", true, 13)
        );
        assert_eq!(
            check_pure_nine_gates(&analyzer, &ordinary, &status, &settings).unwrap(),
            ("純正九蓮宝燈", false, 0)
        );

        settings.double_yakuman = false;
        let analyzer = HandAnalyzer::new(&pure).unwrap();
        assert_eq!(
            check_nine_gates(&analyzer, &pure, &status, &settings).unwrap(),
            ("九蓮宝燈", true, 13)
        );
        assert_eq!(
            check_pure_nine_gates(&analyzer, &pure, &status, &settings).unwrap(),
            ("純正九蓮宝燈", false, 0)
        );
    }
    #[test]
    fn test_win_by_four_quads() {
        let test_str = "111333m444s1777z 1z";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.kan_count = 4;
        status.is_self_drawn = true;
        assert_eq!(
            check_four_quads(&test_analyzer, &status, &settings).unwrap(),
            ("四槓子", true, 13)
        );
    }
    #[test]
    fn test_win_by_blessing_of_heaven() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = true;
        status.is_first_turn = true;
        status.is_self_drawn = true;
        assert_eq!(
            check_blessing_of_heaven(&test_analyzer, &status, &settings).unwrap(),
            ("天和", true, 13)
        );
    }
    #[test]
    fn test_not_win_by_blessing_of_heaven_if_not_dealer() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = false;
        status.is_first_turn = true;
        status.is_self_drawn = true;
        assert_eq!(
            check_blessing_of_heaven(&test_analyzer, &status, &settings).unwrap(),
            ("天和", false, 0)
        );
    }
    #[test]
    fn test_win_by_blessing_of_earth() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = false;
        status.is_first_turn = true;
        status.is_self_drawn = true;
        assert_eq!(
            check_blessing_of_earth(&test_analyzer, &status, &settings).unwrap(),
            ("地和", true, 13)
        );
    }
    #[test]
    fn test_not_win_by_blessing_of_earth_if_dealer() {
        let test_str = "123m45678p999s11z 9p";
        let test = Hand::from(test_str);
        let test_analyzer = HandAnalyzer::new(&test).unwrap();
        let mut status = Status::new();
        let settings = Settings::new();
        status.is_dealer = true;
        status.is_first_turn = true;
        status.is_self_drawn = true;
        assert_eq!(
            check_blessing_of_earth(&test_analyzer, &status, &settings).unwrap(),
            ("地和", false, 0)
        );
    }
}
