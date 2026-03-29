//! プレイヤーの状態管理
//!
//! 各プレイヤーの手牌、捨て牌、点数、リーチ状態などを管理する。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType, Wind};
use serde::{Deserialize, Serialize};

use crate::scoring;

/// プレイヤーの状態
pub struct Player {
    /// 座席の風
    pub seat_wind: Wind,
    /// 手牌
    pub hand: Hand,
    /// 捨て牌（河）
    pub discards: Vec<Discard>,
    /// 持ち点
    pub score: i32,
    /// リーチしているか
    pub is_riichi: bool,
    /// ダブルリーチか
    pub is_double_riichi: bool,
    /// 一発が有効か
    pub is_ippatsu: bool,
    /// 第一ツモか（天和・地和判定用）
    pub is_first_turn: bool,
    /// 副露によって一巡目が中断されたか
    pub first_turn_interrupted: bool,
    /// リーチ後フリテン（リーチ後にロン見逃し → 局終了まで永続）
    pub is_riichi_furiten: bool,
    /// 同巡フリテン（ロン見逃し → 自分のツモ番で解除）
    pub is_temporary_furiten: bool,
}

/// 捨て牌1枚の情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discard {
    /// 捨てた牌
    pub tile: Tile,
    /// ツモ切りか
    pub is_tsumogiri: bool,
    /// リーチ宣言牌か
    pub is_riichi_declaration: bool,
    /// 他プレイヤーに鳴かれたか
    pub is_called: bool,
}

impl Player {
    /// 新しいプレイヤーを作成する
    pub fn new(seat_wind: Wind, tiles: Vec<Tile>, initial_score: i32) -> Self {
        let hand = Hand::new(tiles, None);
        Player {
            seat_wind,
            hand,
            discards: Vec::new(),
            score: initial_score,
            is_riichi: false,
            is_double_riichi: false,
            is_ippatsu: false,
            is_first_turn: true,
            first_turn_interrupted: false,
            is_riichi_furiten: false,
            is_temporary_furiten: false,
        }
    }

    /// ツモ牌をセットする
    pub fn draw(&mut self, tile: Tile) {
        self.hand.set_drawn(Some(tile));
    }

    /// 手牌から指定牌を捨てる
    /// tile が Some(牌) なら手牌からその牌を探して捨てる（手出し）
    /// tile が None ならツモ切り
    pub fn discard(&mut self, tile: Option<Tile>) -> Tile {
        let drawn = self.hand.drawn();

        match tile {
            // 手牌からの手出し: 牌の種類で検索して除去
            Some(target) => {
                let tiles = self.hand.tiles_mut();
                let idx = tiles
                    .iter()
                    .position(|t| *t == target)
                    .expect("指定された牌が手牌にありません");
                let discarded = tiles.remove(idx);

                // ツモ牌を手牌に加える
                if let Some(drawn_tile) = drawn {
                    tiles.push(drawn_tile);
                    tiles.sort();
                }
                self.hand.set_drawn(None);

                self.discards.push(Discard {
                    tile: discarded,
                    is_tsumogiri: false,
                    is_riichi_declaration: false,
                    is_called: false,
                });

                // 一発を無効にする（自分が打牌したので）
                self.is_ippatsu = false;
                self.is_first_turn = false;

                discarded
            }
            // ツモ切り（ツモった牌をそのまま捨てる）
            None => {
                let discarded = drawn.expect("ツモ牌がない状態でツモ切りはできません");
                self.hand.set_drawn(None);

                self.discards.push(Discard {
                    tile: discarded,
                    is_tsumogiri: true,
                    is_riichi_declaration: false,
                    is_called: false,
                });

                self.is_ippatsu = false;
                self.is_first_turn = false;

                discarded
            }
        }
    }

    /// ツモ切りを行う（他プレイヤーの自動打牌用）
    pub fn tsumogiri(&mut self) -> Tile {
        self.discard(None)
    }

    /// 親（東家）かどうか
    pub fn is_dealer(&self) -> bool {
        self.seat_wind == Wind::East
    }

    /// 門前（鳴いていない）かどうか
    pub fn is_menzen(&self) -> bool {
        self.hand.melds().iter().all(|o| {
            // 暗カンは門前扱い
            o.from == MeldFrom::Myself
        })
    }

    /// リーチ宣言を行う
    pub fn declare_riichi(&mut self, is_double: bool) {
        self.is_riichi = true;
        self.is_ippatsu = true;
        if is_double {
            self.is_double_riichi = true;
        }
        // リーチ棒代を引く
        self.score -= 1000;
    }

    // ===== 鳴き判定メソッド =====

    /// ポン可能か判定する
    pub fn can_pon(&self, tile: Tile) -> bool {
        let count = self.hand.tiles().iter().filter(|t| t.get() == tile.get()).count();
        count >= 2
    }

    /// チー可能な組み合わせを返す
    ///
    /// 各要素は [TileType; 2] で、手牌から使う2枚の牌の種類を表す。
    /// 字牌はチー不可。
    pub fn chi_options(&self, tile: Tile) -> Vec<[TileType; 2]> {
        if tile.is_honor() {
            return vec![];
        }

        let tt = tile.get();
        let tiles = self.hand.tiles();
        let mut options = vec![];

        // 同じスーツの範囲を計算
        let suit_start = (tt / 9) * 9;
        let suit_end = suit_start + 9;

        // パターン1: [tt-2, tt-1] + tt （例: 鳴く牌が3m, 手牌に1m2mがある）
        if tt >= suit_start + 2 {
            let a = tt - 2;
            let b = tt - 1;
            if tiles.iter().any(|t| t.get() == a)
                && tiles.iter().any(|t| t.get() == b)
            {
                options.push([a, b]);
            }
        }

        // パターン2: [tt-1, tt+1] + tt （例: 鳴く牌が5m, 手牌に4m6mがある）
        if tt >= suit_start + 1 && tt + 1 < suit_end {
            let a = tt - 1;
            let b = tt + 1;
            if tiles.iter().any(|t| t.get() == a)
                && tiles.iter().any(|t| t.get() == b)
            {
                options.push([a, b]);
            }
        }

        // パターン3: [tt+1, tt+2] + tt （例: 鳴く牌が1m, 手牌に2m3mがある）
        if tt + 2 < suit_end {
            let a = tt + 1;
            let b = tt + 2;
            if tiles.iter().any(|t| t.get() == a)
                && tiles.iter().any(|t| t.get() == b)
            {
                options.push([a, b]);
            }
        }

        options
    }

    /// 大明カン可能か判定する
    pub fn can_daiminkan(&self, tile: Tile) -> bool {
        let count = self.hand.tiles().iter().filter(|t| t.get() == tile.get()).count();
        count >= 3
    }

    /// 暗カン可能な牌種一覧を返す
    pub fn ankan_options(&self) -> Vec<TileType> {
        let mut counts = [0u8; Tile::LEN as usize];
        for tile in self.hand.tiles() {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.hand.drawn() {
            counts[drawn.get() as usize] += 1;
        }

        counts
            .iter()
            .enumerate()
            .filter_map(|(idx, &count)| (count == 4).then_some(idx as TileType))
            .collect()
    }

    /// 加カン可能な牌種一覧を返す
    pub fn kakan_options(&self) -> Vec<TileType> {
        let mut counts = [0u8; Tile::LEN as usize];
        for tile in self.hand.tiles() {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.hand.drawn() {
            counts[drawn.get() as usize] += 1;
        }

        self.hand
            .melds()
            .iter()
            .filter(|open| open.category == MeldType::Pon)
            .filter_map(|open| {
                let tile_type = open.tiles[0].get();
                (counts[tile_type as usize] >= 1).then_some(tile_type)
            })
            .collect()
    }

    /// フリテン状態か判定する
    ///
    /// 以下のいずれかに該当する場合、フリテン（ロン不可・ツモのみ可）:
    /// 1. 捨て牌フリテン: 自分の待ち牌のいずれかが自分の捨て牌に含まれている
    /// 2. リーチ後フリテン: リーチ後にロンを見逃した（局終了まで永続）
    /// 3. 同巡フリテン: ロンを見逃した（自分のツモ番で解除）
    pub fn is_furiten(&self) -> bool {
        // リーチ後フリテン・同巡フリテン（O(1)で早期リターン）
        if self.is_riichi_furiten || self.is_temporary_furiten {
            return true;
        }
        // 捨て牌フリテン
        let waiting = scoring::get_waiting_tiles(self);
        if waiting.is_empty() {
            return false;
        }
        for &wt in &waiting {
            if self.discards.iter().any(|d| d.tile.get() == wt) {
                return true;
            }
        }
        false
    }

    // ===== 鳴き実行メソッド =====

    /// ポンを実行する
    ///
    /// 手牌から同じ種類の牌2枚を取り除き、鳴いた牌と合わせて副露に追加する。
    pub fn do_pon(&mut self, called_tile: Tile, from: MeldFrom) {
        let tt = called_tile.get();
        let mut indices: Vec<usize> = Vec::new();
        for (i, t) in self.hand.tiles().iter().enumerate() {
            if t.get() == tt && indices.len() < 2 {
                indices.push(i);
            }
        }

        let t1 = self.hand.tiles()[indices[0]];
        let t2 = self.hand.tiles()[indices[1]];

        self.hand.remove_tiles_by_indices(&mut indices);

        self.hand.add_meld(Meld {
            tiles: vec![t1, t2, called_tile],
            category: MeldType::Pon,
            from,
            called_tile: Some(called_tile),
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// チーを実行する
    ///
    /// 手牌から指定種類の牌2枚を取り除き、鳴いた牌と合わせて副露に追加する。
    pub fn do_chi(&mut self, called_tile: Tile, hand_tile_types: [TileType; 2]) {
        let mut indices: Vec<usize> = Vec::new();
        for &tt in &hand_tile_types {
            for (i, t) in self.hand.tiles().iter().enumerate() {
                if t.get() == tt && !indices.contains(&i) {
                    indices.push(i);
                    break;
                }
            }
        }

        let t1 = self.hand.tiles()[indices[0]];
        let t2 = self.hand.tiles()[indices[1]];

        self.hand.remove_tiles_by_indices(&mut indices);

        // 順子の牌をソートして副露に追加
        let mut chi_tiles = [t1, t2, called_tile];
        chi_tiles.sort();

        self.hand.add_meld(Meld {
            tiles: chi_tiles.to_vec(),
            category: MeldType::Chi,
            from: MeldFrom::Previous, // チーは常に上家から
            called_tile: Some(called_tile),
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// 大明カンを実行する
    pub fn do_daiminkan(&mut self, called_tile: Tile, from: MeldFrom) {
        let tt = called_tile.get();
        let mut indices: Vec<usize> = Vec::new();
        for (i, t) in self.hand.tiles().iter().enumerate() {
            if t.get() == tt && indices.len() < 3 {
                indices.push(i);
            }
        }
        assert_eq!(indices.len(), 3, "大明カンに必要な3枚がありません");

        let t1 = self.hand.tiles()[indices[0]];
        let t2 = self.hand.tiles()[indices[1]];
        let t3 = self.hand.tiles()[indices[2]];

        self.hand.remove_tiles_by_indices(&mut indices);
        self.hand.add_meld(Meld {
            tiles: vec![t1, t2, t3],
            category: MeldType::Kan,
            from,
            called_tile: Some(called_tile),
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// 暗カンを実行する
    pub fn do_ankan(&mut self, tile_type: TileType) {
        let mut indices: Vec<usize> = Vec::new();
        for (i, t) in self.hand.tiles().iter().enumerate() {
            if t.get() == tile_type {
                indices.push(i);
            }
        }

        let drawn = self.hand.drawn();
        let drawn_matches = drawn.map(|t| t.get() == tile_type).unwrap_or(false);
        assert_eq!(
            indices.len() + usize::from(drawn_matches),
            4,
            "暗カンに必要な4枚が揃っていません"
        );

        let mut kan_tiles: Vec<Tile> = indices
            .iter()
            .map(|&idx| self.hand.tiles()[idx])
            .collect();
        if drawn_matches {
            kan_tiles.push(drawn.unwrap());
            self.hand.set_drawn(None);
        } else if let Some(d) = drawn {
            // ツモ牌がカン牌でない場合、手牌に戻す（嶺上ツモで上書きされないよう）
            self.hand.tiles_mut().push(d);
            self.hand.sort();
            self.hand.set_drawn(None);
        }

        self.hand.remove_tiles_by_indices(&mut indices);
        self.hand.add_meld(Meld {
            tiles: vec![kan_tiles[0], kan_tiles[1], kan_tiles[2]],
            category: MeldType::Kan,
            from: MeldFrom::Myself,
            called_tile: None,
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// 加カンを実行する
    pub fn do_kakan(&mut self, tile_type: TileType) {
        let drawn_matches = self.hand.drawn().map(|t| t.get() == tile_type).unwrap_or(false);
        if drawn_matches {
            self.hand.set_drawn(None);
        } else {
            let idx = self
                .hand
                .tiles()
                .iter()
                .position(|t| t.get() == tile_type)
                .expect("加カンに必要な牌が手牌にありません");
            self.hand.tiles_mut().remove(idx);

            if let Some(drawn_tile) = self.hand.drawn() {
                self.hand.tiles_mut().push(drawn_tile);
                self.hand.sort();
                self.hand.set_drawn(None);
            }
        }

        let open = self
            .hand
            .melds_mut()
            .iter_mut()
            .find(|open| open.category == MeldType::Pon && open.tiles[0].get() == tile_type)
            .expect("加カン対象のポンがありません");
        open.category = MeldType::Kakan;

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// 手牌に含まれる槓子の数を返す
    pub fn kan_count(&self) -> usize {
        self.hand
            .melds()
            .iter()
            .filter(|open| open.category.is_kan())
            .count()
    }

    /// 捨てたプレイヤーと自分の相対位置から MeldFrom を返す
    pub fn meld_from_relative(caller: usize, discarder: usize) -> MeldFrom {
        match (caller + 4 - discarder) % 4 {
            1 => MeldFrom::Previous,   // 上家（カミチャ）
            2 => MeldFrom::Opposite,   // 対面（トイメン）
            3 => MeldFrom::Following,  // 下家（シモチャ）
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::Tile;

    fn make_test_tiles() -> Vec<Tile> {
        // 1m2m3m 4p5p6p 7s8s9s 1z2z3z4z の13枚
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ]
    }

    #[test]
    fn test_player_new() {
        let player = Player::new(Wind::East, make_test_tiles(), 25000);
        assert_eq!(player.seat_wind, Wind::East);
        assert_eq!(player.score, 25000);
        assert_eq!(player.hand.tiles().len(), 13);
        assert!(player.discards.is_empty());
        assert!(!player.is_riichi);
        assert!(player.is_dealer());
    }

    #[test]
    fn test_draw_and_tsumogiri() {
        let mut player = Player::new(Wind::South, make_test_tiles(), 25000);
        let draw_tile = Tile::new(Tile::Z5);

        player.draw(draw_tile);
        assert_eq!(player.hand.drawn(), Some(draw_tile));

        let discarded = player.tsumogiri();
        assert_eq!(discarded, draw_tile);
        assert!(player.hand.drawn().is_none());
        assert_eq!(player.discards.len(), 1);
        assert!(player.discards[0].is_tsumogiri);
    }

    #[test]
    fn test_discard_from_hand() {
        let mut player = Player::new(Wind::West, make_test_tiles(), 25000);
        let draw_tile = Tile::new(Tile::Z5);

        player.draw(draw_tile);

        // 手牌の最初の牌（1m）を捨てる
        let discarded = player.discard(Some(Tile::new(Tile::M1)));
        assert_eq!(discarded.get(), Tile::M1);
        assert_eq!(player.discards.len(), 1);
        assert!(!player.discards[0].is_tsumogiri);

        // 手牌が13枚のままであること（ツモ牌が手牌に入った）
        assert_eq!(player.hand.tiles().len(), 13);
        assert!(player.hand.drawn().is_none());
    }

    #[test]
    fn test_riichi_declaration() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);

        player.declare_riichi(false);
        assert!(player.is_riichi);
        assert!(!player.is_double_riichi);
        assert!(player.is_ippatsu);
        assert_eq!(player.score, 24000); // 1000点引かれる

        player.declare_riichi(true);
        assert!(player.is_double_riichi);
    }

    #[test]
    fn test_is_menzen() {
        let player = Player::new(Wind::North, make_test_tiles(), 25000);
        assert!(player.is_menzen());
    }

    #[test]
    fn test_not_dealer() {
        let player = Player::new(Wind::South, make_test_tiles(), 25000);
        assert!(!player.is_dealer());
    }

    #[test]
    fn test_can_pon() {
        // 手牌: 1m1m3m 4p5p6p 7s8s9s 1z2z3z4z
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let player = Player::new(Wind::East, tiles, 25000);
        assert!(player.can_pon(Tile::new(Tile::M1))); // 1mが2枚ある
        assert!(!player.can_pon(Tile::new(Tile::M3))); // 3mは1枚しかない
    }

    #[test]
    fn test_chi_options() {
        // 手牌: 2m3m5m 4p5p6p 7s8s9s 1z2z3z4z
        let tiles = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M5),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let player = Player::new(Wind::East, tiles, 25000);

        // 4mでチー: [2m,3m] or [3m,5m]
        let options = player.chi_options(Tile::new(Tile::M4));
        assert_eq!(options.len(), 2);
        assert!(options.contains(&[Tile::M2, Tile::M3]));
        assert!(options.contains(&[Tile::M3, Tile::M5]));

        // 字牌はチー不可
        let options = player.chi_options(Tile::new(Tile::Z1));
        assert!(options.is_empty());

        // 1mでチー: [2m,3m]
        let options = player.chi_options(Tile::new(Tile::M1));
        assert_eq!(options.len(), 1);
        assert_eq!(options[0], [Tile::M2, Tile::M3]);
    }

    #[test]
    fn test_do_pon() {
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let mut player = Player::new(Wind::South, tiles, 25000);
        let called = Tile::new(Tile::M1);

        player.do_pon(called, MeldFrom::Previous);

        // 手牌が11枚になること（13 - 2 = 11）
        assert_eq!(player.hand.tiles().len(), 11);
        // 副露が1つ
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Pon);
        // 門前でなくなる
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_do_chi() {
        let tiles = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M5),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let mut player = Player::new(Wind::South, tiles, 25000);
        let called = Tile::new(Tile::M4);

        player.do_chi(called, [Tile::M3, Tile::M5]);

        // 手牌が11枚になること
        assert_eq!(player.hand.tiles().len(), 11);
        // 副露が1つ
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Chi);
        // 門前でなくなる
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_can_daiminkan() {
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
        ];
        let player = Player::new(Wind::East, tiles, 25000);
        assert!(player.can_daiminkan(Tile::new(Tile::M1)));
        assert!(!player.can_daiminkan(Tile::new(Tile::M3)));
    }

    #[test]
    fn test_ankan_options() {
        let hand = Hand::from("111m234p567s789m1z 1m");
        let mut player = Player::new(Wind::East, hand.tiles().to_vec(), 25000);
        player.draw(hand.drawn().unwrap());

        assert_eq!(player.ankan_options(), vec![Tile::M1]);
    }

    #[test]
    fn test_do_daiminkan() {
        let hand = Hand::from("111m234p567s789m1z");
        let mut player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);

        player.do_daiminkan(Tile::new(Tile::M1), MeldFrom::Previous);

        assert_eq!(player.hand.tiles().len(), 10);
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kan);
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_do_ankan() {
        let hand = Hand::from("111m234p567s789m1z 1m");
        let mut player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);
        player.draw(hand.drawn().unwrap());

        player.do_ankan(Tile::M1);

        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.drawn().is_none());
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kan);
        assert!(player.is_menzen());
    }

    #[test]
    fn test_do_kakan() {
        let mut player = Player::new(Wind::South, vec![], 25000);
        player.hand = Hand::from("234p567s789m1z 111m 1m");

        player.do_kakan(Tile::M1);

        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.drawn().is_none());
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kakan);
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_do_kakan_preserves_unrelated_drawn_tile() {
        let mut player = Player::new(Wind::South, vec![], 25000);
        player.hand = Hand::from("127m234p567s1z 111m 9s");

        player.do_kakan(Tile::M1);

        assert!(player.hand.drawn().is_none());
        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.tiles().contains(&Tile::new(Tile::S9)));
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kakan);
    }

    #[test]
    fn test_meld_from_relative() {
        // プレイヤー1から見たプレイヤー0 → 上家（Previous）
        assert_eq!(Player::meld_from_relative(1, 0), MeldFrom::Previous);
        // プレイヤー2から見たプレイヤー0 → 対面（Opposite）
        assert_eq!(Player::meld_from_relative(2, 0), MeldFrom::Opposite);
        // プレイヤー3から見たプレイヤー0 → 下家（Following）
        assert_eq!(Player::meld_from_relative(3, 0), MeldFrom::Following);
    }

    #[test]
    fn test_is_furiten_riichi_furiten() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.is_riichi_furiten = true;
        assert!(player.is_furiten());
    }

    #[test]
    fn test_is_furiten_temporary_furiten() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.is_temporary_furiten = true;
        assert!(player.is_furiten());
    }

    #[test]
    fn test_is_furiten_none() {
        let player = Player::new(Wind::East, make_test_tiles(), 25000);
        assert!(!player.is_furiten());
    }
}
