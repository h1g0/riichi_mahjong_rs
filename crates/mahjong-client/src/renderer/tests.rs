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

// ── 牌インデックスラベル（英語UI） ──────────────────────────────────────

use super::labels::{
    LABEL_BLACK, LABEL_BLUE, LABEL_GREEN, LABEL_RED, bake_label, tile_index_label,
};
use macroquad::prelude::{Color, Image, WHITE};

#[test]
fn number_tiles_get_suit_colored_digits() {
    // 数牌: 萬子=赤・筒子=青・索子=緑の数字。
    for n in 0..9u32 {
        let digit = (n + 1).to_string();
        assert_eq!(tile_index_label(Tile::M1 + n), (digit.as_str(), LABEL_RED));
        assert_eq!(tile_index_label(Tile::P1 + n), (digit.as_str(), LABEL_BLUE));
        assert_eq!(
            tile_index_label(Tile::S1 + n),
            (digit.as_str(), LABEL_GREEN)
        );
    }
}

#[test]
fn honor_tiles_get_letter_labels() {
    // 風牌は E/S/W/N（黒）、三元牌は輸出用麻雀牌の伝統的表記 P/F/C。
    assert_eq!(tile_index_label(Tile::Z1), ("E", LABEL_BLACK));
    assert_eq!(tile_index_label(Tile::Z2), ("S", LABEL_BLACK));
    assert_eq!(tile_index_label(Tile::Z3), ("W", LABEL_BLACK));
    assert_eq!(tile_index_label(Tile::Z4), ("N", LABEL_BLACK));
    assert_eq!(tile_index_label(Tile::Z5), ("P", LABEL_BLACK));
    assert_eq!(tile_index_label(Tile::Z6), ("F", LABEL_GREEN));
    assert_eq!(tile_index_label(Tile::Z7), ("C", LABEL_RED));
}

#[test]
fn every_tile_type_has_a_label() {
    for tile_type in 0..Tile::LEN as TileType {
        let (label, _) = tile_index_label(tile_type);
        assert!(!label.is_empty(), "tile type {tile_type} has no label");
    }
}

/// テスト用にアプリ同梱フォントを読み込む。
fn test_font() -> fontdue::Font {
    let bytes: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansJP-Regular.ttf");
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("font")
}

#[test]
fn bake_label_paints_top_right_and_leaves_rest_untouched() {
    // 単色の不透明画像へ焼き込み、右上にだけ変化があることを確かめる。
    let mut img = Image::gen_image_color(111, 150, Color::new(0.5, 0.5, 0.5, 1.0));
    // 格納時に u8 へ量子化されるため、比較基準は画像から読み戻した値にする
    let base = img.get_pixel(0, 0);
    bake_label(&mut img, "5", LABEL_RED, &test_font());

    // 右上のバッジ領域内に、下地の白とラベルの赤の両方が現れる。
    let mut has_white = false;
    let mut has_red = false;
    for y in 0..55u32 {
        for x in 55..111u32 {
            let p = img.get_pixel(x, y);
            has_white |= p.r > 0.85 && p.g > 0.85 && p.b > 0.85;
            has_red |= p.r > 0.6 && p.g < 0.4 && p.b < 0.4;
        }
    }
    assert!(has_white, "badge background not painted");
    assert!(has_red, "label glyph not painted");

    // バッジ領域の外（左下）は変化しない。
    for y in 80..150u32 {
        for x in 0..55u32 {
            let p = img.get_pixel(x, y);
            assert_eq!((p.r, p.g, p.b, p.a), (base.r, base.g, base.b, base.a));
        }
    }
}

#[test]
fn bake_label_preserves_transparent_silhouette() {
    // 完全に透明な画像（牌の角丸の外側に相当）には何も描かれない。
    let mut img = Image::gen_image_color(111, 150, Color::new(0.0, 0.0, 0.0, 0.0));
    bake_label(&mut img, "E", LABEL_BLACK, &test_font());
    for y in 0..150u32 {
        for x in 0..111u32 {
            assert_eq!(img.get_pixel(x, y).a, 0.0, "pixel ({x},{y}) gained alpha");
        }
    }
}

#[test]
fn bake_label_masks_by_destination_alpha() {
    // 半透明ではなく「不透明部分にだけ」乗ることを、境界を跨いだ画像で確認する。
    let mut img = Image::gen_image_color(111, 150, WHITE);
    // 右端 10px を透明にする（角丸の外側を模す）
    for y in 0..150u32 {
        for x in 101..111u32 {
            img.set_pixel(x, y, Color::new(0.0, 0.0, 0.0, 0.0));
        }
    }
    bake_label(&mut img, "1", LABEL_RED, &test_font());
    for y in 0..150u32 {
        for x in 101..111u32 {
            assert_eq!(img.get_pixel(x, y).a, 0.0);
        }
    }
}

#[test]
#[ignore = "visual check helper"]
fn export_labeled_tiles_for_visual_check() {
    let font = test_font();
    let out = std::env::var("LABEL_EXPORT_DIR").expect("set LABEL_EXPORT_DIR");
    for (name, png) in [
        (
            "1m",
            &include_bytes!("../../../../assets/images/tiles/1m.png")[..],
        ),
        (
            "5s",
            &include_bytes!("../../../../assets/images/tiles/5s.png")[..],
        ),
        (
            "8p",
            &include_bytes!("../../../../assets/images/tiles/8p.png")[..],
        ),
        (
            "1z",
            &include_bytes!("../../../../assets/images/tiles/1z.png")[..],
        ),
        (
            "5z",
            &include_bytes!("../../../../assets/images/tiles/5z.png")[..],
        ),
        (
            "6z",
            &include_bytes!("../../../../assets/images/tiles/6z.png")[..],
        ),
        (
            "7z",
            &include_bytes!("../../../../assets/images/tiles/7z.png")[..],
        ),
        (
            "r5m",
            &include_bytes!("../../../../assets/images/tiles/r5m.png")[..],
        ),
        (
            "9s",
            &include_bytes!("../../../../assets/images/tiles/9s.png")[..],
        ),
    ] {
        let mut img =
            Image::from_file_with_format(png, Some(macroquad::prelude::ImageFormat::Png)).unwrap();
        let tile_type = match name {
            "1m" => Tile::M1,
            "5s" => Tile::S5,
            "8p" => Tile::P8,
            "9s" => Tile::S9,
            "1z" => Tile::Z1,
            "5z" => Tile::Z5,
            "6z" => Tile::Z6,
            "7z" => Tile::Z7,
            "r5m" => Tile::M5,
            _ => unreachable!(),
        };
        let (label, color) = tile_index_label(tile_type);
        bake_label(&mut img, label, color, &font);
        // export_png は保存時に上下反転するため、先に反転して相殺する
        let w = img.width as usize * 4;
        let rows: Vec<Vec<u8>> = img.bytes.chunks(w).rev().map(|r| r.to_vec()).collect();
        img.bytes = rows.concat();
        img.export_png(&format!("{out}/{name}_labeled.png"));
    }
}
