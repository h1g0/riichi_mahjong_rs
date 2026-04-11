use crate::hand_info::meld::*;
use crate::tile::*;
use std::collections::VecDeque;
use std::fmt;

/// 手牌
#[derive(Debug, Clone)]
pub struct Hand {
    /// 現在の手牌（副露がなければ13枚）
    tiles: Vec<Tile>,
    /// 副露
    melds: Vec<Meld>,
    /// ツモってきた牌
    drawn: Option<Tile>,
}
impl Hand {
    /// 手牌の参照を返す
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// 手牌の可変参照を返す
    pub fn tiles_mut(&mut self) -> &mut Vec<Tile> {
        &mut self.tiles
    }

    /// ツモ牌をセットする
    pub fn set_drawn(&mut self, tile: Option<Tile>) {
        self.drawn = tile;
    }

    /// 副露を追加する
    pub fn add_meld(&mut self, open: Meld) {
        self.melds.push(open);
    }

    /// 指定インデックスの牌を手牌から除去する
    pub fn remove_tiles_by_indices(&mut self, indices: &mut [usize]) {
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for &idx in indices.iter() {
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

    /// ツモった牌を返す
    pub fn drawn(&self) -> Option<Tile> {
        self.drawn
    }

    /// 副露を返す
    pub fn melds(&self) -> &[Meld] {
        &self.melds
    }

    /// 副露の可変参照を返す
    pub fn melds_mut(&mut self) -> &mut Vec<Meld> {
        &mut self.melds
    }

    /// 手牌をソートする
    pub fn sort(&mut self) {
        self.tiles.sort();
    }
    /// 種類別に各牌の数をカウントする
    pub fn summarize_tiles(&self) -> TileSummarize {
        let mut result: TileSummarize = [0; Tile::LEN];

        // 通常の手牌をカウント
        for i in 0..self.tiles.len() {
            result[self.tiles[i].get() as usize] += 1;
        }

        // 鳴いている牌があればカウント
        //
        // 解析用途では副露は常に1面子として扱う。槓子の4枚目まで数えると
        // 「4面子1雀頭に加えて孤立牌が1枚ある」手を和了形と誤認しうる。
        for i in 0..self.melds.len() {
            for j in 0..self.melds[i].tiles.len() {
                result[self.melds[i].tiles[j].get() as usize] += 1;
            }
        }

        // ツモった牌があればカウント
        if let Some(t) = self.drawn {
            result[t.get() as usize] += 1;
        }

        result
    }

    /// 絵文字として出力する
    pub fn to_emoji(&self) -> String {
        let mut result = String::new();
        for i in 0..self.tiles.len() {
            result.push(self.tiles[i].to_char());
        }

        for i in 0..self.melds.len() {
            result.push_str(&format!(
                " {}{}{}",
                self.melds[i].tiles[0].to_char(),
                self.melds[i].tiles[1].to_char(),
                self.melds[i].tiles[2].to_char()
            ));
            // カンなら4枚目を追加する
            if self.melds[i].category.is_kan() {
                result.push(self.melds[i].tiles[0].to_char());
            }
        }

        if let Some(tsumo) = self.drawn {
            result.push_str(&format!(" {}", tsumo.to_char()));
        }
        result
    }

    /// `Vec<Tile>`から連続した牌の種類を圧縮した文字列を返す
    fn make_short_str(mut tiles: Vec<Tile>) -> String {
        if tiles.is_empty() {
            return String::from("");
        } else if tiles.len() == 1 {
            return tiles[0].to_string();
        }
        tiles.sort();
        let mut result = String::new();
        let mut prev_suit = 'x';
        for i in 0..tiles.len() {
            let now_suit = tiles[i].to_string().chars().nth(1).unwrap();
            if i > 0 {
                result.push(tiles[i - 1].to_string().chars().nth(0).unwrap());
                if now_suit != prev_suit {
                    result.push(prev_suit);
                }
            }
            if i == tiles.len() - 1 {
                result.push(tiles[i].to_string().chars().nth(0).unwrap());
                result.push(now_suit);
                break;
            }
            prev_suit = now_suit;
        }
        result
    }

    /// 文字列として出力する
    ///
    /// `to_string`と違い、こちらは連続した牌の種類は省略して`123m123p...`と出力する。
    pub fn to_short_string(&self) -> String {
        let tiles = self.tiles.clone();
        let mut result = Hand::make_short_str(tiles);

        for i in 0..self.melds.len() {
            let mut op_tiles = self.melds[i].tiles.clone();
            if self.melds[i].category.is_kan() && op_tiles.len() == 3 {
                op_tiles.push(self.melds[i].tiles[0]);
            }
            result.push_str(&format!(" {}", Hand::make_short_str(op_tiles)));
        }

        if let Some(tsumo) = self.drawn {
            result.push_str(&format!(" {tsumo}"));
        }
        result
    }

    /// 文字列から`Vec<Tile>`を返す
    fn str_to_tiles(hand_str: &str) -> Vec<Tile> {
        let mut result: Vec<Tile> = Vec::new();
        let mut stack: VecDeque<char> = VecDeque::new();
        for c in hand_str.chars() {
            if matches!(c, '1'..='9') {
                stack.push_back(c);
            } else if matches!(c, 'm' | 'p' | 's' | 'z') {
                while let Some(t) = stack.pop_front() {
                    // 字牌の場合は`8z`と`9z`は存在しない
                    if (matches!(c, 'm' | 'p' | 's') || (c == 'z' && matches!(t, '1'..='7')))
                        && let Some(t) = Tile::from(&format!("{}{}", t, c))
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
                    let t = *tile_vec.first().unwrap();
                    drawn = Some(t);
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

/// 文字列として出力する
///
/// `to_short_string`と違い、こちらは牌の種類を省略せずに`1m2m3m1p2p3p...`と必ず2文字単位で出力する。
impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tile in &self.tiles {
            write!(f, "{tile}")?;
        }

        for meld in &self.melds {
            write!(f, " {}{}{}", meld.tiles[0], meld.tiles[1], meld.tiles[2])?;
            // カンなら4枚目を追加する
            if meld.category.is_kan() {
                write!(f, "{}", meld.tiles[0])?;
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
    }
}
