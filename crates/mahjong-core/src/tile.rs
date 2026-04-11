use serde::{Deserialize, Serialize};
use std::fmt;

/// 牌の種類を示す型
pub type TileType = u32;

pub type TileSummarize = [u32; Tile::LEN];

/// 牌
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Tile {
    index: TileType,
    red_dora: bool,
}

impl Tile {
    /// 一萬
    pub const M1: TileType = 0;
    /// 二萬
    pub const M2: TileType = 1;
    /// 三萬
    pub const M3: TileType = 2;
    /// 四萬
    pub const M4: TileType = 3;
    /// 五萬
    pub const M5: TileType = 4;
    /// 六萬
    pub const M6: TileType = 5;
    /// 七萬
    pub const M7: TileType = 6;
    /// 八萬
    pub const M8: TileType = 7;
    /// 九萬
    pub const M9: TileType = 8;
    /// 一筒
    pub const P1: TileType = 9;
    /// 二筒
    pub const P2: TileType = 10;
    /// 三筒
    pub const P3: TileType = 11;
    /// 四筒
    pub const P4: TileType = 12;
    /// 五筒
    pub const P5: TileType = 13;
    /// 六筒
    pub const P6: TileType = 14;
    /// 七筒
    pub const P7: TileType = 15;
    /// 八筒
    pub const P8: TileType = 16;
    /// 九筒
    pub const P9: TileType = 17;
    /// 一索
    pub const S1: TileType = 18;
    /// 二索
    pub const S2: TileType = 19;
    /// 三索
    pub const S3: TileType = 20;
    /// 四索
    pub const S4: TileType = 21;
    /// 五索
    pub const S5: TileType = 22;
    /// 六索
    pub const S6: TileType = 23;
    /// 七索
    pub const S7: TileType = 24;
    /// 八索
    pub const S8: TileType = 25;
    /// 九索
    pub const S9: TileType = 26;
    /// 東
    pub const Z1: TileType = 27;
    /// 南
    pub const Z2: TileType = 28;
    /// 西
    pub const Z3: TileType = 29;
    /// 北
    pub const Z4: TileType = 30;
    /// 白
    pub const Z5: TileType = 31;
    /// 發
    pub const Z6: TileType = 32;
    /// 中
    pub const Z7: TileType = 33;
    /// 牌の種類の数（インデックスは常にこの数よりも少ない整数値）
    pub const LEN: usize = 34;

    /// Unicode表記
    const CHARS: [char; Tile::LEN] = [
        '🀇', '🀈', '🀉', '🀊', '🀋', '🀌', '🀍', '🀎', '🀏', '🀙', '🀚', '🀛', '🀜', '🀝', '🀞', '🀟', '🀠', '🀡',
        '🀐', '🀑', '🀒', '🀓', '🀔', '🀕', '🀖', '🀗', '🀘', '🀀', '🀁', '🀂', '🀃', '🀆', '🀅', '🀄',
    ];
    /// Ascii表記
    const ASCII: [&'static str; Tile::LEN] = [
        "1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m", "1p", "2p", "3p", "4p", "5p", "6p",
        "7p", "8p", "9p", "1s", "2s", "3s", "4s", "5s", "6s", "7s", "8s", "9s", "1z", "2z", "3z",
        "4z", "5z", "6z", "7z",
    ];

    pub fn new(tile_type: TileType) -> Tile {
        Tile {
            index: tile_type,
            red_dora: false,
        }
    }

    /// 赤ドラの牌を作成する
    pub fn new_red(tile_type: TileType) -> Tile {
        Tile {
            index: tile_type,
            red_dora: true,
        }
    }

    pub fn get(&self) -> TileType {
        self.index
    }

    /// 赤ドラか否かを返す
    pub fn is_red_dora(&self) -> bool {
        self.red_dora
    }

    /// 数牌か否かを返す
    pub fn is_suited(&self) -> bool {
        self.is_character() || self.is_circle() || self.is_bamboo()
    }

    /// 萬子か否かを返す
    pub fn is_character(&self) -> bool {
        matches!(self.index, Tile::M1..=Tile::M9)
    }
    /// 筒子か否かを返す
    pub fn is_circle(&self) -> bool {
        matches!(self.index, Tile::P1..=Tile::P9)
    }
    /// 索子か否かを返す
    pub fn is_bamboo(&self) -> bool {
        matches!(self.index, Tile::S1..=Tile::S9)
    }
    /// 風牌か否かを返す
    pub fn is_wind(&self) -> bool {
        matches!(self.index, Tile::Z1..=Tile::Z4)
    }
    /// 三元牌か否かを返す
    pub fn is_dragon(&self) -> bool {
        matches!(self.index, Tile::Z5..=Tile::Z7)
    }
    /// 字牌か否かを返す
    pub fn is_honor(&self) -> bool {
        self.is_wind() || self.is_dragon()
    }

    /// 老頭牌か否かを返す
    pub fn is_1_or_9(&self) -> bool {
        matches!(
            self.index,
            Tile::M1 | Tile::M9 | Tile::P1 | Tile::P9 | Tile::S1 | Tile::S9
        )
    }
    /// 么九牌（老頭牌＋字牌）か否かを返す
    pub fn is_1_9_honor(&self) -> bool {
        self.is_1_or_9() || self.is_honor()
    }

    /// 対子（同じ2枚）か否かを返す
    pub fn is_same_to(&self, tile: Tile) -> bool {
        self.get() == tile.get()
    }
    /// 搭子（連続した2枚）か否かを返す
    pub fn is_sequential_to(&self, tile: Tile) -> bool {
        // 字牌ならば連続はありえない
        if self.is_honor() {
            return false;
        }
        // 一萬・一筒・一索の時に1つ前（九萬・九筒）が来ても連続とはみなさない
        if matches!(self.index, Tile::M1 | Tile::P1 | Tile::S1) && self.get() == tile.get() + 1 {
            return false;
        }
        // 九萬・九筒・九索の時に1つ後（一筒・一索・東）が来ても連続とはみなさない
        if matches!(self.index, Tile::M9 | Tile::P9 | Tile::S9) && self.get() == tile.get() - 1 {
            return false;
        } else if self.get() == tile.get() - 1 || self.get() == tile.get() + 1 {
            return true;
        }
        false
    }

    pub fn to_char(&self) -> char {
        Tile::CHARS[self.index as usize]
    }

    pub fn from(tile_name: &str) -> Option<Tile> {
        let t = match tile_name {
            "1m" | "🀇" => Tile::M1,
            "2m" | "🀈" => Tile::M2,
            "3m" | "🀉" => Tile::M3,
            "4m" | "🀊" => Tile::M4,
            "5m" | "🀋" => Tile::M5,
            "6m" | "🀌" => Tile::M6,
            "7m" | "🀍" => Tile::M7,
            "8m" | "🀎" => Tile::M8,
            "9m" | "🀏" => Tile::M9,
            "1p" | "🀙" => Tile::P1,
            "2p" | "🀚" => Tile::P2,
            "3p" | "🀛" => Tile::P3,
            "4p" | "🀜" => Tile::P4,
            "5p" | "🀝" => Tile::P5,
            "6p" | "🀞" => Tile::P6,
            "7p" | "🀟" => Tile::P7,
            "8p" | "🀠" => Tile::P8,
            "9p" | "🀡" => Tile::P9,
            "1s" | "🀐" => Tile::S1,
            "2s" | "🀑" => Tile::S2,
            "3s" | "🀒" => Tile::S3,
            "4s" | "🀓" => Tile::S4,
            "5s" | "🀔" => Tile::S5,
            "6s" | "🀕" => Tile::S6,
            "7s" | "🀖" => Tile::S7,
            "8s" | "🀗" => Tile::S8,
            "9s" | "🀘" => Tile::S9,
            "1z" | "🀀" => Tile::Z1,
            "2z" | "🀁" => Tile::Z2,
            "3z" | "🀂" => Tile::Z3,
            "4z" | "🀃" => Tile::Z4,
            "5z" | "🀆" => Tile::Z5,
            "6z" | "🀅" => Tile::Z6,
            "7z" | "🀄" => Tile::Z7,
            _ => {
                return None;
            }
        };
        Some(Tile::new(t))
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Tile::ASCII[self.index as usize])
    }
}

/// 数牌のスート内での数字（1〜9）を返す
///
/// 例: `Tile::M7`、`Tile::P7`、`Tile::S7` はいずれも `Some(7)` を返す。
/// 字牌の場合は `None` を返す。
pub fn suit_rank(tile: TileType) -> Option<u32> {
    match tile {
        Tile::M1 | Tile::P1 | Tile::S1 => Some(1),
        Tile::M2 | Tile::P2 | Tile::S2 => Some(2),
        Tile::M3 | Tile::P3 | Tile::S3 => Some(3),
        Tile::M4 | Tile::P4 | Tile::S4 => Some(4),
        Tile::M5 | Tile::P5 | Tile::S5 => Some(5),
        Tile::M6 | Tile::P6 | Tile::S6 => Some(6),
        Tile::M7 | Tile::P7 | Tile::S7 => Some(7),
        Tile::M8 | Tile::P8 | Tile::S8 => Some(8),
        Tile::M9 | Tile::P9 | Tile::S9 => Some(9),
        _ => None,
    }
}

/// ドラ表示牌から実際のドラを返す
pub fn dora_indicator_to_dora(indicator: TileType) -> TileType {
    match indicator {
        // 萬子: 9m→1m にループ
        Tile::M9 => Tile::M1,
        Tile::M1..=Tile::M8 => indicator + 1,
        // 筒子: 9p→1p にループ
        Tile::P9 => Tile::P1,
        Tile::P1..=Tile::P8 => indicator + 1,
        // 索子: 9s→1s にループ
        Tile::S9 => Tile::S1,
        Tile::S1..=Tile::S8 => indicator + 1,
        // 風牌: 北→東 にループ
        Tile::Z4 => Tile::Z1,
        Tile::Z1..=Tile::Z3 => indicator + 1,
        // 三元牌: 中→白 にループ
        Tile::Z7 => Tile::Z5,
        Tile::Z5..=Tile::Z6 => indicator + 1,
        _ => indicator,
    }
}

/// 自風／場風
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Wind {
    /// 東家（`Tile::Z1`）
    East = Tile::Z1 as isize,
    /// 南家（`Tile::Z2`）
    South = Tile::Z2 as isize,
    /// 西家（`Tile::Z3`）
    West = Tile::Z3 as isize,
    /// 北家（`Tile::Z4`）
    North = Tile::Z4 as isize,
}

impl Wind {
    pub fn is_tile_type(tile_type: TileType) -> Option<Wind> {
        match tile_type {
            Tile::Z1 => Some(Wind::East),
            Tile::Z2 => Some(Wind::South),
            Tile::Z3 => Some(Wind::West),
            Tile::Z4 => Some(Wind::North),
            _ => None,
        }
    }
    pub fn is_tile(tile: &Tile) -> Option<Wind> {
        Wind::is_tile_type(tile.get())
    }

    /// 次の風を返す（東→南→西→北→東）
    pub fn next(&self) -> Wind {
        match self {
            Wind::East => Wind::South,
            Wind::South => Wind::West,
            Wind::West => Wind::North,
            Wind::North => Wind::East,
        }
    }

    /// 風をインデックス（0-3）に変換する
    pub fn to_index(&self) -> usize {
        match self {
            Wind::East => 0,
            Wind::South => 1,
            Wind::West => 2,
            Wind::North => 3,
        }
    }

    /// インデックス（0-3）から風を生成する
    pub fn from_index(index: usize) -> Wind {
        match index % 4 {
            0 => Wind::East,
            1 => Wind::South,
            2 => Wind::West,
            3 => Wind::North,
            _ => unreachable!(),
        }
    }
}

/// 三元牌
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Dragon {
    /// 白（`Tile::Z5`）
    White = Tile::Z5 as isize,
    /// 發（`Tile::Z6`）
    Green = Tile::Z6 as isize,
    /// 中（`Tile::Z7`）
    Red = Tile::Z7 as isize,
}

impl Dragon {
    pub fn is_tile_type(tile_type: TileType) -> Option<Dragon> {
        match tile_type {
            Tile::Z5 => Some(Dragon::White),
            Tile::Z6 => Some(Dragon::Green),
            Tile::Z7 => Some(Dragon::Red),
            _ => None,
        }
    }
    pub fn is_tile(tile: &Tile) -> Option<Dragon> {
        Dragon::is_tile_type(tile.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 萬子の属性テスト
    #[test]
    fn suit_char_test() {
        for i in Tile::M1..=Tile::M9 {
            let t = Tile::new(i);
            assert!(t.is_character());
            assert!(!t.is_bamboo());
            assert!(!t.is_circle());
            assert!(!t.is_honor());
            assert_eq!(t.is_1_or_9(), i == Tile::M1 || i == Tile::M9);
        }
    }

    /// 筒子の属性テスト
    #[test]
    fn suit_circle_test() {
        for i in Tile::P1..=Tile::P9 {
            let t = Tile::new(i);
            assert!(!t.is_character());
            assert!(!t.is_bamboo());
            assert!(t.is_circle());
            assert!(!t.is_honor());
            assert_eq!(t.is_1_or_9(), i == Tile::P1 || i == Tile::P9);
        }
    }
    /// 索子の属性テスト
    #[test]
    fn suit_bamboo_test() {
        for i in Tile::S1..=Tile::S9 {
            let t = Tile::new(i);
            assert!(!t.is_character());
            assert!(t.is_bamboo());
            assert!(!t.is_circle());
            assert!(!t.is_honor());
            assert_eq!(t.is_1_or_9(), i == Tile::S1 || i == Tile::S9);
        }
    }
    /// 風牌の属性テスト
    #[test]
    fn suit_wind_test() {
        for i in Tile::Z1..=Tile::Z4 {
            let t = Tile::new(i);
            assert!(!t.is_character());
            assert!(!t.is_bamboo());
            assert!(!t.is_circle());
            assert!(t.is_wind());
            assert!(!t.is_dragon());
            assert!(t.is_honor());
        }
    }
    /// 三元牌の属性テスト
    #[test]
    fn suit_dragon_test() {
        for i in Tile::Z5..=Tile::Z7 {
            let t = Tile::new(i);
            assert!(!t.is_character());
            assert!(!t.is_bamboo());
            assert!(!t.is_circle());
            assert!(!t.is_wind());
            assert!(t.is_dragon());
            assert!(t.is_honor());
        }
    }
    /// 字牌の属性テスト
    #[test]
    fn suit_honor_test() {
        for i in Tile::Z1..=Tile::Z7 {
            let t = Tile::new(i);
            assert!(!t.is_character());
            assert!(!t.is_bamboo());
            assert!(!t.is_circle());
            assert!(t.is_honor());
        }
    }

    /// 対子テスト
    #[test]
    fn sameness_test() {
        // 1m→1mは対子
        assert!(Tile::new(Tile::M1).is_same_to(Tile::new(Tile::M1)));
        // 1m→1pは対子ではない
        assert!(!Tile::new(Tile::M1).is_same_to(Tile::new(Tile::P1)));
        // 1z→1zは対子
        assert!(Tile::new(Tile::Z1).is_same_to(Tile::new(Tile::Z1)));
    }

    /// 搭子テスト
    #[test]
    fn sequential_test() {
        // 1m→2mは搭子
        assert!(Tile::new(Tile::M1).is_sequential_to(Tile::new(Tile::M2)));
        // 3p→3pは搭子ではない
        assert!(!Tile::new(Tile::P3).is_sequential_to(Tile::new(Tile::P3)));
        // 7s→8sは搭子
        assert!(Tile::new(Tile::S7).is_sequential_to(Tile::new(Tile::S8)));
        // 1m→1pは搭子ではない
        assert!(!Tile::new(Tile::M1).is_sequential_to(Tile::new(Tile::P1)));
        // 9m→8mは搭子
        assert!(Tile::new(Tile::M9).is_sequential_to(Tile::new(Tile::M8)));
        // 9m→1pは搭子ではない
        assert!(!Tile::new(Tile::M9).is_sequential_to(Tile::new(Tile::P1)));
        // 1s→9pは搭子ではない
        assert!(!Tile::new(Tile::S1).is_sequential_to(Tile::new(Tile::P9)));
        // 9s→1zは搭子ではない
        assert!(!Tile::new(Tile::S9).is_sequential_to(Tile::new(Tile::Z1)));
        // 1z→2zは搭子ではない
        assert!(!Tile::new(Tile::Z1).is_sequential_to(Tile::new(Tile::Z2)));
    }

    /// ドラ表示牌テスト
    #[test]
    fn dora_indicator_test() {
        assert_eq!(dora_indicator_to_dora(Tile::M1), Tile::M2);
        assert_eq!(dora_indicator_to_dora(Tile::M9), Tile::M1);
        assert_eq!(dora_indicator_to_dora(Tile::P5), Tile::P6);
        assert_eq!(dora_indicator_to_dora(Tile::P9), Tile::P1);
        assert_eq!(dora_indicator_to_dora(Tile::S9), Tile::S1);
        assert_eq!(dora_indicator_to_dora(Tile::Z1), Tile::Z2);
        assert_eq!(dora_indicator_to_dora(Tile::Z4), Tile::Z1);
        assert_eq!(dora_indicator_to_dora(Tile::Z5), Tile::Z6);
        assert_eq!(dora_indicator_to_dora(Tile::Z7), Tile::Z5);
    }

    /// 赤ドラテスト
    #[test]
    fn red_dora_test() {
        let red5m = Tile::new_red(Tile::M5);
        assert!(red5m.is_red_dora());
        assert_eq!(red5m.get(), Tile::M5);

        let normal5m = Tile::new(Tile::M5);
        assert!(!normal5m.is_red_dora());
    }

    /// Windテスト
    #[test]
    fn wind_test() {
        assert_eq!(Wind::East.next(), Wind::South);
        assert_eq!(Wind::South.next(), Wind::West);
        assert_eq!(Wind::West.next(), Wind::North);
        assert_eq!(Wind::North.next(), Wind::East);
        assert_eq!(Wind::East.to_index(), 0);
        assert_eq!(Wind::from_index(2), Wind::West);
        assert_eq!(Wind::from_index(4), Wind::East);
    }

    #[test]
    fn suit_rank_manzu() {
        assert_eq!(suit_rank(Tile::M1), Some(1));
        assert_eq!(suit_rank(Tile::M2), Some(2));
        assert_eq!(suit_rank(Tile::M3), Some(3));
        assert_eq!(suit_rank(Tile::M4), Some(4));
        assert_eq!(suit_rank(Tile::M5), Some(5));
        assert_eq!(suit_rank(Tile::M6), Some(6));
        assert_eq!(suit_rank(Tile::M7), Some(7));
        assert_eq!(suit_rank(Tile::M8), Some(8));
        assert_eq!(suit_rank(Tile::M9), Some(9));
    }

    #[test]
    fn suit_rank_pinzu() {
        assert_eq!(suit_rank(Tile::P1), Some(1));
        assert_eq!(suit_rank(Tile::P2), Some(2));
        assert_eq!(suit_rank(Tile::P3), Some(3));
        assert_eq!(suit_rank(Tile::P4), Some(4));
        assert_eq!(suit_rank(Tile::P5), Some(5));
        assert_eq!(suit_rank(Tile::P6), Some(6));
        assert_eq!(suit_rank(Tile::P7), Some(7));
        assert_eq!(suit_rank(Tile::P8), Some(8));
        assert_eq!(suit_rank(Tile::P9), Some(9));
    }

    #[test]
    fn suit_rank_souzu() {
        assert_eq!(suit_rank(Tile::S1), Some(1));
        assert_eq!(suit_rank(Tile::S2), Some(2));
        assert_eq!(suit_rank(Tile::S3), Some(3));
        assert_eq!(suit_rank(Tile::S4), Some(4));
        assert_eq!(suit_rank(Tile::S5), Some(5));
        assert_eq!(suit_rank(Tile::S6), Some(6));
        assert_eq!(suit_rank(Tile::S7), Some(7));
        assert_eq!(suit_rank(Tile::S8), Some(8));
        assert_eq!(suit_rank(Tile::S9), Some(9));
    }

    #[test]
    fn suit_rank_honor_returns_none() {
        // 字牌（風牌・三元牌）はすべて None
        for tile in Tile::Z1..=Tile::Z7 {
            assert_eq!(suit_rank(tile), None, "tile {tile} should return None");
        }
    }
}
