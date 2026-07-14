//! Unit tests for the rendering layout.

use super::{
    PLAYER_ROTATIONS, buffer_to_design, calc_meld_width, other_meld_x_positions, rotation_index,
    seat_at_relative_position, self_meld_x_positions,
};
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType};

#[test]
fn buffer_to_design_scales_each_axis_independently() {
    assert_eq!(
        buffer_to_design(640.0, 500.0, 1280.0, 1000.0),
        (640.0, 400.0)
    );
    assert_eq!(
        buffer_to_design(800.0, 400.0, 1600.0, 800.0),
        (640.0, 400.0)
    );
}

#[test]
fn buffer_to_design_handles_zero_sized_surface() {
    assert_eq!(buffer_to_design(10.0, 20.0, 0.0, 800.0), (0.0, 0.0));
}

/// Minimal pon meld for the ordering tests.
fn pon(tile_type: TileType) -> Meld {
    let tile = Tile::new(tile_type);
    Meld {
        tiles: vec![tile, tile, tile],
        category: MeldType::Pon,
        from: MeldFrom::Previous,
        called_tile: Some(tile),
    }
}

// Melds must lay out earliest-rightmost from the player's view;
// our own pack right-to-left from the right edge.
#[test]
fn self_melds_place_first_called_on_the_right() {
    let (tw, th, gap, right_edge) = (40.0, 56.0, 12.0, 1220.0);
    let melds = vec![pon(Tile::M1), pon(Tile::P2), pon(Tile::S3)];
    let xs = self_meld_x_positions(&melds, tw, th, gap, right_edge);

    assert_eq!(xs.len(), 3);
    // The earliest meld (index 0) is rightmost.
    assert!(
        xs[0] > xs[1],
        "first-called meld must be right of the second"
    );
    assert!(
        xs[1] > xs[2],
        "second-called meld must be right of the third"
    );
    // The first meld must not overflow the right edge.
    assert!(xs[0] + calc_meld_width(&melds[0], tw, th) <= right_edge);
}

// Opponents' melds are also earliest-rightmost from their own view.
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
    // The layout starts at start_x beside the hand and never
    // overflows left.
    assert!(xs[2] >= start_x);
}

#[test]
fn player_rotations_place_turn_order_counterclockwise() {
    assert_eq!(PLAYER_ROTATIONS, [0.0, -90.0, 180.0, 90.0]);
}

#[test]
fn seat0_relative_positions_match_seat_indices() {
    // At seat 0 (local play) relative positions equal seat indices.
    for rel in 0..4 {
        assert_eq!(seat_at_relative_position(0, rel, 4), rel);
    }
}

#[test]
fn nonzero_seat_maps_relative_positions_to_correct_seats() {
    // Online non-host: relative positions rotate correctly from any
    // seat. Self (0) = seat 2, right (1) = seat 3, across (2) = seat 0,
    // left (3) = seat 1.
    assert_eq!(seat_at_relative_position(2, 0, 4), 2);
    assert_eq!(seat_at_relative_position(2, 1, 4), 3);
    assert_eq!(seat_at_relative_position(2, 2, 4), 0);
    assert_eq!(seat_at_relative_position(2, 3, 4), 1);
}

#[test]
fn sanma_seat_mapping_wraps_at_three() {
    // Three-player: seats cycle over 0-2.
    assert_eq!(seat_at_relative_position(0, 0, 3), 0);
    assert_eq!(seat_at_relative_position(0, 1, 3), 1);
    assert_eq!(seat_at_relative_position(0, 2, 3), 2);
    assert_eq!(seat_at_relative_position(1, 2, 3), 0);
    assert_eq!(seat_at_relative_position(2, 1, 3), 0);
}

#[test]
fn sanma_rotation_leaves_north_position_empty() {
    assert_eq!(rotation_index(0, 3, 0), 0);
    assert_eq!(rotation_index(1, 3, 0), 1);
    assert_eq!(rotation_index(2, 3, 0), 2);
    assert_eq!(rotation_index(0, 3, 1), 0);
    assert_eq!(rotation_index(1, 3, 1), 1);
    assert_eq!(rotation_index(2, 3, 1), 3);
    assert_eq!(rotation_index(0, 3, 2), 0);
    assert_eq!(rotation_index(1, 3, 2), 2);
    assert_eq!(rotation_index(2, 3, 2), 3);
    for wind in 0..4 {
        for rel in 0..4 {
            assert_eq!(rotation_index(rel, 4, wind), rel);
        }
    }
}

// --- Tile index labels (English UI) ---

use super::labels::{
    LABEL_BLACK, LABEL_BLUE, LABEL_GREEN, LABEL_RED, bake_label, tile_index_label,
};
use macroquad::prelude::{Color, Image, WHITE};

#[test]
fn number_tiles_get_suit_colored_digits() {
    // Suit digits: characters red, circles blue, bamboos green.
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
    // Winds E/S/W/N in black; dragons use the export-set P/F/C.
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

/// Loads the bundled font for the tests.
fn test_font() -> fontdue::Font {
    let bytes: &[u8] = include_bytes!("../../../../assets/fonts/ShipporiMincho-Regular.ttf");
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("font")
}

#[test]
fn bake_label_paints_top_right_and_leaves_rest_untouched() {
    // Bake onto a solid opaque image; only the top-right may change.
    let mut img = Image::gen_image_color(111, 150, Color::new(0.5, 0.5, 0.5, 1.0));
    // Colors quantize to u8 on store, so compare against values read
    // back from the image.
    let base = img.get_pixel(0, 0);
    bake_label(&mut img, "5", LABEL_RED, &test_font());

    // The badge area must show both the white badge and the red glyph.
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

    // Outside the badge (bottom-left) nothing changes.
    for y in 80..150u32 {
        for x in 0..55u32 {
            let p = img.get_pixel(x, y);
            assert_eq!((p.r, p.g, p.b, p.a), (base.r, base.g, base.b, base.a));
        }
    }
}

#[test]
fn bake_label_preserves_transparent_silhouette() {
    // A fully transparent image (outside the tile silhouette)
    // stays untouched.
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
    // Labels must land on opaque pixels only, checked across
    // the boundary.
    let mut img = Image::gen_image_color(111, 150, WHITE);
    // The right 10px are transparent, mimicking the rounded corner.
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
        // export_png flips vertically on save; pre-flip to cancel it.
        let w = img.width as usize * 4;
        let rows: Vec<Vec<u8>> = img.bytes.chunks(w).rev().map(|r| r.to_vec()).collect();
        img.bytes = rows.concat();
        img.export_png(&format!("{out}/{name}_labeled.png"));
    }
}

// --- Opponent hand-discard animation ---

use super::tiles::{
    TEDASHI_GAP_HOLD_SECS, TEDASHI_SLIDE_SECS, tedashi_progress, tedashi_tile_offset,
};

#[test]
fn tedashi_progress_holds_gap_then_finishes() {
    // Progress stays 0 while the gap shows.
    assert_eq!(tedashi_progress(0.0), 0.0);
    assert_eq!(tedashi_progress(TEDASHI_GAP_HOLD_SECS * 0.9), 0.0);
    // Progress rises monotonically during the slide.
    let mid = tedashi_progress(TEDASHI_GAP_HOLD_SECS + TEDASHI_SLIDE_SECS / 2.0);
    assert!(0.0 < mid && mid < 1.0);
    // Progress clamps to 1 after the slide.
    assert_eq!(
        tedashi_progress(TEDASHI_GAP_HOLD_SECS + TEDASHI_SLIDE_SECS),
        1.0
    );
    assert_eq!(tedashi_progress(999.0), 1.0);
}

#[test]
fn tedashi_offsets_show_gap_at_discarded_position() {
    let (step, gap) = (28.0, 8.0);
    // Discard the fifth tile (index 4) from a 13-tile hand with a drawn
    // tile. At progress 0: tiles left of the gap are still, tiles right
    // of it sit one tile to the right, and the last tile (the former
    // drawn tile) sits at the drawn-tile slot.
    assert_eq!(tedashi_tile_offset(3, 13, 4, true, step, gap, 0.0), 0.0);
    assert_eq!(tedashi_tile_offset(4, 13, 4, true, step, gap, 0.0), step);
    assert_eq!(tedashi_tile_offset(11, 13, 4, true, step, gap, 0.0), step);
    assert_eq!(
        tedashi_tile_offset(12, 13, 4, true, step, gap, 0.0),
        step + gap
    );
    // At progress 1 every tile is at its final position (no offset).
    for i in 0..13 {
        assert_eq!(tedashi_tile_offset(i, 13, 4, true, step, gap, 1.0), 0.0);
    }
}

#[test]
fn tedashi_offsets_without_drawn_include_recentering_shift() {
    let (step, gap) = (28.0, 8.0);
    // A post-call discard (no drawn tile) shrinks the hand by one, so
    // centering shifts half a tile and tiles left of the gap slide too.
    assert_eq!(
        tedashi_tile_offset(0, 10, 2, false, step, gap, 0.0),
        -step / 2.0
    );
    assert_eq!(
        tedashi_tile_offset(5, 10, 2, false, step, gap, 0.0),
        step / 2.0
    );
    // No offsets at the final position.
    assert_eq!(tedashi_tile_offset(0, 10, 2, false, step, gap, 1.0), 0.0);
}
