use crate::hand_info::meld::*;
use crate::tile::*;
use std::collections::VecDeque;
use std::fmt::{self, Write};

/// A player's hand (tehai / 手牌).
#[derive(Debug, Clone)]
pub struct Hand {
    /// Concealed tiles (13 when no melds have been called)
    tiles: Vec<Tile>,
    /// Melded (open) groups
    melds: Vec<Meld>,
    /// The tile just drawn, if any
    drawn: Option<Tile>,
}
impl Hand {
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn tiles_mut(&mut self) -> &mut Vec<Tile> {
        &mut self.tiles
    }

    pub fn set_drawn(&mut self, tile: Option<Tile>) {
        self.drawn = tile;
    }

    pub fn add_meld(&mut self, open: Meld) {
        self.melds.push(open);
    }

    /// Removes the tiles at the given indices from the concealed hand.
    pub fn remove_tiles_by_indices(&mut self, indices: &[usize]) {
        let mut unique_indices = indices.to_vec();
        unique_indices.sort_unstable_by(|a, b| b.cmp(a));
        unique_indices.dedup();
        for idx in unique_indices {
            if idx < self.tiles.len() {
                self.tiles.remove(idx);
            }
        }
    }

    pub fn new(tiles: Vec<Tile>, drawn: Option<Tile>) -> Hand {
        Hand::new_with_melds(tiles, Vec::new(), drawn)
    }
    pub fn new_with_melds(tiles: Vec<Tile>, melds: Vec<Meld>, drawn: Option<Tile>) -> Hand {
        Hand {
            tiles,
            drawn,
            melds,
        }
    }

    pub fn drawn(&self) -> Option<Tile> {
        self.drawn
    }

    pub fn melds(&self) -> &[Meld] {
        &self.melds
    }

    pub fn melds_mut(&mut self) -> &mut Vec<Meld> {
        &mut self.melds
    }

    pub fn sort(&mut self) {
        self.tiles.sort();
    }
    /// Counts the physical tiles of each kind, including melds and the drawn tile.
    ///
    /// A quad contributes four tiles even though its canonical `Meld::tiles`
    /// representation stores only three.
    pub fn summarize_tiles(&self) -> TileSummarize {
        let mut result: TileSummarize = [0; Tile::LEN];

        for i in 0..self.tiles.len() {
            result[self.tiles[i].get() as usize] += 1;
        }

        for meld in &self.melds {
            for tile in &meld.tiles {
                result[tile.get() as usize] += 1;
            }
            if meld.category.is_kan() && meld.tiles.len() == 3 {
                result[meld.kan_fourth_tile().get() as usize] += 1;
            }
        }

        if let Some(t) = self.drawn {
            result[t.get() as usize] += 1;
        }

        result
    }

    /// Renders the hand as Unicode mahjong-tile glyphs.
    pub fn to_emoji(&self) -> String {
        let mut result = String::new();
        for tile in &self.tiles {
            result.push(tile.to_char());
        }

        for meld in &self.melds {
            result.push(' ');
            for tile in meld.expanded_tiles() {
                result.push(tile.to_char());
            }
        }

        if let Some(tsumo) = self.drawn {
            let _ = write!(result, " {}", tsumo.to_char());
        }
        result
    }

    /// Renders tiles in compact notation, merging runs of the same suit ("123m").
    fn make_short_str(mut tiles: Vec<Tile>) -> String {
        if tiles.is_empty() {
            return String::new();
        } else if tiles.len() == 1 {
            return tiles[0].to_string();
        }
        tiles.sort();
        let mut result = String::new();
        let mut current_suit = None;
        for tile in tiles {
            let (rank, suit) = Self::short_tile_parts(tile);
            if let Some(prev_suit) = current_suit
                && suit != prev_suit
            {
                result.push(prev_suit);
            }
            result.push(rank);
            current_suit = Some(suit);
        }
        if let Some(suit) = current_suit {
            result.push(suit);
        }
        result
    }

    fn short_tile_parts(tile: Tile) -> (char, char) {
        match tile.get() {
            Tile::M1..=Tile::M9 => (Self::rank_char(tile.get() - Tile::M1), 'm'),
            Tile::P1..=Tile::P9 => (Self::rank_char(tile.get() - Tile::P1), 'p'),
            Tile::S1..=Tile::S9 => (Self::rank_char(tile.get() - Tile::S1), 's'),
            Tile::Z1..=Tile::Z7 => (Self::rank_char(tile.get() - Tile::Z1), 'z'),
            _ => ('?', '?'),
        }
    }

    fn rank_char(zero_based_rank: TileType) -> char {
        char::from(b'1' + zero_based_rank as u8)
    }

    /// Renders the hand as a string.
    ///
    /// Unlike `to_string`, runs in the same suit are compacted: `123m123p...`.
    pub fn to_short_string(&self) -> String {
        let tiles = self.tiles.clone();
        let mut result = Hand::make_short_str(tiles);

        for meld in &self.melds {
            let _ = write!(result, " {}", Hand::make_short_str(meld.expanded_tiles()));
        }

        if let Some(tsumo) = self.drawn {
            let _ = write!(result, " {tsumo}");
        }
        result
    }

    /// Parses tile notation ("123m4z" etc.) into tiles.
    fn str_to_tiles(hand_str: &str) -> Vec<Tile> {
        let mut result: Vec<Tile> = Vec::new();
        let mut stack: VecDeque<char> = VecDeque::new();
        for c in hand_str.chars() {
            if matches!(c, '1'..='9') {
                stack.push_back(c);
            } else if matches!(c, 'm' | 'p' | 's' | 'z') {
                while let Some(t) = stack.pop_front() {
                    // Honours only go up to 7z: "8z" and "9z" do not exist.
                    if (matches!(c, 'm' | 'p' | 's') || (c == 'z' && matches!(t, '1'..='7')))
                        && let Some(t) = Tile::from(&format!("{t}{c}"))
                    {
                        result.push(t);
                    }
                }
            }
        }
        result
    }

    pub fn from(hand_str: &str) -> Hand {
        let mut itr = hand_str.split_ascii_whitespace();
        let hand = Hand::str_to_tiles(itr.next().unwrap_or(""));
        let mut melds: Vec<Meld> = Vec::new();
        let mut drawn: Option<Tile> = None;

        for tile_str in itr {
            let tile_vec = Hand::str_to_tiles(tile_str);
            match tile_vec.len() {
                1 => {
                    drawn = Some(tile_vec[0]);
                }
                3 => {
                    melds.push(Meld {
                        tiles: tile_vec.clone(),
                        category: if tile_vec[0] == tile_vec[1] {
                            MeldType::Pon
                        } else {
                            MeldType::Chi
                        },
                        from: MeldFrom::Unknown,
                        called_tile: None,
                    });
                }
                4 => {
                    melds.push(Meld {
                        tiles: tile_vec[..3].to_vec(),
                        category: MeldType::Kan,
                        from: MeldFrom::Unknown,
                        called_tile: None,
                    });
                }
                _ => {}
            }
        }
        Hand::new_with_melds(hand, melds, drawn)
    }

    pub fn from_summarized(sum: &TileSummarize) -> Hand {
        let mut result: Vec<Tile> = Vec::new();

        for (i, &count) in sum
            .iter()
            .enumerate()
            .take(Tile::LEN)
            .skip(Tile::M1 as usize)
        {
            for _ in 0..count {
                result.push(Tile::new(i as TileType));
            }
        }
        Hand::new(result, None)
    }
}

/// Renders the hand as a string.
///
/// Unlike `to_short_string`, every tile is written as a full two-character
/// pair: `1m2m3m1p2p3p...`.
impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tile in &self.tiles {
            write!(f, "{tile}")?;
        }

        for meld in &self.melds {
            f.write_str(" ")?;
            for tile in meld.expanded_tiles() {
                write!(f, "{tile}")?;
            }
        }

        if let Some(tsumo) = self.drawn {
            write!(f, " {tsumo}")?;
        }

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summarize_test() {
        let test_str = "111m456p789s123z 4z";
        let test_hand = Hand::from(test_str);
        let test = test_hand.summarize_tiles();
        let answer = [
            3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1,
            1, 1, 0, 0, 0,
        ];
        assert_eq!(test, answer);
    }
    #[test]
    fn str_to_tiles_test() {
        let test = Hand::str_to_tiles("123m456p789s1234z");
        assert_eq!(test[0], Tile::new(Tile::M1));
        assert_eq!(test[1], Tile::new(Tile::M2));
        assert_eq!(test[2], Tile::new(Tile::M3));
        assert_eq!(test[3], Tile::new(Tile::P4));
        assert_eq!(test[4], Tile::new(Tile::P5));
        assert_eq!(test[5], Tile::new(Tile::P6));
        assert_eq!(test[6], Tile::new(Tile::S7));
        assert_eq!(test[7], Tile::new(Tile::S8));
        assert_eq!(test[8], Tile::new(Tile::S9));
        assert_eq!(test[9], Tile::new(Tile::Z1));
        assert_eq!(test[10], Tile::new(Tile::Z2));
        assert_eq!(test[11], Tile::new(Tile::Z3));
        assert_eq!(test[12], Tile::new(Tile::Z4));
    }
    #[test]
    fn str_to_tiles_test2() {
        let test = Hand::str_to_tiles("1m2m3m4p5p6p");
        assert_eq!(test[0], Tile::new(Tile::M1));
        assert_eq!(test[1], Tile::new(Tile::M2));
        assert_eq!(test[2], Tile::new(Tile::M3));
        assert_eq!(test[3], Tile::new(Tile::P4));
        assert_eq!(test[4], Tile::new(Tile::P5));
        assert_eq!(test[5], Tile::new(Tile::P6));
    }
    #[test]
    fn str_to_tiles_test3() {
        let test = Hand::str_to_tiles("");
        assert_eq!(test.len(), 0);
    }

    #[test]
    fn remove_tiles_by_indices_ignores_duplicate_indices() {
        let mut hand = Hand::from("1234m");

        hand.remove_tiles_by_indices(&[1, 1]);

        assert_eq!(hand.tiles(), Hand::str_to_tiles("134m"));
    }

    #[test]
    fn from_with_no_melds_test() {
        let test_str = "123m456p789s1115z 5z";
        let test = Hand::from(test_str);
        assert_eq!(test.tiles[0], Tile::new(Tile::M1));
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
    }

    #[test]
    fn from_with_chi_test() {
        let test_str = "123m456p1115z 789s 5z";
        let test = Hand::from(test_str);
        assert_eq!(test.tiles[0], Tile::new(Tile::M1));
        assert_eq!(test.melds[0].category, MeldType::Chi);
        assert_eq!(
            test.melds[0].tiles,
            vec![
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9)
            ]
        );
        assert_eq!(test.melds[0].from, MeldFrom::Unknown);
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
    }

    #[test]
    fn from_with_pon_test() {
        let test_str = "123m456p789s5z 111z 5z";
        let test = Hand::from(test_str);
        assert_eq!(test.tiles[0], Tile::new(Tile::M1));
        assert_eq!(test.melds[0].category, MeldType::Pon);
        assert_eq!(
            test.melds[0].tiles,
            vec![
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1)
            ]
        );
        assert_eq!(test.melds[0].from, MeldFrom::Unknown);
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
    }

    #[test]
    fn from_with_kan_test() {
        let test_str = "123m456p789s5z 1111z 5z";
        let test = Hand::from(test_str);
        assert_eq!(test.tiles[0], Tile::new(Tile::M1));
        assert_eq!(test.melds[0].category, MeldType::Kan);
        assert_eq!(
            test.melds[0].tiles,
            vec![
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1)
            ]
        );
        assert_eq!(test.melds[0].from, MeldFrom::Unknown);
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
        assert_eq!(test.summarize_tiles()[Tile::Z1 as usize], 4);
    }
}
