//! 描画レイアウトのユニットテスト

use super::{
    PLAYER_ROTATIONS, calc_meld_width, other_meld_x_positions, rotation_index,
    seat_at_relative_position, self_meld_x_positions,
};
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType};

/// 並び順検証用の最小のポン副露を作る。
fn pon(tile_type: TileType) -> Meld {
    let tile = Tile::new(tile_type);
    Meld {
        tiles: vec![tile, tile, tile],
        category: MeldType::Pon,
        from: MeldFrom::Previous,
        called_tile: Some(tile),
    }
}

// 副露の並び順は「最初に鳴いた牌ほどプレイヤーから見て右」が正。
// 自分の手牌では右端 (x が大きい側) から左へ詰める。
#[test]
fn self_melds_place_first_called_on_the_right() {
    let (tw, th, gap, right_edge) = (40.0, 56.0, 12.0, 1220.0);
    let melds = vec![pon(Tile::M1), pon(Tile::P2), pon(Tile::S3)];
    let xs = self_meld_x_positions(&melds, tw, th, gap, right_edge);

    assert_eq!(xs.len(), 3);
    // 最初に鳴いた副露 (index 0) が最も右、後の副露ほど左。
    assert!(
        xs[0] > xs[1],
        "first-called meld must be right of the second"
    );
    assert!(
        xs[1] > xs[2],
        "second-called meld must be right of the third"
    );
    // 先頭の副露の右端は描画領域の右端を超えない。
    assert!(xs[0] + calc_meld_width(&melds[0], tw, th) <= right_edge);
}

// 他家の手牌でも、そのプレイヤー視点で最初に鳴いた牌が右 (x が大きい側) に来る。
#[test]
fn other_melds_place_first_called_on_the_right() {
    let (tw, th, gap, start_x) = (28.0, 40.0, 6.0, 100.0);
    let melds = vec![pon(Tile::M1), pon(Tile::P2), pon(Tile::S3)];
    let xs = other_meld_x_positions(&melds, tw, th, gap, start_x);

    assert_eq!(xs.len(), 3);
    assert!(
        xs[0] > xs[1],
        "first-called meld must be right of the second"
    );
    assert!(
        xs[1] > xs[2],
        "second-called meld must be right of the third"
    );
    // 描画領域は手牌の右隣 (start_x) から始まり、左へはみ出さない。
    assert!(xs[2] >= start_x);
}

#[test]
fn player_rotations_place_turn_order_counterclockwise() {
    assert_eq!(PLAYER_ROTATIONS, [0.0, -90.0, 180.0, 90.0]);
}

#[test]
fn seat0_relative_positions_match_seat_indices() {
    // 自分が座席0なら相対位置と座席インデックスは一致する（ローカル対局）。
    for rel in 0..4 {
        assert_eq!(seat_at_relative_position(0, rel, 4), rel);
    }
}

#[test]
fn nonzero_seat_maps_relative_positions_to_correct_seats() {
    // オンライン非ホスト: 自分の座席が0以外でも、相対位置→絶対座席へ正しく回る。
    // 自分(0)=座席2, 下家(1)=座席3, 対面(2)=座席0, 上家(3)=座席1。
    assert_eq!(seat_at_relative_position(2, 0, 4), 2);
    assert_eq!(seat_at_relative_position(2, 1, 4), 3);
    assert_eq!(seat_at_relative_position(2, 2, 4), 0);
    assert_eq!(seat_at_relative_position(2, 3, 4), 1);
}

#[test]
fn sanma_seat_mapping_wraps_at_three() {
    // 三麻: 座席は0〜2で循環する。
    assert_eq!(seat_at_relative_position(0, 0, 3), 0);
    assert_eq!(seat_at_relative_position(0, 1, 3), 1);
    assert_eq!(seat_at_relative_position(0, 2, 3), 2);
    assert_eq!(seat_at_relative_position(1, 2, 3), 0);
    assert_eq!(seat_at_relative_position(2, 1, 3), 0);
}

#[test]
fn sanma_rotation_maps_kamicha_to_left() {
    // 三麻: 相対2（上家）は左（90°）の描画パスを使う。
    assert_eq!(rotation_index(0, 3), 0);
    assert_eq!(rotation_index(1, 3), 1);
    assert_eq!(rotation_index(2, 3), 3);
    assert_eq!(PLAYER_ROTATIONS[rotation_index(2, 3)], 90.0);
    // 四麻は恒等写像。
    for rel in 0..4 {
        assert_eq!(rotation_index(rel, 4), rel);
    }
}
