//! Unit tests for the rendering layout.

use super::board::{
    DISCARD_FIRST_ROW_OFFSET, DISCARD_TILE_H, DORA_PANEL_HEIGHT, DORA_PANEL_Y,
    TABLE_DORA_INDICATOR_TILE_H, TABLE_DORA_INDICATOR_TILE_W, TOP_BAR_HEIGHT,
};
use super::{
    BOARD_CENTER_Y, DORA_TILE_TINT, DRAWN_GAP, HAND_Y, PLAYER_ROTATIONS,
    PUBLIC_TILE_HIGHLIGHT_TINT, SELF_HAND_LEFT_MARGIN, SELF_HAND_MELD_GAP, SELF_MELD_GAP,
    SELF_MELD_RIGHT_EDGE, SELF_MELD_TILE_H, SELF_MELD_TILE_W, TILE_W, add_dora_tile_types,
    buffer_to_design, calc_meld_width, dora_tile_tint, dora_tile_tint_with_base, dora_tile_types,
    other_meld_x_positions, pei_tile_x, player_hand_start_x, player_hand_tile_x, public_tile_tint,
    public_tile_tint_with_base, rotation_index, score_chip_entries, score_delta_color,
    seat_at_relative_position, self_meld_x_positions, visible_tile_tint,
};
use super::{
    format_score_delta,
    result::{
        RESULT_INDICATOR_ROW_HEIGHT, RESULT_INDICATOR_TILE_H, RESULT_INDICATOR_TILE_W,
        WIN_HAND_ROW_HEIGHT, WIN_HAND_TILE_H, WIN_HAND_TILE_W, WIN_PANEL_CONTENT_WIDTH,
        WIN_PANEL_WIDTH, WIN_TILE_OUTLINE_MARGIN, YAKU_ROW_HEIGHT, result_indicator_rows_width,
        win_hand_layout, yaku_right_aligned_x, yaku_row_baseline,
    },
    theme,
};
use crate::game::{GameState, SelfTedashiAnim, SelfTileOrigin};
use macroquad::prelude::TextDimensions;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType};

#[test]
fn named_font_sizes_match_the_rendered_pixel_sizes() {
    let named_sizes = [
        theme::font_size::MICRO,
        theme::font_size::TINY,
        theme::font_size::CAPTION,
        theme::font_size::SMALL,
        theme::font_size::LABEL,
        theme::font_size::BODY,
        theme::font_size::BODY_LARGE,
        theme::font_size::SUBHEADING,
        theme::font_size::HEADING_SMALL,
        theme::font_size::HEADING,
        theme::font_size::HEADING_LARGE,
        theme::font_size::TITLE_SMALL,
        theme::font_size::TITLE,
        theme::font_size::DISPLAY,
        theme::font_size::DISPLAY_LARGE,
    ];
    let expected = [13, 14, 15, 16, 17, 18, 19, 20, 22, 24, 25, 29, 31, 34, 38];

    assert_eq!(named_sizes, expected);
    assert_eq!(&theme::font_size::ALL[..], &expected);
}

#[test]
fn font_size_catalog_is_strictly_increasing() {
    assert!(
        theme::font_size::ALL
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
}

#[test]
fn yaku_text_and_shadow_are_centered_between_row_dividers() {
    let name = TextDimensions {
        width: 48.0,
        height: 16.0,
        offset_y: 15.0,
    };
    let han = TextDimensions {
        width: 24.0,
        height: 17.0,
        offset_y: 14.0,
    };
    let row_top = 100.0;
    let baseline = yaku_row_baseline(row_top, YAKU_ROW_HEIGHT, name, Some(han));
    let visible_top = baseline - name.offset_y.max(han.offset_y);
    let visible_bottom = baseline
        + (name.height - name.offset_y).max(han.height - han.offset_y)
        + theme::TEXT_SHADOW_OFFSET;

    assert!((visible_top - row_top - (row_top + YAKU_ROW_HEIGHT - visible_bottom)).abs() < 0.001);
}

#[test]
fn yaku_han_shadow_ends_at_the_row_divider_edge() {
    let dimensions = TextDimensions {
        width: 24.0,
        height: 17.0,
        offset_y: 14.0,
    };
    let right = 620.0;
    let x = yaku_right_aligned_x(right, dimensions);

    assert_eq!(x + dimensions.width + theme::TEXT_SHADOW_OFFSET, right);
}

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

#[test]
fn public_tile_highlight_matches_red_and_normal_fives_by_kind() {
    let red_five = Tile::new_red(Tile::M5);
    assert_eq!(
        public_tile_tint(&red_five, Some(Tile::M5)),
        PUBLIC_TILE_HIGHLIGHT_TINT
    );
    assert_eq!(public_tile_tint(&red_five, Some(Tile::P5)), WHITE);
}

#[test]
fn public_tile_highlight_preserves_called_discard_transparency() {
    let tile = Tile::new(Tile::S3);
    let base = Color::new(0.72, 0.72, 0.72, 0.28);
    let tint = public_tile_tint_with_base(&tile, Some(Tile::S3), base);

    assert_eq!(tint.a, 0.28);
    assert_eq!(tint.r, PUBLIC_TILE_HIGHLIGHT_TINT.r);
    assert_eq!(tint.g, PUBLIC_TILE_HIGHLIGHT_TINT.g);
    assert_eq!(tint.b, PUBLIC_TILE_HIGHLIGHT_TINT.b);
}

#[test]
fn dora_tiles_use_a_light_yellow_tint() {
    let dora_types = dora_tile_types(&[Tile::new(Tile::M4)], false);

    assert_eq!(
        dora_tile_tint(&Tile::new(Tile::M5), &dora_types),
        DORA_TILE_TINT
    );
    assert_eq!(
        dora_tile_tint(&Tile::new_red(Tile::P5), &dora_types),
        DORA_TILE_TINT
    );
    assert_eq!(dora_tile_tint(&Tile::new(Tile::P5), &dora_types), WHITE);
}

#[test]
fn dora_tint_preserves_transparency() {
    let tile = Tile::new(Tile::S3);
    let dora_types = dora_tile_types(&[Tile::new(Tile::S2)], false);
    let base = Color::new(0.72, 0.72, 0.72, 0.28);
    let tint = dora_tile_tint_with_base(&tile, &dora_types, base);

    assert_eq!(tint.a, 0.28);
    assert_eq!(tint.r, DORA_TILE_TINT.r);
    assert_eq!(tint.g, DORA_TILE_TINT.g);
    assert_eq!(tint.b, DORA_TILE_TINT.b);
}

#[test]
fn selected_public_tile_tint_takes_priority_over_dora_tint() {
    let tile = Tile::new(Tile::Z1);
    let dora_types = dora_tile_types(&[Tile::new(Tile::Z4)], false);

    assert_eq!(
        visible_tile_tint(&tile, Some(Tile::Z1), &dora_types),
        PUBLIC_TILE_HIGHLIGHT_TINT
    );
}

#[test]
fn dora_types_include_three_player_and_uradora_indicators() {
    let mut dora_types = dora_tile_types(&[Tile::new(Tile::M1)], true);
    add_dora_tile_types(&mut dora_types, &[Tile::new(Tile::P4)], true);

    assert!(dora_types[Tile::M9 as usize]);
    assert!(!dora_types[Tile::M2 as usize]);
    assert!(dora_types[Tile::P5 as usize]);
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

/// Minimal called quad for the layout tests.
fn called_kan(tile_type: TileType) -> Meld {
    let tile = Tile::new(tile_type);
    Meld {
        tiles: vec![tile; 4],
        category: MeldType::Kan,
        from: MeldFrom::Previous,
        called_tile: Some(tile),
    }
}

/// Minimal added quad for vertical result-layout tests.
fn added_kan(tile_type: TileType) -> Meld {
    let tile = Tile::new(tile_type);
    Meld {
        tiles: vec![tile; 4],
        category: MeldType::Kakan,
        from: MeldFrom::Previous,
        called_tile: Some(tile),
    }
}

#[test]
fn enlarged_dora_indicators_fit_their_rows() {
    const {
        assert!(TABLE_DORA_INDICATOR_TILE_H <= DORA_PANEL_HEIGHT);
        assert!(DORA_PANEL_Y + DORA_PANEL_HEIGHT <= TOP_BAR_HEIGHT);
        assert!(RESULT_INDICATOR_TILE_H <= RESULT_INDICATOR_ROW_HEIGHT);
    }
    assert_eq!(TABLE_DORA_INDICATOR_TILE_W, 26.0);
    assert_eq!(TABLE_DORA_INDICATOR_TILE_H, 37.0);
    assert_eq!(RESULT_INDICATOR_TILE_W, 30.0);
    assert_eq!(RESULT_INDICATOR_TILE_H, 42.0);

    let ten_tiles_without_labels = result_indicator_rows_width(5, 5, 0.0, 0.0);
    assert_eq!(ten_tiles_without_labels, 350.0);
    assert!(ten_tiles_without_labels < WIN_PANEL_CONTENT_WIDTH);
}

#[test]
fn normal_win_hand_uses_the_enlarged_tile_size() {
    const {
        assert!(WIN_HAND_TILE_H + 2.0 * WIN_TILE_OUTLINE_MARGIN <= WIN_HAND_ROW_HEIGHT);
    }
    let layout = win_hand_layout(13, true, &[]);

    assert_eq!(WIN_PANEL_WIDTH, 780.0);
    assert_eq!(WIN_PANEL_CONTENT_WIDTH, 700.0);
    assert_eq!(layout.tile_w, WIN_HAND_TILE_W);
    assert_eq!(layout.tile_h, WIN_HAND_TILE_H);
    assert_eq!(layout.row_width, 574.0);
}

#[test]
fn widest_win_hand_scales_down_to_the_panel_width() {
    let melds = vec![
        called_kan(Tile::M1),
        called_kan(Tile::P2),
        called_kan(Tile::S3),
        called_kan(Tile::Z1),
    ];
    let layout = win_hand_layout(1, true, &melds);

    assert_eq!(WIN_HAND_TILE_W, 40.0);
    assert_eq!(WIN_HAND_TILE_H, 56.0);
    assert!(layout.tile_w >= 34.0);
    assert!(layout.tile_h >= 47.0);
    assert!(layout.tile_w < WIN_HAND_TILE_W);
    assert!((layout.row_width - WIN_PANEL_CONTENT_WIDTH).abs() < 0.001);
}

#[test]
fn added_quad_reserves_space_above_the_win_hand() {
    let layout = win_hand_layout(10, true, &[added_kan(Tile::M1)]);

    assert_eq!(layout.tile_w, WIN_HAND_TILE_W);
    assert_eq!(layout.tile_h, WIN_HAND_TILE_H);
    assert_eq!(layout.top_overhang, 24.0);
}

#[test]
fn enlarged_opponent_tiles_fit_between_the_top_bar_and_our_hand() {
    const DISCARD_ROW_COUNT: f32 = 3.0;

    let normal = opponent_tile_layout(13, true, false, &[]);
    assert_eq!(OTHER_HAND_TILE_W, 33.0);
    assert_eq!(OTHER_HAND_TILE_H, 46.0);
    assert_eq!(normal.tile_w, OTHER_HAND_TILE_W);
    assert_eq!(normal.tile_h, OTHER_HAND_TILE_H);
    assert_eq!(normal.row_width, 470.0);

    let melds = vec![
        called_kan(Tile::M1),
        called_kan(Tile::P2),
        called_kan(Tile::S3),
        called_kan(Tile::Z1),
    ];
    let widest = opponent_tile_layout(1, true, false, &melds);
    let half_width = widest.row_width / 2.0;

    assert!(widest.tile_w < OTHER_HAND_TILE_W);
    assert!(widest.row_width <= OTHER_HAND_MAX_ROW_WIDTH + 0.001);
    assert!(BOARD_CENTER_Y - half_width >= TOP_BAR_HEIGHT);
    assert!(BOARD_CENTER_Y + half_width <= HAND_Y);
    assert_eq!(BOARD_CENTER_Y - OTHER_HAND_OUTER_DISTANCE, TOP_BAR_HEIGHT);

    let discard_edge =
        BOARD_CENTER_Y + DISCARD_FIRST_ROW_OFFSET + DISCARD_ROW_COUNT * DISCARD_TILE_H;
    let stacked_kakan_edge = BOARD_CENTER_Y + OTHER_HAND_OUTER_DISTANCE - 2.0 * OTHER_HAND_TILE_W;
    assert_eq!(stacked_kakan_edge - discard_edge, 2.0);
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
    let (tw, th, gap, start_x) = (OTHER_HAND_TILE_W, OTHER_HAND_TILE_H, OTHER_MELD_GAP, 100.0);
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
fn score_chips_follow_rankings_without_changing_player_identity() {
    let mut state = GameState::new();
    state.my_seat = 2;
    state.initial_dealer_seat = 1;
    state.scores = [24_000, 30_000, 25_000, 21_000];

    let summary: Vec<_> = score_chip_entries(&state)
        .iter()
        .map(|chip| (chip.seat, chip.relative_index, chip.rank, chip.score_delta))
        .collect();

    assert_eq!(
        summary,
        vec![
            (1, 3, 0, Some(5_000)),
            (2, 0, 1, None),
            (0, 2, 2, Some(-1_000)),
            (3, 1, 3, Some(-4_000)),
        ]
    );
}

#[test]
fn score_chips_keep_zero_delta_for_tied_opponents_only() {
    let state = GameState::new();

    let deltas: Vec<_> = score_chip_entries(&state)
        .iter()
        .map(|chip| chip.score_delta)
        .collect();

    assert_eq!(deltas, vec![None, Some(0), Some(0), Some(0)]);
}

#[test]
fn score_chips_exclude_the_unused_sanma_seat() {
    let mut state = GameState::new();
    state.player_count = 3;
    state.my_seat = 1;
    state.scores = [35_000, 34_000, 36_000, 999_999];

    let seats: Vec<_> = score_chip_entries(&state)
        .iter()
        .map(|chip| chip.seat)
        .collect();

    assert_eq!(seats, vec![2, 0, 1]);
}

#[test]
fn score_deltas_have_explicit_signs_and_grouped_digits() {
    assert_eq!(format_score_delta(12_300), "+12,300");
    assert_eq!(format_score_delta(0), "±0");
    assert_eq!(format_score_delta(-4_500), "-4,500");
}

#[test]
fn score_delta_colors_distinguish_ahead_behind_and_tied() {
    assert_eq!(score_delta_color(1), theme::BLUE_LT);
    assert_eq!(score_delta_color(-1), theme::RED_LT);
    assert_eq!(score_delta_color(0), theme::TEXT_DIM);
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

#[test]
fn sanma_pei_tiles_do_not_overlap() {
    let tile_width = 24.0;
    let positions: Vec<_> = (0..4)
        .map(|index| pei_tile_x(100.0, tile_width, index))
        .collect();

    for pair in positions.windows(2) {
        assert!(pair[0] + tile_width < pair[1]);
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
    OTHER_HAND_MAX_ROW_WIDTH, OTHER_HAND_OUTER_DISTANCE, OTHER_HAND_TILE_H, OTHER_HAND_TILE_W,
    OTHER_MELD_GAP, TEDASHI_GAP_HOLD_SECS, TEDASHI_SLIDE_SECS, opponent_drawn_slot_width,
    opponent_tile_layout, tedashi_progress, tedashi_tile_offset,
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
fn tedashi_offsets_without_drawn_only_move_tiles_right_of_gap() {
    let (step, gap) = (28.0, 8.0);
    // A post-call discard must leave tiles left of the discarded tile
    // fixed while tiles to its right close the gap.
    assert_eq!(tedashi_tile_offset(0, 10, 2, false, step, gap, 0.0), 0.0);
    assert_eq!(tedashi_tile_offset(5, 10, 2, false, step, gap, 0.0), step);
    // No offsets at the final position.
    assert_eq!(tedashi_tile_offset(0, 10, 2, false, step, gap, 1.0), 0.0);
}

#[test]
fn post_call_discard_keeps_hand_edges_and_melds_fixed() {
    let (step, gap) = (28.0, 8.0);
    let before_count = 11;
    let after_count = 10;
    let before_slot = opponent_drawn_slot_width(before_count, false, step, gap);
    let after_slot = opponent_drawn_slot_width(after_count, false, step, gap);

    assert_eq!(
        before_slot, gap,
        "the extra concealed tile should occupy the drawn-tile slot immediately after a call"
    );
    assert_eq!(
        after_slot,
        step + gap,
        "the normal drawn-tile slot should be restored after the discard"
    );
    assert_eq!(
        before_count as f32 * step + before_slot,
        after_count as f32 * step + after_slot,
        "the hand bounds and following meld position should stay fixed across the discard"
    );
}

#[test]
fn own_post_call_discard_keeps_the_hand_left_edge_fixed() {
    let melds = vec![pon(Tile::M1)];
    assert_eq!(
        player_hand_start_x(11, &melds),
        player_hand_start_x(10, &melds),
        "the left edge of the hand should not move on the discard after a call"
    );
}

#[test]
fn own_hand_stays_centered_while_melds_leave_enough_room() {
    let melds = vec![pon(Tile::M1), pon(Tile::P2)];
    let hand_len = 7;
    let centered_x = (super::DESIGN_W - hand_len as f32 * TILE_W) / 2.0;

    assert_eq!(player_hand_start_x(hand_len, &melds), centered_x);
}

#[test]
fn own_hand_shifts_left_to_clear_three_melds() {
    let melds = vec![pon(Tile::M1), pon(Tile::P2), pon(Tile::S3)];
    let hand_len = 4;
    let centered_x = (super::DESIGN_W - hand_len as f32 * TILE_W) / 2.0;
    let hand_start_x = player_hand_start_x(hand_len, &melds);
    let reserved_hand_right_x = hand_start_x + hand_len as f32 * TILE_W + DRAWN_GAP + TILE_W;
    let meld_left_x = self_meld_x_positions(
        &melds,
        SELF_MELD_TILE_W,
        SELF_MELD_TILE_H,
        SELF_MELD_GAP,
        SELF_MELD_RIGHT_EDGE,
    )
    .into_iter()
    .fold(f32::INFINITY, f32::min);

    assert!(
        hand_start_x < centered_x,
        "the hand should shift left only when it would overlap"
    );
    assert_eq!(
        meld_left_x - reserved_hand_right_x,
        SELF_HAND_MELD_GAP,
        "a fixed gap should remain between the reserved drawn-tile slot and melds"
    );
}

#[test]
fn own_hand_and_four_called_quads_fit_without_overlap() {
    let melds = vec![
        called_kan(Tile::M1),
        called_kan(Tile::P2),
        called_kan(Tile::S3),
        called_kan(Tile::Z1),
    ];
    let hand_len = 1;
    let hand_start_x = player_hand_start_x(hand_len, &melds);
    let reserved_hand_right_x = hand_start_x + hand_len as f32 * TILE_W + DRAWN_GAP + TILE_W;
    let meld_left_x = self_meld_x_positions(
        &melds,
        SELF_MELD_TILE_W,
        SELF_MELD_TILE_H,
        SELF_MELD_GAP,
        SELF_MELD_RIGHT_EDGE,
    )
    .into_iter()
    .fold(f32::INFINITY, f32::min);

    assert!(hand_start_x >= SELF_HAND_LEFT_MARGIN);
    assert!(reserved_hand_right_x + SELF_HAND_MELD_GAP <= meld_left_x);
}

#[test]
fn own_tedashi_tiles_move_from_pre_discard_origins() {
    let mut state = GameState::new();
    state.hand = vec![
        Tile::new(Tile::M1),
        Tile::new(Tile::M3),
        Tile::new(Tile::P9),
    ];
    state.self_tedashi_anim = Some(SelfTedashiAnim {
        origins: vec![
            SelfTileOrigin::Hand(0),
            SelfTileOrigin::Hand(2),
            SelfTileOrigin::Drawn,
        ],
        pre_hand_len: 3,
        started_at: 100.0,
    });
    let start_x = player_hand_start_x(3, &state.melds);

    assert_eq!(player_hand_tile_x(&state, 0, 100.0), start_x);
    assert_eq!(
        player_hand_tile_x(&state, 1, 100.0),
        start_x + 2.0 * TILE_W,
        "the vacated discard position should remain open"
    );
    assert_eq!(
        player_hand_tile_x(&state, 2, 100.0),
        start_x + 3.0 * TILE_W + DRAWN_GAP,
        "the drawn tile should start moving from the drawn-tile position"
    );

    let finished_at = 100.0 + TEDASHI_GAP_HOLD_SECS + TEDASHI_SLIDE_SECS;
    for i in 0..state.hand.len() {
        assert_eq!(
            player_hand_tile_x(&state, i, finished_at),
            start_x + i as f32 * TILE_W
        );
    }
}
