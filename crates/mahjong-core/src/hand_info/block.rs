use anyhow::anyhow;
use anyhow::Result;
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
        Tile::Z1..=Tile::Z7 => Ok(matches!(t2, Tile::Z1..=Tile::Z7)),
        _ => Err(anyhow!("invalid tile: {}", t1)),
    }
}

#[derive(Debug, Eq, Clone, Copy)]
/// 対子（同じ2枚）
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
impl Ord for Same2 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles.cmp(&other.tiles)
    }
}

impl PartialOrd for Same2 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Same2 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles == other.tiles
    }
}
impl BlockProperty for Same2 {
    /// 么九牌が含まれているか
    fn has_1_or_9(&self) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        has_1_or_9(self.tiles[0])
    }
    /// 字牌が含まれているか
    fn has_honor(&self) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        has_honor(self.tiles[0])
    }
    /// 特定の風牌が含まれているか
    fn has_wind(&self, wind: Wind) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        has_wind(self.tiles[0], wind)
    }
    /// 特定の三元牌が含まれているか
    fn has_dragon(&self, dragon: Dragon) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        has_dragon(self.tiles[0], dragon)
    }
    /// 萬子のブロックか
    fn is_character(&self) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        is_character(self.tiles[0])
    }
    /// 筒子のブロックか
    fn is_circle(&self) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        is_circle(self.tiles[0])
    }
    /// 索子のブロックか
    fn is_bamboo(&self) -> Result<bool> {
        // 2枚は同じ牌なので１枚目のみ調べれば良い
        is_bamboo(self.tiles[0])
    }
}

#[derive(Debug, Eq, Clone, Copy)]
/// 刻子（同じ3枚）
pub struct Same3 {
    tiles: [TileType; 3],
}
impl Same3 {
    pub fn new(tile1: TileType, tile2: TileType, tile3: TileType) -> Result<Same3> {
        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        is_proper_tile(tile3)?;
        if tile1 != tile2 || tile1 != tile3 {
            return Err(anyhow!("Not same tiles in `Same3`: {}, {}, {}!", tile1, tile2, tile3));
        }



        Ok(Same3 {
            tiles: [tile1, tile2, tile3],
        })
    }
    /// 牌の配列を返す
    pub fn get(&self) -> [TileType; 3] {
        self.tiles
    }
}
impl Ord for Same3 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles.cmp(&other.tiles)
    }
}

impl PartialOrd for Same3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Same3 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles == other.tiles
    }
}
impl BlockProperty for Same3 {
    /// 么九牌が含まれているか
    fn has_1_or_9(&self) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        has_1_or_9(self.tiles[0])
    }
    /// 字牌が含まれているか
    fn has_honor(&self) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        has_honor(self.tiles[0])
    }
    /// 特定の風牌が含まれているか
    fn has_wind(&self, wind: Wind) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        has_wind(self.tiles[0], wind)
    }
    /// 特定の三元牌が含まれているか
    fn has_dragon(&self, dragon: Dragon) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        has_dragon(self.tiles[0], dragon)
    }
    /// 萬子のブロックか
    fn is_character(&self) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        is_character(self.tiles[0])
    }
    /// 筒子のブロックか
    fn is_circle(&self) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        is_circle(self.tiles[0])
    }
    /// 索子のブロックか
    fn is_bamboo(&self) -> Result<bool> {
        // 3枚は同じ牌なので１枚目のみ調べれば良い
        is_bamboo(self.tiles[0])
    }
}

#[derive(Debug, Eq, Clone, Copy)]
/// 塔子（連続した牌が2枚）もしくは嵌張（順子の真ん中が抜けている2枚）
pub struct Sequential2 {
    tiles: [TileType; 2],
}
impl Sequential2 {
    pub fn new(tile1: TileType, tile2: TileType) -> Result<Sequential2> {
        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        // まず連続でなければパニック
        if !(tile2 == tile1 + 1 || tile2 == tile1 + 2) {
            return Err(anyhow!("Not sequential tiles in `Sequential2`: {}, {} !",tile1, tile2));
        }

        // 字牌は順子にならない
        if has_honor(tile1)? || has_honor(tile2)? {
            return Err(anyhow!("Cannot assign Honor tiles to `Sequential2`: {}, {} !",tile1, tile2));
        }
        if !is_same_suit(tile1, tile2)? {
            return Err(anyhow!("Cannot assign different suits to `Sequential2`: {}, {} !",tile1, tile2));
        }
        Ok(Sequential2 {
            tiles: [tile1, tile2],
        })
    }
    /// 牌の配列を返す
    pub fn get(&self) -> [TileType; 2] {
        self.tiles
    }
}
impl Ord for Sequential2 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles.cmp(&other.tiles)
    }
}

impl PartialOrd for Sequential2 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Sequential2 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles == other.tiles
    }
}
impl BlockProperty for Sequential2 {
    /// 么九牌が含まれているか
    fn has_1_or_9(&self) -> Result<bool> {
        Ok(has_1_or_9(self.tiles[0])? || has_1_or_9(self.tiles[1])?)
    }
    /// 字牌が含まれているか
    fn has_honor(&self) -> Result<bool> {
        //字牌は塔子にならない
        Ok(false)
    }
    /// 特定の風牌が含まれているか
    fn has_wind(&self, _: Wind) -> Result<bool> {
        // 字牌は塔子にならない
        Ok(false)
    }
    /// 特定の三元牌が含まれているか
    fn has_dragon(&self, _: Dragon) -> Result<bool> {
        // 字牌は塔子にならない
        Ok(false)
    }
    /// 萬子のブロックか
    fn is_character(&self) -> Result<bool> {
        is_character(self.tiles[0])
    }
    /// 筒子のブロックか
    fn is_circle(&self) -> Result<bool> {
        is_circle(self.tiles[0])
    }
    /// 索子のブロックか
    fn is_bamboo(&self) -> Result<bool> {
        is_bamboo(self.tiles[0])
    }
}

#[derive(Debug, Eq, Clone, Copy)]
/// 順子（連続した3枚）
pub struct Sequential3 {
    tiles: [TileType; 3],
}
impl Sequential3 {
    pub fn new(tile1: TileType, tile2: TileType, tile3: TileType) -> Result<Sequential3> {

        is_proper_tile(tile1)?;
        is_proper_tile(tile2)?;
        is_proper_tile(tile3)?;

        // 連続でなければエラー
        if tile2 != tile1 + 1 || tile3 != tile2 + 1 {
            return Err(anyhow!("Not sequential tiles in `Sequential3`:{}, {}, {} !",tile1, tile2, tile3));
        }

        // 字牌は順子にならない
        if has_honor(tile1)? || has_honor(tile2)? || has_honor(tile3)? {
            return Err(anyhow!("Cannot assign Honor tiles to `Sequential3`: {}, {}, {} !",tile1, tile2, tile3));
        }

        if !is_same_suit(tile1, tile2)? || !is_same_suit(tile2, tile3)? {
            return Err(anyhow!("Cannot assign different suits to `Sequential3`: {}, {}, {} !",tile1, tile2, tile3));
        }
        Ok(Sequential3 {
            tiles: [tile1, tile2, tile3],
        })
    }
    /// 牌の配列を返す
    pub fn get(&self) -> [TileType; 3] {
        self.tiles
    }
}
impl Ord for Sequential3 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.tiles.cmp(&other.tiles)
    }
}

impl PartialOrd for Sequential3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Sequential3 {
    fn eq(&self, other: &Self) -> bool {
        self.tiles == other.tiles
    }
}
impl BlockProperty for Sequential3 {
    /// 么九牌が含まれているか
    fn has_1_or_9(&self) -> Result<bool> {
        Ok(has_1_or_9(self.tiles[0])? || has_1_or_9(self.tiles[2])?)
    }
    /// 字牌が含まれているか
    fn has_honor(&self) -> Result<bool> {
        //字牌は順子にならない
        Ok(false)
    }
    /// 特定の風牌が含まれているか
    fn has_wind(&self, _: Wind) -> Result<bool> {
        // 字牌は順子にならない
        Ok(false)
    }
    /// 特定の三元牌が含まれているか
    fn has_dragon(&self, _: Dragon) -> Result<bool> {
        // 字牌は順子にならない
        Ok(false)
    }
    /// 萬子のブロックか
    fn is_character(&self) -> Result<bool> {
        is_character(self.tiles[0])
    }
    /// 筒子のブロックか
    fn is_circle(&self) -> Result<bool> {
        is_circle(self.tiles[0])
    }
    /// 索子のブロックか
    fn is_bamboo(&self) -> Result<bool> {
        is_bamboo(self.tiles[0])
    }
}

/// ユニットテスト
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_same2_normal() {
        assert_eq!(Same2::new(Tile::M1, Tile::M1).unwrap().get(), [Tile::M1; 2]);
    }
    #[test]
    #[should_panic]
    fn test_same2_errors_when_not_same() {
        Same2::new(Tile::M1, Tile::M2).unwrap();
    }
    #[test]
    fn test_same3_normal() {
        assert_eq!(
            Same3::new(Tile::M1, Tile::M1, Tile::M1).unwrap().get(),
            [Tile::M1; 3]
        );
    }
    #[test]
    #[should_panic]
    fn test_same3_errors_when_not_same() {
        Same3::new(Tile::M1, Tile::M1, Tile::M2).unwrap();
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
    #[should_panic]
    fn test_sequential2_errors_when_not_sequential() {
        Sequential2::new(Tile::M1, Tile::M4).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_sequential2_errors_when_honor() {
        Sequential2::new(Tile::Z1, Tile::Z2).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_sequential2_errors_when_other_kind() {
        Sequential2::new(Tile::M9, Tile::P1).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_sequential2_errors_when_other_kind2() {
        Sequential2::new(Tile::P8, Tile::S1).unwrap();
    }
    #[test]
    fn test_sequential3_normal() {
        assert_eq!(
            Sequential3::new(Tile::M1, Tile::M2, Tile::M3).unwrap().get(),
            [Tile::M1, Tile::M2, Tile::M3]
        );
    }
    #[test]
    #[should_panic]
    fn test_sequential3_errors_when_not_sequential() {
        Sequential3::new(Tile::M1, Tile::M2, Tile::M4).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_sequential3_errors_when_honor() {
        Sequential3::new(Tile::Z1, Tile::Z2, Tile::Z3).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_sequential3_errors_when_other_kind() {
        Sequential3::new(Tile::M8, Tile::M9, Tile::P1).unwrap();
    }
    #[test]
    #[should_panic]
    fn test_sequential3_errors_when_other_kind2() {
        Sequential3::new(Tile::P9, Tile::S1, Tile::S2).unwrap();
    }
}
