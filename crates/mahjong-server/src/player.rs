//! プレイヤーの状態管理
//!
//! 各プレイヤーの手牌、捨て牌、点数、リーチ状態などを管理する。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::opened::OpenFrom;
use mahjong_core::tile::{Tile, Wind};
use serde::{Deserialize, Serialize};

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
        }
    }

    /// ツモ牌をセットする
    pub fn draw(&mut self, tile: Tile) {
        self.hand.set_drawn(Some(tile));
    }

    /// 手牌から指定牌を捨てる
    /// ツモ牌と同じならツモ切り、そうでなければ手出し
    pub fn discard(&mut self, tile_index: Option<usize>) -> Tile {
        let drawn = self.hand.drawn();

        match tile_index {
            // 手牌からの手出し
            Some(idx) => {
                let tiles = self.hand.tiles_mut();
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
        self.hand.opened().iter().all(|o| {
            // 暗カンは門前扱い
            o.from == OpenFrom::Myself
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
        let discarded = player.discard(Some(0));
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
}
