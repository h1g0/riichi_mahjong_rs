//! 牌山の管理
//!
//! 136枚の牌（各34種×4枚、うち赤ドラ3枚）を管理する。
//! 王牌（14枚）・ドラ表示牌・嶺上牌の分離も行う。

use mahjong_core::tile::{Tile, TileType};
use rand::seq::SliceRandom;

/// 牌山
pub struct Wall {
    /// ツモ牌（通常の山）: インデックスが小さい方からツモる
    tiles: Vec<Tile>,
    /// 王牌（14枚）
    dead_wall: Vec<Tile>,
    /// 嶺上牌のうち次にツモる位置（dead_wall 内のインデックス）
    rinshan_index: usize,
    /// ドラ表示牌の公開枚数（初期1枚、カンするたびに増加、最大5枚）
    dora_indicator_count: usize,
}

impl Wall {
    /// 136枚の牌を生成する（赤ドラ3枚含む）
    fn create_all_tiles() -> Vec<Tile> {
        let mut tiles = Vec::with_capacity(136);

        for tile_type in 0..Tile::LEN as TileType {
            for copy in 0..4u8 {
                // 赤ドラ: 5m, 5p, 5s の各1枚目を赤にする
                let is_red = copy == 0
                    && (tile_type == Tile::M5
                        || tile_type == Tile::P5
                        || tile_type == Tile::S5);

                if is_red {
                    tiles.push(Tile::new_red(tile_type));
                } else {
                    tiles.push(Tile::new(tile_type));
                }
            }
        }

        tiles
    }

    /// 牌山を生成してシャッフルする
    pub fn new() -> Self {
        let mut tiles = Self::create_all_tiles();
        tiles.shuffle(&mut rand::rng());

        // 末尾14枚を王牌として分離
        let dead_wall: Vec<Tile> = tiles.split_off(tiles.len() - 14);

        Wall {
            tiles,
            dead_wall,
            rinshan_index: 0,
            dora_indicator_count: 1,
        }
    }

    /// テスト用：指定した牌列で牌山を生成する（シャッフルなし）
    #[cfg(test)]
    pub fn from_tiles(mut tiles: Vec<Tile>) -> Self {
        let dead_wall: Vec<Tile> = tiles.split_off(tiles.len() - 14);
        Wall {
            tiles,
            dead_wall,
            rinshan_index: 0,
            dora_indicator_count: 1,
        }
    }

    /// 通常のツモを行う（山の先頭から1枚引く）
    pub fn draw(&mut self) -> Option<Tile> {
        if self.tiles.is_empty() {
            return None;
        }
        Some(self.tiles.remove(0))
    }

    /// 嶺上牌をツモる（王牌の嶺上牌位置から1枚引く）
    pub fn draw_rinshan(&mut self) -> Option<Tile> {
        if self.rinshan_index >= 4 {
            return None; // 嶺上牌は最大4枚
        }
        let tile = self.dead_wall[self.rinshan_index];
        self.rinshan_index += 1;
        Some(tile)
    }

    /// カン時にドラ表示牌を追加で公開する
    pub fn add_dora_indicator(&mut self) {
        if self.dora_indicator_count < 5 {
            self.dora_indicator_count += 1;
        }
    }

    /// 現在公開されているドラ表示牌を返す
    /// 王牌の配置: [嶺上0, 嶺上1, 嶺上2, 嶺上3, ドラ表示0, ?, ドラ表示1, ?, ドラ表示2, ?, ドラ表示3, ?, ドラ表示4, ?]
    /// ドラ表示牌は dead_wall[4], dead_wall[6], dead_wall[8], dead_wall[10], dead_wall[12]
    pub fn dora_indicators(&self) -> Vec<Tile> {
        let mut result = Vec::with_capacity(self.dora_indicator_count);
        for i in 0..self.dora_indicator_count {
            let idx = 4 + i * 2;
            if idx < self.dead_wall.len() {
                result.push(self.dead_wall[idx]);
            }
        }
        result
    }

    /// 裏ドラ表示牌を返す（和了時のみ公開される）
    /// dead_wall[5], dead_wall[7], dead_wall[9], dead_wall[11], dead_wall[13]
    pub fn uradora_indicators(&self) -> Vec<Tile> {
        let mut result = Vec::with_capacity(self.dora_indicator_count);
        for i in 0..self.dora_indicator_count {
            let idx = 5 + i * 2;
            if idx < self.dead_wall.len() {
                result.push(self.dead_wall[idx]);
            }
        }
        result
    }

    /// 山の残り枚数を返す
    pub fn remaining(&self) -> usize {
        self.tiles.len()
    }

    /// 山が空かどうか（流局判定用）
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// 配牌を行う（4枚×3回+1枚 = 13枚を各プレイヤーに配る）
    /// 戻り値: 4人分の手牌（各13枚）
    pub fn deal(&mut self) -> [Vec<Tile>; 4] {
        let mut hands: [Vec<Tile>; 4] = [
            Vec::with_capacity(13),
            Vec::with_capacity(13),
            Vec::with_capacity(13),
            Vec::with_capacity(13),
        ];

        // 4枚ずつ3回配る
        for _ in 0..3 {
            for player in 0..4 {
                for _ in 0..4 {
                    if let Some(tile) = self.draw() {
                        hands[player].push(tile);
                    }
                }
            }
        }

        // 1枚ずつ配る
        for player in 0..4 {
            if let Some(tile) = self.draw() {
                hands[player].push(tile);
            }
        }

        hands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_all_tiles() {
        let tiles = Wall::create_all_tiles();
        assert_eq!(tiles.len(), 136);

        // 各種類が4枚ずつあることを確認
        for tile_type in 0..Tile::LEN as TileType {
            let count = tiles.iter().filter(|t| t.get() == tile_type).count();
            assert_eq!(count, 4, "Tile type {} should have 4 copies", tile_type);
        }

        // 赤ドラが3枚あることを確認
        let red_count = tiles.iter().filter(|t| t.is_red_dora()).count();
        assert_eq!(red_count, 3);

        // 赤ドラがそれぞれ5m, 5p, 5sであることを確認
        let red_5m = tiles
            .iter()
            .filter(|t| t.get() == Tile::M5 && t.is_red_dora())
            .count();
        let red_5p = tiles
            .iter()
            .filter(|t| t.get() == Tile::P5 && t.is_red_dora())
            .count();
        let red_5s = tiles
            .iter()
            .filter(|t| t.get() == Tile::S5 && t.is_red_dora())
            .count();
        assert_eq!(red_5m, 1);
        assert_eq!(red_5p, 1);
        assert_eq!(red_5s, 1);
    }

    #[test]
    fn test_wall_new() {
        let wall = Wall::new();
        // 122枚が通常山（136 - 14 = 122）
        assert_eq!(wall.tiles.len(), 122);
        // 14枚が王牌
        assert_eq!(wall.dead_wall.len(), 14);
        // ドラ表示牌は1枚
        assert_eq!(wall.dora_indicator_count, 1);
        assert_eq!(wall.dora_indicators().len(), 1);
    }

    #[test]
    fn test_deal() {
        let mut wall = Wall::new();
        let hands = wall.deal();

        // 各プレイヤー13枚
        for i in 0..4 {
            assert_eq!(hands[i].len(), 13, "Player {} should have 13 tiles", i);
        }

        // 配牌後の山の残り枚数: 122 - 52 = 70
        assert_eq!(wall.remaining(), 70);
    }

    #[test]
    fn test_draw() {
        let mut wall = Wall::new();
        let initial_remaining = wall.remaining();

        let tile = wall.draw();
        assert!(tile.is_some());
        assert_eq!(wall.remaining(), initial_remaining - 1);
    }

    #[test]
    fn test_draw_rinshan() {
        let mut wall = Wall::new();

        // 嶺上牌は4枚まで引ける
        for i in 0..4 {
            let tile = wall.draw_rinshan();
            assert!(tile.is_some(), "Rinshan draw {} should succeed", i);
        }

        // 5枚目はNone
        let tile = wall.draw_rinshan();
        assert!(tile.is_none());
    }

    #[test]
    fn test_dora_indicators() {
        let mut wall = Wall::new();

        assert_eq!(wall.dora_indicators().len(), 1);
        assert_eq!(wall.uradora_indicators().len(), 1);

        wall.add_dora_indicator();
        assert_eq!(wall.dora_indicators().len(), 2);
        assert_eq!(wall.uradora_indicators().len(), 2);

        // 最大5枚まで
        for _ in 0..10 {
            wall.add_dora_indicator();
        }
        assert_eq!(wall.dora_indicators().len(), 5);
        assert_eq!(wall.uradora_indicators().len(), 5);
    }

    #[test]
    fn test_wall_exhaustion() {
        let mut wall = Wall::new();
        let remaining = wall.remaining();

        for _ in 0..remaining {
            assert!(!wall.is_empty());
            wall.draw();
        }

        assert!(wall.is_empty());
        assert!(wall.draw().is_none());
    }
}
