//! 牌山の管理
//!
//! 四麻: 136枚の牌（各34種×4枚、うち赤ドラ3枚）を管理する。
//! 三麻: 萬子2〜8を除いた108枚（うち赤ドラ2枚）を管理する。
//! 王牌（14枚）・ドラ表示牌・嶺上牌の分離も行う。

use std::collections::VecDeque;

use mahjong_core::tile::{Tile, TileType};
use rand::seq::SliceRandom;

/// 牌山
pub struct Wall {
    /// ツモ牌（通常の山）: 先頭からツモる
    tiles: VecDeque<Tile>,
    /// 王牌（14枚）
    dead_wall: Vec<Tile>,
    /// 嶺上牌のうち次にツモる位置（dead_wall 内のインデックス）
    rinshan_index: usize,
    /// ドラ表示牌の公開枚数（初期1枚、カンするたびに増加、最大5枚）
    dora_indicator_count: usize,
}

impl Wall {
    /// 全ての牌を生成する
    ///
    /// 四麻: 136枚（34種×4枚、赤ドラは5m/5p/5sの3枚）
    /// 三麻: 108枚（萬子2〜8を除外した27種×4枚、赤ドラは5p/5sの2枚）
    fn create_all_tiles(three_player: bool) -> Vec<Tile> {
        let mut tiles = Vec::with_capacity(136);

        for tile_type in 0..Tile::LEN as TileType {
            // 三麻では萬子2〜8を使用しない
            if three_player && (Tile::M2..=Tile::M8).contains(&tile_type) {
                continue;
            }
            for copy in 0..4u8 {
                // 赤ドラ: 5m, 5p, 5s の各1枚目を赤にする（三麻では5mが存在しないため5p/5sのみ）
                let is_red = copy == 0
                    && (tile_type == Tile::M5 || tile_type == Tile::P5 || tile_type == Tile::S5);

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
    pub fn new(three_player: bool) -> Self {
        let mut tiles = Self::create_all_tiles(three_player);
        tiles.shuffle(&mut rand::rng());
        Self::from_shuffled(tiles)
    }

    /// 固定シードで牌山を生成する（再現性のある乱数）
    ///
    /// シミュレーション・再現性のあるテストに使用する。
    pub fn new_with_seed(seed: u64, three_player: bool) -> Self {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let mut tiles = Self::create_all_tiles(three_player);
        tiles.shuffle(&mut rng);
        Self::from_shuffled(tiles)
    }

    /// テスト用：指定した牌列で牌山を生成する（シャッフルなし）
    #[cfg(test)]
    pub fn from_tiles(tiles: Vec<Tile>) -> Self {
        Self::from_shuffled(tiles)
    }

    /// 並び順確定済みの136枚から、末尾14枚を王牌として分離して牌山を作る
    fn from_shuffled(mut tiles: Vec<Tile>) -> Self {
        let dead_wall: Vec<Tile> = tiles.split_off(tiles.len() - 14);
        Wall {
            tiles: tiles.into(),
            dead_wall,
            rinshan_index: 0,
            dora_indicator_count: 1,
        }
    }

    /// 通常のツモを行う（山の先頭から1枚引く）
    pub fn draw(&mut self) -> Option<Tile> {
        self.tiles.pop_front()
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

    /// 山の末尾から1枚引く（三麻の北抜きの補充用）
    ///
    /// 王牌は補充しないという既存の簡略化に合わせて、補充は生牌山の末尾から行う。
    /// これにより `remaining()` と海底の計算が自動的に整合する。
    pub fn draw_replacement_from_tail(&mut self) -> Option<Tile> {
        self.tiles.pop_back()
    }

    /// 配牌を行う（4枚×3回+1枚 = 13枚を各プレイヤーに配る）
    /// 戻り値: 4人分の手牌（各13枚）。三麻ではシート3は空のまま。
    pub fn deal(&mut self, player_count: usize) -> [Vec<Tile>; 4] {
        let mut hands: [Vec<Tile>; 4] = [
            Vec::with_capacity(13),
            Vec::with_capacity(13),
            Vec::with_capacity(13),
            Vec::with_capacity(13),
        ];

        // 4枚ずつ3回配る
        for _ in 0..3 {
            for hand in hands.iter_mut().take(player_count) {
                for _ in 0..4 {
                    if let Some(tile) = self.draw() {
                        hand.push(tile);
                    }
                }
            }
        }

        // 1枚ずつ配る
        for hand in hands.iter_mut().take(player_count) {
            if let Some(tile) = self.draw() {
                hand.push(tile);
            }
        }

        hands
    }
}

impl Default for Wall {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_all_tiles() {
        let tiles = Wall::create_all_tiles(false);
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
    fn test_create_all_tiles_three_player() {
        let tiles = Wall::create_all_tiles(true);
        // 108枚（(34 - 7)種 × 4枚）
        assert_eq!(tiles.len(), 108);

        // 萬子2〜8が存在しないことを確認
        for tile_type in Tile::M2..=Tile::M8 {
            let count = tiles.iter().filter(|t| t.get() == tile_type).count();
            assert_eq!(
                count, 0,
                "Tile type {} should not exist in sanma",
                tile_type
            );
        }

        // 1m・9mと萬子以外は4枚ずつあることを確認
        for tile_type in 0..Tile::LEN as TileType {
            if (Tile::M2..=Tile::M8).contains(&tile_type) {
                continue;
            }
            let count = tiles.iter().filter(|t| t.get() == tile_type).count();
            assert_eq!(count, 4, "Tile type {} should have 4 copies", tile_type);
        }

        // 赤ドラは5p/5sの2枚のみ
        let red_count = tiles.iter().filter(|t| t.is_red_dora()).count();
        assert_eq!(red_count, 2);
        assert!(!tiles.iter().any(|t| t.get() == Tile::M5 && t.is_red_dora()));
    }

    #[test]
    fn test_wall_new_three_player() {
        let wall = Wall::new(true);
        // 94枚が通常山（108 - 14 = 94）
        assert_eq!(wall.tiles.len(), 94);
        // 14枚が王牌
        assert_eq!(wall.dead_wall.len(), 14);
    }

    #[test]
    fn test_deal_three_player() {
        let mut wall = Wall::new(true);
        let hands = wall.deal(3);

        // シート0〜2は13枚、シート3は空
        for (i, hand) in hands.iter().enumerate().take(3) {
            assert_eq!(hand.len(), 13, "Player {} should have 13 tiles", i);
        }
        assert!(hands[3].is_empty());

        // 配牌後の山の残り枚数: 94 - 39 = 55
        assert_eq!(wall.remaining(), 55);
    }

    #[test]
    fn test_draw_replacement_from_tail() {
        let mut wall = Wall::new(true);
        let before = wall.remaining();

        // 末尾からの補充ツモで残り枚数が減る
        let tile = wall.draw_replacement_from_tail();
        assert!(tile.is_some());
        assert_eq!(wall.remaining(), before - 1);

        // 通常のツモ（先頭）とは別の牌を引いている
        let head = wall.draw().unwrap();
        assert_eq!(wall.remaining(), before - 2);
        // 先頭と末尾は独立して減る（枚数勘定のみ確認）
        let _ = head;
    }

    #[test]
    fn test_wall_new() {
        let wall = Wall::new(false);
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
        let mut wall = Wall::new(false);
        let hands = wall.deal(4);

        // 各プレイヤー13枚
        for (i, hand) in hands.iter().enumerate() {
            assert_eq!(hand.len(), 13, "Player {} should have 13 tiles", i);
        }

        // 配牌後の山の残り枚数: 122 - 52 = 70
        assert_eq!(wall.remaining(), 70);
    }

    #[test]
    fn test_draw() {
        let mut wall = Wall::new(false);
        let initial_remaining = wall.remaining();

        let tile = wall.draw();
        assert!(tile.is_some());
        assert_eq!(wall.remaining(), initial_remaining - 1);
    }

    #[test]
    fn test_draw_rinshan() {
        let mut wall = Wall::new(false);

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
        let mut wall = Wall::new(false);

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
        let mut wall = Wall::new(false);
        let remaining = wall.remaining();

        for _ in 0..remaining {
            assert!(!wall.is_empty());
            wall.draw();
        }

        assert!(wall.is_empty());
        assert!(wall.draw().is_none());
    }
}
