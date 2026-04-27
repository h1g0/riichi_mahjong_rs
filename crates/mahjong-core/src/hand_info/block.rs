use anyhow::Result;
use anyhow::anyhow;
use std::cmp::Ordering;

use crate::tile::*;

/// ブロック（対子、順子、刻子）の振る舞いを定義する
pub trait BlockProperty {
    /// 么九牌が含まれているか
    fn has_1_or_9(&self) -> Result<bool>;
    /// 字牌が含まれているか
    fn has_honor(&self) -> Result<bool>;
    /// 特定の風牌が含まれているか
    fn has_wind(&self, wind: Wind) -> Result<bool>;
    /// 特定の三元牌が含まれているか
    fn has_dragon(&self, dragon: Dragon) -> Result<bool>;
    /// 萬子のブロックか
    fn is_character(&self) -> Result<bool>;
    /// 筒子のブロックか
    fn is_circle(&self) -> Result<bool>;
    /// 索子のブロックか
    fn is_bamboo(&self) -> Result<bool>;
}

fn is_proper_tile(tile: TileType) -> Result<()> {
    if matches!(tile, Tile::M1..=Tile::Z7) {
        Ok(())
    } else {
        Err(anyhow!("invalid tile: {}", tile))
    }
}

fn has_1_or_9(t: TileType) -> Result<bool> {
    is_proper_tile(t)?;
    match t {
        Tile::M1 | Tile::M9 => Ok(true),
        Tile::P1 | Tile::P9 => Ok(true),
        Tile::S1 | Tile::S9 => Ok(true),
        _ => Ok(false),
    }
}

fn has_honor(t: TileType) -> Result<bool> {
    is_proper_tile(t)?;
    match t {
        Tile::Z1..=Tile::Z7 => Ok(true),
        _ => Ok(false),
    }
}

fn has_wind(t: TileType, wind: Wind) -> Result<bool> {
    is_proper_tile(t)?;
    if let Some(w) = Wind::is_tile_type(t) {
        Ok(w == wind)
    } else {
        Ok(false)
    }
}

fn has_dragon(t: TileType, dragon: Dragon) -> Result<bool> {
    is_proper_tile(t)?;
    if let Some(d) = Dragon::is_tile_type(t) {
        Ok(d == dragon)
    } else {
        Ok(false)
    }
}

fn is_character(t: TileType) -> Result<bool> {
    is_proper_tile(t)?;
    match t {
        Tile::M1..=Tile::M9 => Ok(true),
        _ => Ok(false),
    }
}

fn is_circle(t: TileType) -> Result<bool> {
    is_proper_tile(t)?;
    match t {
        Tile::P1..=Tile::P9 => Ok(true),
        _ => Ok(false),
    }
}

fn is_bamboo(t: TileType) -> Result<bool> {
    is_proper_tile(t)?;
    match t {
        Tile::S1..=Tile::S9 => Ok(true),
        _ => Ok(false),
    }
}

fn is_same_suit(t1: TileType, t2: TileType) -> Result<bool> {
    is_proper_tile(t1)?;
    is_proper_tile(t2)?;
    match t1 {
        Tile::M1..=Tile::M9 => Ok(matches!(t2, Tile::M1..=Tile::M9)),
        Tile::P1..=Tile::P9 => Ok(matches!(t2, Tile::P1..=Tile::P9)),
        Tile::S1..=Tile::S9 => Ok(matches!(t2, Tile::S1..=Tile::S9)),
        _ => Ok(matches!(t2, Tile::Z1..=Tile::Z7)),
    }
}

/// 対子（同じ2枚）
#[derive(Debug, Eq, Clone, Copy)]
pub struct Same2 {
    tiles: [TileType; 2],
}
impl Same2 {
    pub fn new(tile1: TileType, tile2: TileType) -> Result<Same2> {
        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        if tile1 != tile2 {
            return Err(anyhow!("Not same tiles in `Same2`: {}, {} !", tile1, tile2));
        }
        Ok(Same2 {
            tiles: [tile1, tile2],
        })
    }
    pub fn get(&self) -> [TileType; 2] {
        self.tiles
    }
}
impl BlockProperty for Same2 {
    fn has_1_or_9(&self) -> Result<bool> {
        has_1_or_9(self.tiles[0])
    }
    fn has_honor(&self) -> Result<bool> {
        has_honor(self.tiles[0])
    }
    fn has_wind(&self, wind: Wind) -> Result<bool> {
        has_wind(self.tiles[0], wind)
    }
    fn has_dragon(&self, dragon: Dragon) -> Result<bool> {
        has_dragon(self.tiles[0], dragon)
    }
    fn is_character(&self) -> Result<bool> {
        is_character(self.tiles[0])
    }
    fn is_circle(&self) -> Result<bool> {
        is_circle(self.tiles[0])
    }
    fn is_bamboo(&self) -> Result<bool> {
        is_bamboo(self.tiles[0])
    }
}
impl PartialEq for Same2 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles[0] == other.tiles[0]
    }
}
impl PartialOrd for Same2 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Same2 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles[0].cmp(&other.tiles[0])
    }
}

/// 刻子（同じ3枚）
#[derive(Debug, Eq, Clone, Copy)]
pub struct Same3 {
    tiles: [TileType; 3],
}
impl Same3 {
    pub fn new(tile1: TileType, tile2: TileType, tile3: TileType) -> Result<Same3> {
        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        is_proper_tile(tile3)?;
        if tile1 != tile2 || tile1 != tile3 {
            return Err(anyhow!(
                "Not same tiles in `Same3`: {}, {}, {}!",
                tile1,
                tile2,
                tile3
            ));
        }
        Ok(Same3 {
            tiles: [tile1, tile2, tile3],
        })
    }
    pub fn get(&self) -> [TileType; 3] {
        self.tiles
    }
}
impl BlockProperty for Same3 {
    fn has_1_or_9(&self) -> Result<bool> {
        has_1_or_9(self.tiles[0])
    }
    fn has_honor(&self) -> Result<bool> {
        has_honor(self.tiles[0])
    }
    fn has_wind(&self, wind: Wind) -> Result<bool> {
        has_wind(self.tiles[0], wind)
    }
    fn has_dragon(&self, dragon: Dragon) -> Result<bool> {
        has_dragon(self.tiles[0], dragon)
    }
    fn is_character(&self) -> Result<bool> {
        is_character(self.tiles[0])
    }
    fn is_circle(&self) -> Result<bool> {
        is_circle(self.tiles[0])
    }
    fn is_bamboo(&self) -> Result<bool> {
        is_bamboo(self.tiles[0])
    }
}
impl PartialEq for Same3 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles[0] == other.tiles[0]
    }
}
impl PartialOrd for Same3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Same3 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles[0].cmp(&other.tiles[0])
    }
}

/// 塔子（連続した牌が2枚）もしくは嵌張（順子の真ん中が抜けている2枚）
#[derive(Debug, Eq, Clone, Copy)]
pub struct Sequential2 {
    tiles: [TileType; 2],
}
impl Sequential2 {
    pub fn new(tile1: TileType, tile2: TileType) -> Result<Sequential2> {
        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        if !(tile2 == tile1 + 1 || tile2 == tile1 + 2) {
            return Err(anyhow!(
                "Not sequential tiles in `Sequential2`: {}, {} !",
                tile1,
                tile2
            ));
        }
        if has_honor(tile1)? || has_honor(tile2)? {
            return Err(anyhow!(
                "Cannot assign Honor tiles to `Sequential2`: {}, {} !",
                tile1,
                tile2
            ));
        }
        if !is_same_suit(tile1, tile2)? {
            return Err(anyhow!(
                "Cannot assign different suits to `Sequential2`: {}, {} !",
                tile1,
                tile2
            ));
        }
        Ok(Sequential2 {
            tiles: [tile1, tile2],
        })
    }
    pub fn get(&self) -> [TileType; 2] {
        self.tiles
    }
}
impl BlockProperty for Sequential2 {
    fn has_1_or_9(&self) -> Result<bool> {
        Ok(has_1_or_9(self.tiles[0])? || has_1_or_9(self.tiles[1])?)
    }
    fn has_honor(&self) -> Result<bool> {
        Ok(false)
    }
    fn has_wind(&self, _: Wind) -> Result<bool> {
        Ok(false)
    }
    fn has_dragon(&self, _: Dragon) -> Result<bool> {
        Ok(false)
    }
    fn is_character(&self) -> Result<bool> {
        is_character(self.tiles[0])
    }
    fn is_circle(&self) -> Result<bool> {
        is_circle(self.tiles[0])
    }
    fn is_bamboo(&self) -> Result<bool> {
        is_bamboo(self.tiles[0])
    }
}
impl PartialEq for Sequential2 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles[0] == other.tiles[0] && self.tiles[1] == other.tiles[1]
    }
}
impl PartialOrd for Sequential2 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Sequential2 {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.tiles[0].cmp(&other.tiles[0]) {
            Ordering::Equal => self.tiles[1].cmp(&other.tiles[1]),
            ord => ord,
        }
    }
}

/// 順子（連続した3枚）
#[derive(Debug, Eq, Clone, Copy)]
pub struct Sequential3 {
    tiles: [TileType; 3],
}
impl Sequential3 {
    pub fn new(tile1: TileType, tile2: TileType, tile3: TileType) -> Result<Sequential3> {
        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        is_proper_tile(tile3)?;
        if tile2 != tile1 + 1 || tile3 != tile2 + 1 {
            return Err(anyhow!(
                "Not sequential tiles in `Sequential3`:{}, {}, {} !",
                tile1,
                tile2,
                tile3
            ));
        }
        if has_honor(tile1)? || has_honor(tile2)? || has_honor(tile3)? {
            return Err(anyhow!(
                "Cannot assign Honor tiles to `Sequential3`: {}, {}, {} !",
                tile1,
                tile2,
                tile3
            ));
        }
        if !is_same_suit(tile1, tile2)? || !is_same_suit(tile2, tile3)? {
            return Err(anyhow!(
                "Cannot assign different suits to `Sequential3`: {}, {}, {} !",
                tile1,
                tile2,
                tile3
            ));
        }
        Ok(Sequential3 {
            tiles: [tile1, tile2, tile3],
        })
    }
    pub fn get(&self) -> [TileType; 3] {
        self.tiles
    }

    /// 指定した牌がこの順子の両面待ち牌かを返す
    ///
    /// 辺張（123の3待ち・789の7待ち）と嵌張は両面待ちではない
    pub fn is_two_sided_wait(&self, winning_tile: TileType) -> bool {
        if winning_tile == self.tiles[0] && suit_rank(self.tiles[0]) != Some(7) {
            return true;
        }
        if winning_tile == self.tiles[2] && suit_rank(self.tiles[2]) != Some(3) {
            return true;
        }
        false
    }
}
impl BlockProperty for Sequential3 {
    fn has_1_or_9(&self) -> Result<bool> {
        Ok(has_1_or_9(self.tiles[0])? || has_1_or_9(self.tiles[2])?)
    }
    fn has_honor(&self) -> Result<bool> {
        Ok(false)
    }
    fn has_wind(&self, _: Wind) -> Result<bool> {
        Ok(false)
    }
    fn has_dragon(&self, _: Dragon) -> Result<bool> {
        Ok(false)
    }
    fn is_character(&self) -> Result<bool> {
        is_character(self.tiles[0])
    }
    fn is_circle(&self) -> Result<bool> {
        is_circle(self.tiles[0])
    }
    fn is_bamboo(&self) -> Result<bool> {
        is_bamboo(self.tiles[0])
    }
}
impl PartialEq for Sequential3 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles[0] == other.tiles[0]
    }
}
impl PartialOrd for Sequential3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Sequential3 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles[0].cmp(&other.tiles[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_same2_normal() {
        assert_eq!(Same2::new(Tile::M1, Tile::M1).unwrap().get(), [Tile::M1; 2]);
    }
    #[test]
    fn test_same2_errors_when_not_same() {
        assert!(Same2::new(Tile::M1, Tile::M2).is_err());
    }
    #[test]
    fn test_same2_errors_when_invalid_tile() {
        assert!(Same2::new(34, 34).is_err());
    }
    #[test]
    fn test_same2_errors_when_invalid_tile2() {
        assert!(Same2::new(Tile::M1, 34).is_err());
    }
    #[test]
    fn test_same3_normal() {
        assert_eq!(
            Same3::new(Tile::M1, Tile::M1, Tile::M1).unwrap().get(),
            [Tile::M1; 3]
        );
    }
    #[test]
    fn test_same3_errors_when_not_same() {
        assert!(Same3::new(Tile::M1, Tile::M1, Tile::M2).is_err());
    }
    #[test]
    fn test_same3_errors_when_invalid_tile() {
        assert!(Same3::new(34, 34, 34).is_err());
    }
    #[test]
    fn test_same3_errors_when_invalid_tile2() {
        assert!(Same3::new(Tile::M1, 34, 34).is_err());
    }
    #[test]
    fn test_same3_errors_when_invalid_tile3() {
        assert!(Same3::new(Tile::M1, Tile::M1, 34).is_err());
    }

    #[test]
    fn test_sequential2_normal() {
        assert_eq!(
            Sequential2::new(Tile::M1, Tile::M2).unwrap().get(),
            [Tile::M1, Tile::M2]
        );
    }
    #[test]
    fn test_sequential2_normal2() {
        assert_eq!(
            Sequential2::new(Tile::M1, Tile::M3).unwrap().get(),
            [Tile::M1, Tile::M3]
        );
    }
    #[test]
    fn test_sequential2_errors_when_not_sequential() {
        assert!(Sequential2::new(Tile::M1, Tile::M4).is_err());
    }
    #[test]
    fn test_sequential2_errors_when_honor() {
        assert!(Sequential2::new(Tile::Z1, Tile::Z2).is_err());
    }
    #[test]
    fn test_sequential2_errors_when_other_kind() {
        assert!(Sequential2::new(Tile::M9, Tile::P1).is_err());
    }
    #[test]
    fn test_sequential2_errors_when_other_kind2() {
        assert!(Sequential2::new(Tile::P8, Tile::S1).is_err());
    }
    #[test]
    fn test_sequential2_errors_when_invalid_tile() {
        assert!(Sequential2::new(34, 35).is_err());
    }
    #[test]
    fn test_sequential2_errors_when_invalid_tile2() {
        assert!(Sequential2::new(Tile::M1, 34).is_err());
    }
    #[test]
    fn test_sequential3_normal() {
        assert_eq!(
            Sequential3::new(Tile::M1, Tile::M2, Tile::M3)
                .unwrap()
                .get(),
            [Tile::M1, Tile::M2, Tile::M3]
        );
    }
    #[test]
    fn test_sequential3_errors_when_not_sequential() {
        assert!(Sequential3::new(Tile::M1, Tile::M2, Tile::M4).is_err());
    }
    #[test]
    fn test_sequential3_errors_when_honor() {
        assert!(Sequential3::new(Tile::Z1, Tile::Z2, Tile::Z3).is_err());
    }
    #[test]
    fn test_sequential3_errors_when_other_kind() {
        assert!(Sequential3::new(Tile::M8, Tile::M9, Tile::P1).is_err());
    }
    #[test]
    fn test_sequential3_errors_when_other_kind2() {
        assert!(Sequential3::new(Tile::P9, Tile::S1, Tile::S2).is_err());
    }
    #[test]
    fn test_sequential3_errors_when_invalid_tile() {
        assert!(Sequential3::new(34, 35, 36).is_err());
    }
    #[test]
    fn test_sequential3_errors_when_invalid_tile2() {
        assert!(Sequential3::new(Tile::M1, 34, 35).is_err());
    }
    #[test]
    fn test_sequential3_errors_when_invalid_tile3() {
        assert!(Sequential3::new(Tile::M1, Tile::M2, 34).is_err());
    }

    fn seq3(t1: u32, t2: u32, t3: u32) -> Sequential3 {
        Sequential3::new(t1, t2, t3).unwrap()
    }

    // is_two_sided_wait: 両面待ち
    #[test]
    fn test_two_sided_wait_low_end() {
        // 456の4待ち — 低位端、両面成立
        assert!(seq3(Tile::M4, Tile::M5, Tile::M6).is_two_sided_wait(Tile::M4));
    }
    #[test]
    fn test_two_sided_wait_high_end() {
        // 456の6待ち — 高位端、両面成立
        assert!(seq3(Tile::M4, Tile::M5, Tile::M6).is_two_sided_wait(Tile::M6));
    }

    // is_two_sided_wait: 辺張（両面でない）
    #[test]
    fn test_penchan_low_end_not_two_sided() {
        // 123の3待ち — 高位端が % 9 == 2 なので辺張
        assert!(!seq3(Tile::M1, Tile::M2, Tile::M3).is_two_sided_wait(Tile::M3));
    }
    #[test]
    fn test_penchan_high_end_not_two_sided() {
        // 789の7待ち — 低位端が % 9 == 6 なので辺張
        assert!(!seq3(Tile::M7, Tile::M8, Tile::M9).is_two_sided_wait(Tile::M7));
    }

    // is_two_sided_wait: 嵌張（両面でない）
    #[test]
    fn test_kanchan_not_two_sided() {
        // 468の5待ち — 中牌は低位端でも高位端でもない
        assert!(!seq3(Tile::M4, Tile::M5, Tile::M6).is_two_sided_wait(Tile::M5));
    }

    // is_two_sided_wait: 上がり牌がこの順子に含まれない
    #[test]
    fn test_unrelated_tile_not_two_sided() {
        assert!(!seq3(Tile::M4, Tile::M5, Tile::M6).is_two_sided_wait(Tile::M1));
    }

    // is_two_sided_wait: 筒子・索子でも正しく動作する
    #[test]
    fn test_two_sided_wait_pinzu() {
        assert!(seq3(Tile::P5, Tile::P6, Tile::P7).is_two_sided_wait(Tile::P5));
        assert!(seq3(Tile::P5, Tile::P6, Tile::P7).is_two_sided_wait(Tile::P7));
    }
    #[test]
    fn test_two_sided_wait_souzu() {
        assert!(seq3(Tile::S2, Tile::S3, Tile::S4).is_two_sided_wait(Tile::S2));
        assert!(seq3(Tile::S2, Tile::S3, Tile::S4).is_two_sided_wait(Tile::S4));
    }

    // --- Same2 BlockProperty ---

    #[test]
    fn test_same2_has_1_or_9() {
        assert!(
            Same2::new(Tile::M1, Tile::M1)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Same2::new(Tile::M9, Tile::M9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Same2::new(Tile::P1, Tile::P1)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Same2::new(Tile::P9, Tile::P9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Same2::new(Tile::S1, Tile::S1)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Same2::new(Tile::S9, Tile::S9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::M5, Tile::M5)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::Z1, Tile::Z1)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
    }

    #[test]
    fn test_same2_has_honor() {
        assert!(Same2::new(Tile::Z1, Tile::Z1).unwrap().has_honor().unwrap());
        assert!(Same2::new(Tile::Z7, Tile::Z7).unwrap().has_honor().unwrap());
        assert!(!Same2::new(Tile::M1, Tile::M1).unwrap().has_honor().unwrap());
        assert!(!Same2::new(Tile::P5, Tile::P5).unwrap().has_honor().unwrap());
    }

    #[test]
    fn test_same2_has_wind() {
        assert!(
            Same2::new(Tile::Z1, Tile::Z1)
                .unwrap()
                .has_wind(Wind::East)
                .unwrap()
        );
        assert!(
            Same2::new(Tile::Z2, Tile::Z2)
                .unwrap()
                .has_wind(Wind::South)
                .unwrap()
        );
        assert!(
            Same2::new(Tile::Z3, Tile::Z3)
                .unwrap()
                .has_wind(Wind::West)
                .unwrap()
        );
        assert!(
            Same2::new(Tile::Z4, Tile::Z4)
                .unwrap()
                .has_wind(Wind::North)
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::Z1, Tile::Z1)
                .unwrap()
                .has_wind(Wind::South)
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::M1, Tile::M1)
                .unwrap()
                .has_wind(Wind::East)
                .unwrap()
        );
    }

    #[test]
    fn test_same2_has_dragon() {
        assert!(
            Same2::new(Tile::Z5, Tile::Z5)
                .unwrap()
                .has_dragon(Dragon::White)
                .unwrap()
        );
        assert!(
            Same2::new(Tile::Z6, Tile::Z6)
                .unwrap()
                .has_dragon(Dragon::Green)
                .unwrap()
        );
        assert!(
            Same2::new(Tile::Z7, Tile::Z7)
                .unwrap()
                .has_dragon(Dragon::Red)
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::Z5, Tile::Z5)
                .unwrap()
                .has_dragon(Dragon::Green)
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::M1, Tile::M1)
                .unwrap()
                .has_dragon(Dragon::White)
                .unwrap()
        );
    }

    #[test]
    fn test_same2_is_character() {
        assert!(
            Same2::new(Tile::M5, Tile::M5)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::P1, Tile::P1)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::S1, Tile::S1)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Same2::new(Tile::Z1, Tile::Z1)
                .unwrap()
                .is_character()
                .unwrap()
        );
    }

    #[test]
    fn test_same2_is_circle() {
        assert!(Same2::new(Tile::P5, Tile::P5).unwrap().is_circle().unwrap());
        assert!(!Same2::new(Tile::M1, Tile::M1).unwrap().is_circle().unwrap());
        assert!(!Same2::new(Tile::S1, Tile::S1).unwrap().is_circle().unwrap());
        assert!(!Same2::new(Tile::Z1, Tile::Z1).unwrap().is_circle().unwrap());
    }

    #[test]
    fn test_same2_is_bamboo() {
        assert!(Same2::new(Tile::S5, Tile::S5).unwrap().is_bamboo().unwrap());
        assert!(!Same2::new(Tile::M1, Tile::M1).unwrap().is_bamboo().unwrap());
        assert!(!Same2::new(Tile::P1, Tile::P1).unwrap().is_bamboo().unwrap());
        assert!(!Same2::new(Tile::Z1, Tile::Z1).unwrap().is_bamboo().unwrap());
    }

    // --- Same2 Ord/PartialEq ---

    #[test]
    fn test_same2_ord() {
        let a = Same2::new(Tile::M1, Tile::M1).unwrap();
        let b = Same2::new(Tile::M2, Tile::M2).unwrap();
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a);
        assert_ne!(a, b);
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(a.partial_cmp(&a), Some(std::cmp::Ordering::Equal));
    }

    // --- Same3 BlockProperty ---

    #[test]
    fn test_same3_has_1_or_9() {
        assert!(
            Same3::new(Tile::M1, Tile::M1, Tile::M1)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Same3::new(Tile::S9, Tile::S9, Tile::S9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::M5, Tile::M5, Tile::M5)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::Z1, Tile::Z1, Tile::Z1)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
    }

    #[test]
    fn test_same3_has_honor() {
        assert!(
            Same3::new(Tile::Z1, Tile::Z1, Tile::Z1)
                .unwrap()
                .has_honor()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::M5, Tile::M5, Tile::M5)
                .unwrap()
                .has_honor()
                .unwrap()
        );
    }

    #[test]
    fn test_same3_has_wind() {
        assert!(
            Same3::new(Tile::Z1, Tile::Z1, Tile::Z1)
                .unwrap()
                .has_wind(Wind::East)
                .unwrap()
        );
        assert!(
            Same3::new(Tile::Z4, Tile::Z4, Tile::Z4)
                .unwrap()
                .has_wind(Wind::North)
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::Z1, Tile::Z1, Tile::Z1)
                .unwrap()
                .has_wind(Wind::West)
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::M5, Tile::M5, Tile::M5)
                .unwrap()
                .has_wind(Wind::East)
                .unwrap()
        );
    }

    #[test]
    fn test_same3_has_dragon() {
        assert!(
            Same3::new(Tile::Z5, Tile::Z5, Tile::Z5)
                .unwrap()
                .has_dragon(Dragon::White)
                .unwrap()
        );
        assert!(
            Same3::new(Tile::Z7, Tile::Z7, Tile::Z7)
                .unwrap()
                .has_dragon(Dragon::Red)
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::Z5, Tile::Z5, Tile::Z5)
                .unwrap()
                .has_dragon(Dragon::Red)
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::M1, Tile::M1, Tile::M1)
                .unwrap()
                .has_dragon(Dragon::White)
                .unwrap()
        );
    }

    #[test]
    fn test_same3_is_character() {
        assert!(
            Same3::new(Tile::M3, Tile::M3, Tile::M3)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::P3, Tile::P3, Tile::P3)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::Z1, Tile::Z1, Tile::Z1)
                .unwrap()
                .is_character()
                .unwrap()
        );
    }

    #[test]
    fn test_same3_is_circle() {
        assert!(
            Same3::new(Tile::P3, Tile::P3, Tile::P3)
                .unwrap()
                .is_circle()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::M3, Tile::M3, Tile::M3)
                .unwrap()
                .is_circle()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::S3, Tile::S3, Tile::S3)
                .unwrap()
                .is_circle()
                .unwrap()
        );
    }

    #[test]
    fn test_same3_is_bamboo() {
        assert!(
            Same3::new(Tile::S3, Tile::S3, Tile::S3)
                .unwrap()
                .is_bamboo()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::M3, Tile::M3, Tile::M3)
                .unwrap()
                .is_bamboo()
                .unwrap()
        );
        assert!(
            !Same3::new(Tile::P3, Tile::P3, Tile::P3)
                .unwrap()
                .is_bamboo()
                .unwrap()
        );
    }

    // --- Same3 Ord/PartialEq ---

    #[test]
    fn test_same3_ord() {
        let a = Same3::new(Tile::M1, Tile::M1, Tile::M1).unwrap();
        let b = Same3::new(Tile::P1, Tile::P1, Tile::P1).unwrap();
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a);
        assert_ne!(a, b);
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(b.partial_cmp(&a), Some(std::cmp::Ordering::Greater));
    }

    // --- Sequential2 BlockProperty ---

    #[test]
    fn test_sequential2_has_1_or_9_first_tile() {
        // tiles[0] が 1
        assert!(
            Sequential2::new(Tile::M1, Tile::M2)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Sequential2::new(Tile::P1, Tile::P2)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Sequential2::new(Tile::S1, Tile::S2)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_has_1_or_9_second_tile() {
        // tiles[0] が 1でも9でもなく、tiles[1] が 9
        assert!(
            Sequential2::new(Tile::M8, Tile::M9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Sequential2::new(Tile::P8, Tile::P9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            Sequential2::new(Tile::S8, Tile::S9)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_has_1_or_9_false() {
        assert!(
            !Sequential2::new(Tile::M3, Tile::M4)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::P5, Tile::P6)
                .unwrap()
                .has_1_or_9()
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_has_honor() {
        // 字牌は塔子にならないので常に false
        assert!(
            !Sequential2::new(Tile::M2, Tile::M3)
                .unwrap()
                .has_honor()
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_has_wind() {
        assert!(
            !Sequential2::new(Tile::M2, Tile::M3)
                .unwrap()
                .has_wind(Wind::East)
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::S4, Tile::S5)
                .unwrap()
                .has_wind(Wind::North)
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_has_dragon() {
        assert!(
            !Sequential2::new(Tile::M2, Tile::M3)
                .unwrap()
                .has_dragon(Dragon::White)
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::P6, Tile::P7)
                .unwrap()
                .has_dragon(Dragon::Red)
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_is_character() {
        assert!(
            Sequential2::new(Tile::M3, Tile::M4)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::P3, Tile::P4)
                .unwrap()
                .is_character()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::S3, Tile::S4)
                .unwrap()
                .is_character()
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_is_circle() {
        assert!(
            Sequential2::new(Tile::P3, Tile::P4)
                .unwrap()
                .is_circle()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::M3, Tile::M4)
                .unwrap()
                .is_circle()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::S3, Tile::S4)
                .unwrap()
                .is_circle()
                .unwrap()
        );
    }

    #[test]
    fn test_sequential2_is_bamboo() {
        assert!(
            Sequential2::new(Tile::S3, Tile::S4)
                .unwrap()
                .is_bamboo()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::M3, Tile::M4)
                .unwrap()
                .is_bamboo()
                .unwrap()
        );
        assert!(
            !Sequential2::new(Tile::P3, Tile::P4)
                .unwrap()
                .is_bamboo()
                .unwrap()
        );
    }

    // --- Sequential2 Ord/PartialEq ---

    #[test]
    fn test_sequential2_ord() {
        let a = Sequential2::new(Tile::M1, Tile::M2).unwrap();
        let b = Sequential2::new(Tile::M3, Tile::M4).unwrap();
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a);
        assert_ne!(a, b);
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(a.partial_cmp(&a), Some(std::cmp::Ordering::Equal));
    }

    #[test]
    fn test_sequential2_ord_same_first_tile() {
        let a = Sequential2::new(Tile::M1, Tile::M2).unwrap();
        let b = Sequential2::new(Tile::M1, Tile::M3).unwrap();
        assert!(a < b);
        assert!(b > a);
        assert_ne!(a, b);
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(b.partial_cmp(&a), Some(std::cmp::Ordering::Greater));
    }

    // --- Sequential3 BlockProperty ---

    #[test]
    fn test_sequential3_has_1_or_9_first_tile() {
        // tiles[0] が 1
        assert!(seq3(Tile::M1, Tile::M2, Tile::M3).has_1_or_9().unwrap());
        assert!(seq3(Tile::P1, Tile::P2, Tile::P3).has_1_or_9().unwrap());
        assert!(seq3(Tile::S1, Tile::S2, Tile::S3).has_1_or_9().unwrap());
    }

    #[test]
    fn test_sequential3_has_1_or_9_last_tile() {
        // tiles[0] が 1でも9でもなく、tiles[2] が 9
        assert!(seq3(Tile::M7, Tile::M8, Tile::M9).has_1_or_9().unwrap());
        assert!(seq3(Tile::P7, Tile::P8, Tile::P9).has_1_or_9().unwrap());
        assert!(seq3(Tile::S7, Tile::S8, Tile::S9).has_1_or_9().unwrap());
    }

    #[test]
    fn test_sequential3_has_1_or_9_false() {
        assert!(!seq3(Tile::M3, Tile::M4, Tile::M5).has_1_or_9().unwrap());
        assert!(!seq3(Tile::P4, Tile::P5, Tile::P6).has_1_or_9().unwrap());
    }

    #[test]
    fn test_sequential3_has_honor() {
        // 字牌は順子にならないので常に false
        assert!(!seq3(Tile::M2, Tile::M3, Tile::M4).has_honor().unwrap());
    }

    #[test]
    fn test_sequential3_has_wind() {
        assert!(
            !seq3(Tile::M2, Tile::M3, Tile::M4)
                .has_wind(Wind::East)
                .unwrap()
        );
        assert!(
            !seq3(Tile::S5, Tile::S6, Tile::S7)
                .has_wind(Wind::North)
                .unwrap()
        );
    }

    #[test]
    fn test_sequential3_has_dragon() {
        assert!(
            !seq3(Tile::M2, Tile::M3, Tile::M4)
                .has_dragon(Dragon::White)
                .unwrap()
        );
        assert!(
            !seq3(Tile::P5, Tile::P6, Tile::P7)
                .has_dragon(Dragon::Red)
                .unwrap()
        );
    }

    #[test]
    fn test_sequential3_is_character() {
        assert!(seq3(Tile::M3, Tile::M4, Tile::M5).is_character().unwrap());
        assert!(!seq3(Tile::P3, Tile::P4, Tile::P5).is_character().unwrap());
        assert!(!seq3(Tile::S3, Tile::S4, Tile::S5).is_character().unwrap());
    }

    #[test]
    fn test_sequential3_is_circle() {
        assert!(seq3(Tile::P3, Tile::P4, Tile::P5).is_circle().unwrap());
        assert!(!seq3(Tile::M3, Tile::M4, Tile::M5).is_circle().unwrap());
        assert!(!seq3(Tile::S3, Tile::S4, Tile::S5).is_circle().unwrap());
    }

    #[test]
    fn test_sequential3_is_bamboo() {
        assert!(seq3(Tile::S3, Tile::S4, Tile::S5).is_bamboo().unwrap());
        assert!(!seq3(Tile::M3, Tile::M4, Tile::M5).is_bamboo().unwrap());
        assert!(!seq3(Tile::P3, Tile::P4, Tile::P5).is_bamboo().unwrap());
    }

    // --- Sequential3 Ord/PartialEq ---

    #[test]
    fn test_sequential3_ord() {
        let a = seq3(Tile::M1, Tile::M2, Tile::M3);
        let b = seq3(Tile::M4, Tile::M5, Tile::M6);
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a);
        assert_ne!(a, b);
        assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
        assert_eq!(b.partial_cmp(&a), Some(std::cmp::Ordering::Greater));
        assert_eq!(a.partial_cmp(&a), Some(std::cmp::Ordering::Equal));
    }

    // --- Private function direct tests (error paths and is_same_suit Z-tile arm) ---

    #[test]
    fn test_has_1_or_9_invalid_tile() {
        assert!(has_1_or_9(34).is_err());
    }

    #[test]
    fn test_has_honor_invalid_tile() {
        assert!(has_honor(34).is_err());
    }

    #[test]
    fn test_has_wind_invalid_tile() {
        assert!(has_wind(34, Wind::East).is_err());
    }

    #[test]
    fn test_has_dragon_invalid_tile() {
        assert!(has_dragon(34, Dragon::White).is_err());
    }

    #[test]
    fn test_is_character_invalid_tile() {
        assert!(is_character(34).is_err());
    }

    #[test]
    fn test_is_circle_invalid_tile() {
        assert!(is_circle(34).is_err());
    }

    #[test]
    fn test_is_bamboo_invalid_tile() {
        assert!(is_bamboo(34).is_err());
    }

    #[test]
    fn test_is_same_suit_honor_same() {
        assert!(is_same_suit(Tile::Z1, Tile::Z7).unwrap());
    }

    #[test]
    fn test_is_same_suit_honor_different() {
        assert!(!is_same_suit(Tile::Z1, Tile::M1).unwrap());
    }

    #[test]
    fn test_is_same_suit_invalid_first_tile() {
        assert!(is_same_suit(34, 35).is_err());
    }

    #[test]
    fn test_is_same_suit_invalid_second_tile() {
        assert!(is_same_suit(Tile::M1, 34).is_err());
    }
}
