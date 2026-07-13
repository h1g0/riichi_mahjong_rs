//! The wall (haiyama / 牌山).
//!
//! Four-player: 136 tiles (34 kinds x 4, incl. 3 red fives).
//! Three-player: 108 tiles with 2m-8m removed (incl. 2 red fives).
//! Also splits off the 14-tile dead wall with its dora indicators and
//! replacement tiles.

use std::collections::VecDeque;

use mahjong_core::tile::{Tile, TileType};
use rand::seq::SliceRandom;

/// The wall.
pub struct Wall {
    /// Live wall; draws come from the front
    tiles: VecDeque<Tile>,
    /// Dead wall (14 tiles)
    dead_wall: Vec<Tile>,
    /// Index into dead_wall of the next replacement tile
    rinshan_index: usize,
    /// Number of revealed dora indicators (starts at 1, +1 per quad, max 5)
    dora_indicator_count: usize,
}

impl Wall {
    /// Creates the full tile set.
    ///
    /// Four-player: 136 tiles with red fives in 5m/5p/5s.
    /// Three-player: 108 tiles (2m-8m removed) with red fives in 5p/5s only.
    fn create_all_tiles(three_player: bool) -> Vec<Tile> {
        let mut tiles = Vec::with_capacity(136);

        for tile_type in 0..Tile::LEN as TileType {
            if three_player && (Tile::M2..=Tile::M8).contains(&tile_type) {
                continue;
            }
            for copy in 0..4u8 {
                // The first copy of each five becomes the red five.
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

    /// Creates a shuffled wall.
    pub fn new(three_player: bool) -> Self {
        let mut tiles = Self::create_all_tiles(three_player);
        tiles.shuffle(&mut rand::rng());
        Self::from_shuffled(tiles)
    }

    /// Creates a wall from a fixed seed, for simulations and
    /// reproducible tests.
    pub fn new_with_seed(seed: u64, three_player: bool) -> Self {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let mut tiles = Self::create_all_tiles(three_player);
        tiles.shuffle(&mut rng);
        Self::from_shuffled(tiles)
    }

    /// Test helper: builds a wall from a fixed tile order, unshuffled.
    #[cfg(test)]
    pub fn from_tiles(tiles: Vec<Tile>) -> Self {
        Self::from_shuffled(tiles)
    }

    /// Splits the last 14 tiles off as the dead wall.
    fn from_shuffled(mut tiles: Vec<Tile>) -> Self {
        let dead_wall: Vec<Tile> = tiles.split_off(tiles.len() - 14);
        Wall {
            tiles: tiles.into(),
            dead_wall,
            rinshan_index: 0,
            dora_indicator_count: 1,
        }
    }

    /// Draws from the front of the live wall.
    pub fn draw(&mut self) -> Option<Tile> {
        self.tiles.pop_front()
    }

    /// Draws a replacement tile (rinshan / 嶺上牌) from the dead wall.
    pub fn draw_rinshan(&mut self) -> Option<Tile> {
        if self.rinshan_index >= 4 {
            return None; // Only four replacement tiles exist.
        }
        let tile = self.dead_wall[self.rinshan_index];
        self.rinshan_index += 1;
        Some(tile)
    }

    /// Reveals one more dora indicator after a quad.
    pub fn add_dora_indicator(&mut self) {
        if self.dora_indicator_count < 5 {
            self.dora_indicator_count += 1;
        }
    }

    /// Returns the currently revealed dora indicators.
    ///
    /// Dead wall layout: [rinshan 0-3, then indicator/ura pairs], so the
    /// indicators sit at dead_wall[4], [6], [8], [10], [12].
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

    /// Returns the ura dora indicators (revealed only on a riichi win):
    /// dead_wall[5], [7], [9], [11], [13].
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

    /// Number of tiles left in the live wall.
    pub fn remaining(&self) -> usize {
        self.tiles.len()
    }

    /// Whether the live wall is exhausted (exhaustive draw check).
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Draws from the back of the live wall (replacement for a North
    /// extraction in three-player games).
    ///
    /// The dead wall is deliberately never replenished in this codebase, so
    /// the replacement comes from the live wall's tail; `remaining()` and
    /// the last-tile (haitei) bookkeeping then stay consistent for free.
    pub fn draw_replacement_from_tail(&mut self) -> Option<Tile> {
        self.tiles.pop_back()
    }

    /// Deals starting hands: three rounds of four tiles plus one, 13 each.
    /// Returns four hands; in three-player games seat 3 stays empty.
    pub fn deal(&mut self, player_count: usize) -> [Vec<Tile>; 4] {
        let mut hands: [Vec<Tile>; 4] = [
            Vec::with_capacity(13),
            Vec::with_capacity(13),
            Vec::with_capacity(13),
            Vec::with_capacity(13),
        ];

        for _ in 0..3 {
            for hand in hands.iter_mut().take(player_count) {
                for _ in 0..4 {
                    if let Some(tile) = self.draw() {
                        hand.push(tile);
                    }
                }
            }
        }

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

        for tile_type in 0..Tile::LEN as TileType {
            let count = tiles.iter().filter(|t| t.get() == tile_type).count();
            assert_eq!(count, 4, "Tile type {} should have 4 copies", tile_type);
        }

        let red_count = tiles.iter().filter(|t| t.is_red_dora()).count();
        assert_eq!(red_count, 3);

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
        // (34 - 7) kinds x 4 copies
        assert_eq!(tiles.len(), 108);

        for tile_type in Tile::M2..=Tile::M8 {
            let count = tiles.iter().filter(|t| t.get() == tile_type).count();
            assert_eq!(
                count, 0,
                "Tile type {} should not exist in sanma",
                tile_type
            );
        }

        for tile_type in 0..Tile::LEN as TileType {
            if (Tile::M2..=Tile::M8).contains(&tile_type) {
                continue;
            }
            let count = tiles.iter().filter(|t| t.get() == tile_type).count();
            assert_eq!(count, 4, "Tile type {} should have 4 copies", tile_type);
        }

        // Only the 5p and 5s red fives exist in three-player games.
        let red_count = tiles.iter().filter(|t| t.is_red_dora()).count();
        assert_eq!(red_count, 2);
        assert!(!tiles.iter().any(|t| t.get() == Tile::M5 && t.is_red_dora()));
    }

    #[test]
    fn test_wall_new_three_player() {
        let wall = Wall::new(true);
        // 108 - 14 dead wall = 94
        assert_eq!(wall.tiles.len(), 94);
        assert_eq!(wall.dead_wall.len(), 14);
    }

    #[test]
    fn test_deal_three_player() {
        let mut wall = Wall::new(true);
        let hands = wall.deal(3);

        for (i, hand) in hands.iter().enumerate().take(3) {
            assert_eq!(hand.len(), 13, "Player {} should have 13 tiles", i);
        }
        assert!(hands[3].is_empty());

        // 94 - 3 x 13 = 55
        assert_eq!(wall.remaining(), 55);
    }

    #[test]
    fn test_draw_replacement_from_tail() {
        let mut wall = Wall::new(true);
        let before = wall.remaining();

        let tile = wall.draw_replacement_from_tail();
        assert!(tile.is_some());
        assert_eq!(wall.remaining(), before - 1);

        // Front draws and tail draws must both shrink the same count.
        let head = wall.draw().unwrap();
        assert_eq!(wall.remaining(), before - 2);
        let _ = head;
    }

    #[test]
    fn test_wall_new() {
        let wall = Wall::new(false);
        // 136 - 14 dead wall = 122
        assert_eq!(wall.tiles.len(), 122);
        assert_eq!(wall.dead_wall.len(), 14);
        assert_eq!(wall.dora_indicator_count, 1);
        assert_eq!(wall.dora_indicators().len(), 1);
    }

    #[test]
    fn test_deal() {
        let mut wall = Wall::new(false);
        let hands = wall.deal(4);

        for (i, hand) in hands.iter().enumerate() {
            assert_eq!(hand.len(), 13, "Player {} should have 13 tiles", i);
        }

        // 122 - 4 x 13 = 70
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

        for i in 0..4 {
            let tile = wall.draw_rinshan();
            assert!(tile.is_some(), "Rinshan draw {} should succeed", i);
        }

        // The fifth replacement draw must fail.
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

        // Capped at five indicators.
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
