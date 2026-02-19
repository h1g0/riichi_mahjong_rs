use crate::hand_info::opened::*;
use crate::tile::*;
use std::collections::VecDeque;

/// 手牌
#[derive(Debug)]
pub struct Hand {
    /// 現在の手牌（副露がなければ13枚）
    tiles: Vec<Tile>,
    /// 副露
    opened: Vec<OpenTiles>,
    /// ツモってきた牌
    drawn: Option<Tile>,
}
impl Hand {
    pub fn new(tiles: Vec<Tile>, drawn: Option<Tile>) -> Hand {
        return Hand::new_with_opened(tiles, Vec::new(), drawn);
    }
    pub fn new_with_opened(tiles: Vec<Tile>, opened: Vec<OpenTiles>, drawn: Option<Tile>) -> Hand {
        Hand {
            tiles,
            drawn,
            opened,
        }
    }

    /// ツモった牌を返す
    pub fn drawn(&self) -> Option<Tile> {
        self.drawn
    }

    /// 手牌をソートする
    pub fn sort(&mut self) {
        self.tiles.sort();
    }
    /// 種類別に各牌の数をカウントする
    pub fn summarize_tiles(&self) -> TileSummarize {
        let mut result: TileSummarize = [0; Tile::LEN as usize];

        // 通常の手牌をカウント
        for i in 0..self.tiles.len() {
            result[self.tiles[i].get() as usize] += 1;
        }

        // 鳴いている牌があればカウント
        for i in 0..self.opened.len() {
            for j in 0..self.opened[i].tiles.len() {
                result[self.opened[i].tiles[j].get() as usize] += 1;
            }
        }

        // ツモった牌があればカウント
        if let Some(t) = self.drawn {
            result[t.get() as usize] += 1;
        }

        return result;
    }

    /// 絵文字として出力する
    pub fn to_emoji(&self) -> String {
        let mut result = String::new();
        for i in 0..self.tiles.len() {
            result.push(self.tiles[i].to_char());
        }

        for i in 0..self.opened.len() {
            result.push_str(&format!(
                " {}{}{}",
                self.opened[i].tiles[0].to_char(),
                self.opened[i].tiles[1].to_char(),
                self.opened[i].tiles[2].to_char()
            ))
        }

        if let Some(tsumo) = self.drawn {
            result.push_str(&format!(" {}", tsumo.to_char()));
        }
        return result;
    }

    /// 文字列として出力する
    ///
    /// `to_short_string`と違い、こちらは牌の種類を省略せずに`1m2m3m1p2p3p...`と必ず2文字単位で出力する。
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        for i in 0..self.tiles.len() {
            result.push_str(&self.tiles[i].to_string());
        }

        for i in 0..self.opened.len() {
            result.push_str(&format!(
                " {}{}{}",
                self.opened[i].tiles[0].to_string(),
                self.opened[i].tiles[1].to_string(),
                self.opened[i].tiles[2].to_string()
            ));
            // カンなら4枚目を追加する
            if self.opened[i].category == OpenType::Kan {
                result.push_str(&format!("{}", self.opened[i].tiles[0].to_string(),));
            }
        }

        if let Some(tsumo) = self.drawn {
            result.push_str(&format!(" {}", tsumo.to_string()));
        }
        return result;
    }

    /// `Vec<Tile>`から連続した牌の種類を圧縮した文字列を返す
    fn make_short_str(mut tiles: Vec<Tile>) -> String {
        if tiles.len() == 0 {
            return String::from("");
        } else if tiles.len() == 1 {
            return tiles[1].to_string();
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
        return result;
    }

    /// 文字列として出力する
    ///
    /// `to_string`と違い、こちらは連続した牌の種類は省略して`123m123p...`と出力する。
    pub fn to_short_string(&self) -> String {
        let tiles = self.tiles.clone();
        let mut result = Hand::make_short_str(tiles);

        for i in 0..self.opened.len() {
            let mut op_tiles = Vec::from(self.opened[i].tiles);
            if self.opened[i].category == OpenType::Kan {
                op_tiles.push(self.opened[i].tiles[0]);
            }
            result.push_str(&format!(" {}", Hand::make_short_str(op_tiles)));
        }

        if let Some(tsumo) = self.drawn {
            result.push_str(&format!(" {}", tsumo.to_string()));
        }
        return result;
    }

    /// 文字列から`Vec<Tile>`を返す
    fn str_to_tiles(hand_str: &str) -> Vec<Tile> {
        let mut result: Vec<Tile> = Vec::new();
        let mut stack: VecDeque<char> = VecDeque::new();
        let mut itr = hand_str.chars();
        while let Some(c) = itr.next() {
            if matches!(c, '1'..='9') {
                stack.push_back(c);
            } else if matches!(c, 'm' | 'p' | 's' | 'z') {
                while let Some(t) = stack.pop_front() {
                    // 字牌の場合は`8z`と`9z`は存在しない
                    if matches!(c, 'm' | 'p' | 's') || (c == 'z' && matches!(t, '1'..='7')) {
                        if let Some(t) = Tile::from(&format!("{}{}", t, c)) {
                            result.push(t);
                        }
                    }
                }
            }
        }
        return result;
    }

    pub fn from(hand_str: &str) -> Hand {
        let mut itr = hand_str.split_ascii_whitespace();
        let hand = Hand::str_to_tiles(itr.next().unwrap_or(""));
        let mut opened: Vec<OpenTiles> = Vec::new();
        let mut drawn: Option<Tile> = None;

        while let Some(tile_str) = itr.next() {
            let tile_vec = Hand::str_to_tiles(tile_str);
            match tile_vec.len() {
                1 => {
                    let t = *tile_vec.get(0).unwrap();
                    drawn = Some(t);
                }
                3 => {
                    opened.push(OpenTiles {
                        tiles: [
                            *tile_vec.get(0).unwrap(),
                            *tile_vec.get(1).unwrap(),
                            *tile_vec.get(2).unwrap(),
                        ],
                        category: if *tile_vec.get(0).unwrap() == *tile_vec.get(1).unwrap() {
                            OpenType::Pon
                        } else {
                            OpenType::Chi
                        },
                        from: OpenFrom::Unknown,
                    });
                }
                4 => {
                    opened.push(OpenTiles {
                        tiles: [
                            *tile_vec.get(0).unwrap(),
                            *tile_vec.get(1).unwrap(),
                            *tile_vec.get(2).unwrap(),
                        ],
                        category: OpenType::Kan,
                        from: OpenFrom::Unknown,
                    });
                }
                _ => {}
            }
        }
        return Hand::new_with_opened(hand, opened, drawn);
    }

    pub fn from_summarized(sum: &TileSummarize) -> Hand {
        let mut result: Vec<Tile> = Vec::new();

        for i in Tile::M1 as usize..Tile::LEN {
            for _ in 0..sum[i] {
                result.push(Tile::new(i as TileType));
            }
        }
        return Hand::new(result, None);
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
    fn from_with_no_opened_test() {
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
        assert_eq!(test.opened[0].category, OpenType::Chi);
        assert_eq!(
            test.opened[0].tiles,
            [
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9)
            ]
        );
        assert_eq!(test.opened[0].from, OpenFrom::Unknown);
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
    }

    #[test]
    fn from_with_pon_test() {
        let test_str = "123m456p789s5z 111z 5z";
        let test = Hand::from(test_str);
        assert_eq!(test.tiles[0], Tile::new(Tile::M1));
        assert_eq!(test.opened[0].category, OpenType::Pon);
        assert_eq!(
            test.opened[0].tiles,
            [
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1)
            ]
        );
        assert_eq!(test.opened[0].from, OpenFrom::Unknown);
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
    }

    #[test]
    fn from_with_kan_test() {
        let test_str = "123m456p789s5z 1111z 5z";
        let test = Hand::from(test_str);
        assert_eq!(test.tiles[0], Tile::new(Tile::M1));
        assert_eq!(test.opened[0].category, OpenType::Kan);
        assert_eq!(
            test.opened[0].tiles,
            [
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1)
            ]
        );
        assert_eq!(test.opened[0].from, OpenFrom::Unknown);
        assert_eq!(test.drawn, Some(Tile::new(Tile::Z5)));
        assert_eq!(test.to_short_string(), test_str);
    }
}
