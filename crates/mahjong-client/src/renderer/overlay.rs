//! 鳴き・和了選択オーバーレイの描画
//!
//! チー/ポン選択UI、鳴き確認パネル、和了ボタンを描画する。
//! レイアウト定数は game.rs の入力処理と共有する。

use macroquad::prelude::*;
use mahjong_core::tile::Tile;
use mahjong_server::protocol::AvailableCall;

use crate::game::GameState;
use super::{draw_jp_text, draw_tile_sprite, TileTextures, FONT_SIZE, SMALL_FONT, AGARI_FONT};

// ─── チー／ポン選択UI定数 ─────────────────────────────────────────────────────

/// チー／ポン選択UIの定数（renderer と game.rs 入力処理で共有）
pub const CHI_SEL_TILE_W: f32 = 44.0;
pub const CHI_SEL_TILE_H: f32 = 62.0;
pub const CHI_SEL_TILE_GAP: f32 = 2.0;
pub const CHI_SEL_OPT_SPACING: f32 = 24.0;
pub const CHI_SEL_PANEL_H: f32 = 180.0;

// ─── 鳴きパネル定数 ──────────────────────────────────────────────────────────

/// 鳴きパネルのボタン定数（renderer と game.rs 入力処理で共有）
pub const CALL_BTN_W: f32 = 100.0;
pub const CALL_BTN_H: f32 = 40.0;
pub const CALL_BTN_SPACING: f32 = 10.0;
/// 鳴きパネル内のパディング（renderer と game.rs で共有）
pub const CALL_PANEL_PAD: f32 = 14.0;
/// 鳴きパネル内の捨て牌アイコンサイズ
pub const CALL_PANEL_TILE_W: f32 = 44.0;
pub const CALL_PANEL_TILE_H: f32 = 62.0;
/// ノーロン時：鳴きパネル右端 X 座標（下家の手牌右端に合わせる）
pub const CALL_PANEL_RIGHT_X_NO_RON: f32 = 820.0;
/// ノーロン時：鳴きパネル下端 Y 座標（手牌 y=680 のわずか上）
pub const CALL_PANEL_BOTTOM_Y_NO_RON: f32 = 672.0;
/// ノーロン時：鳴きパネルのボタン基準 Y 座標（panel_y + (panel_h - btn_h)/2 + title_h）
pub const CALL_BTN_BASE_Y_NO_RON: f32 = 624.0;
/// 鳴きオーバーレイパネルの高さ（20 + tile_h(62) + pad(14)）
pub const CALL_OVERLAY_PANEL_H: f32 = 96.0;

// ─── 和了ボタン定数 ──────────────────────────────────────────────────────────

/// 和了ボタンの定数（描画・入力の両方で使用）
/// 右端と下端を鳴きパネルと揃える
pub const AGARI_BTN_W: f32 = 200.0;
pub const AGARI_BTN_H: f32 = 60.0;
pub const AGARI_BTN_X: f32 = CALL_PANEL_RIGHT_X_NO_RON - AGARI_BTN_W; // 620
/// 和了ボタンのデフォルト Y（ロンのみ・ツモ時）= 下端を鳴きパネルと揃える
pub const AGARI_BTN_Y: f32 = CALL_PANEL_BOTTOM_Y_NO_RON - AGARI_BTN_H; // 612
/// 和了ボタンと鳴きパネルを同時表示する際の隙間
pub const AGARI_BTN_GAP: f32 = 8.0;

// ─── エントリポイント ─────────────────────────────────────────────────────────

/// アクションボタン群の描画エントリポイント。mod.rs の draw_game から呼ばれる。
pub(super) fn draw_action_buttons(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) {
    // 選択オーバーレイ表示中は call_overlay の代わりに選択UIを右下に表示
    if state.chi_option_selecting {
        draw_chi_selection_overlay(state, font, tile_textures);
        return;
    }
    if state.pon_option_selecting {
        draw_pon_selection_overlay(state, font, tile_textures);
        return;
    }
    if !state.available_calls.is_empty() {
        draw_call_overlay(state, font, tile_textures);
        return;
    }

    if !state.is_my_turn {
        draw_jp_text(
            font,
            "他のプレイヤーの手番です...",
            480.0,
            640.0,
            FONT_SIZE,
            Color::new(0.8, 0.8, 0.8, 0.7),
        );
        return;
    }

    if state.riichi_selection_mode {
        draw_jp_text(
            font,
            "【リーチ】聴牌になる牌を選んで打牌",
            330.0,
            640.0,
            FONT_SIZE,
            Color::new(1.0, 0.9, 0.3, 1.0),
        );
    } else if state.is_riichi {
        draw_jp_text(
            font,
            "【リーチ中】自動ツモ切り",
            400.0,
            640.0,
            FONT_SIZE,
            Color::new(1.0, 0.3, 0.3, 1.0),
        );
    }

    if state.drawn.is_some() {
        // 和了ボタン（ツモ）を目立つ位置に表示
        if state.can_tsumo {
            draw_agari_button(font, AGARI_BTN_X, AGARI_BTN_Y);
        }

        if state.can_riichi {
            let riichi_bg = Color::new(0.1, 0.6, 0.1, 1.0);
            draw_rectangle(1000.0, 720.0, 80.0, 40.0, riichi_bg);
            draw_rectangle_lines(1000.0, 720.0, 80.0, 40.0, 2.0, WHITE);
            draw_jp_text(font, "リーチ", 1008.0, 747.0, SMALL_FONT, WHITE);
        }

        for (idx, tile) in state.self_kan_options.iter().enumerate() {
            let x = 720.0 + idx as f32 * 110.0;
            let kan_bg = Color::new(0.1, 0.3, 0.8, 1.0);
            draw_rectangle(x, 670.0, 100.0, 40.0, kan_bg);
            draw_rectangle_lines(x, 670.0, 100.0, 40.0, 2.0, WHITE);
            draw_jp_text(
                font,
                &format!("{}カン", tile.to_string()),
                x + 10.0,
                697.0,
                SMALL_FONT,
                WHITE,
            );
        }

        if state.riichi_selection_mode {
            draw_jp_text(
                font,
                "黄色の牌だけがリーチ打牌できます。リーチボタンでも解除できます。",
                100.0,
                770.0,
                SMALL_FONT,
                Color::new(0.9, 0.9, 0.5, 0.8),
            );
        } else if !state.is_riichi {
            draw_jp_text(
                font,
                "牌をクリックで選択、もう一度クリックで打牌",
                100.0,
                770.0,
                SMALL_FONT,
                Color::new(0.8, 0.8, 0.8, 0.7),
            );
        }
    }
}

// ─── 和了ボタン ───────────────────────────────────────────────────────────────

/// 和了ボタンを描画する（ロン・ツモ共通）。x/y は左上座標。
fn draw_agari_button(font: Option<&Font>, x: f32, y: f32) {
    let bg = Color::new(0.9, 0.05, 0.05, 1.0);
    let border = Color::new(1.0, 0.85, 0.0, 1.0);

    draw_rectangle(x, y, AGARI_BTN_W, AGARI_BTN_H, bg);
    draw_rectangle_lines(x, y, AGARI_BTN_W, AGARI_BTN_H, 4.0, border);
    draw_jp_text(font, "和　了", x + 50.0, y + 42.0, AGARI_FONT, WHITE);
}

// ─── 鳴き確認オーバーレイ ────────────────────────────────────────────────────

fn draw_call_overlay(state: &GameState, font: Option<&Font>, tile_textures: &TileTextures) {
    let has_ron = state
        .available_calls
        .iter()
        .any(|c| matches!(c, AvailableCall::Ron));
    let has_non_ron = state
        .available_calls
        .iter()
        .any(|c| !matches!(c, AvailableCall::Ron));

    if !has_non_ron {
        // 鳴きなし・ロンのみ：和了ボタンを右下に単独表示
        if has_ron {
            draw_agari_button(font, AGARI_BTN_X, AGARI_BTN_Y);
        }
        return;
    }

    // 鳴きあり（ロンの有無を問わずパネルを描画、ロンがあれば上に和了ボタンを追加）

    let btn_w = CALL_BTN_W;
    let btn_h = CALL_BTN_H;
    let btn_spacing = CALL_BTN_SPACING;
    let tile_w = CALL_PANEL_TILE_W;
    let tile_h = CALL_PANEL_TILE_H;
    let tile_gap = 12.0_f32; // 牌とボタン間の隙間

    // 非ロンボタンの個数（パスを除く）を数えてパネル幅を決定
    let non_ron_call_count = state
        .available_calls
        .iter()
        .filter(|c| !matches!(c, AvailableCall::Ron))
        .count();
    let total_btn_count = non_ron_call_count + 1; // +1 for pass
    let btns_w = total_btn_count as f32 * btn_w + (total_btn_count - 1) as f32 * btn_spacing;

    // パネル領域: 牌アイコン + ボタン群 + パディング（ロン有無に関わらず同一レイアウト）
    let pad = CALL_PANEL_PAD;
    let tile_area_w = tile_w + tile_gap;
    let panel_w = tile_area_w + btns_w + pad * 2.0;
    let panel_h = CALL_OVERLAY_PANEL_H;
    let panel_x = CALL_PANEL_RIGHT_X_NO_RON - panel_w;
    let panel_y = CALL_PANEL_BOTTOM_Y_NO_RON - panel_h;
    let base_x = panel_x + pad + tile_area_w;
    let base_y = CALL_BTN_BASE_Y_NO_RON;

    // ロン＋鳴き同時：和了ボタンをパネルの上に表示
    if has_ron {
        let agari_y = panel_y - AGARI_BTN_GAP - AGARI_BTN_H;
        draw_agari_button(font, AGARI_BTN_X, agari_y);
    }

    // パネル背景（チー／ポン選択UIと同じスタイル）
    draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::new(0.0, 0.0, 0.0, 0.88));
    draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 2.0, Color::new(1.0, 0.85, 0.3, 1.0));

    // タイトル
    draw_jp_text(
        font,
        "鳴きますか？",
        panel_x + pad,
        panel_y + 30.0,
        FONT_SIZE,
        Color::new(1.0, 0.95, 0.5, 1.0),
    );

    // 捨て牌アイコン
    let tile_x = base_x - tile_area_w;
    let tile_y = base_y + (btn_h - tile_h) / 2.0;
    if let Some(target) = state.call_target_tile {
        draw_tile_sprite(
            tile_textures.for_tile(&target),
            tile_x,
            tile_y,
            tile_w - 8.0,
            tile_h - 8.0,
            WHITE,
        );
    }

    let call_btn_bg = Color::new(0.8, 0.2, 0.2, 1.0);
    let pass_btn_bg = Color::new(0.35, 0.35, 0.35, 1.0);

    let mut btn_idx = 0;

    for call in &state.available_calls {
        if matches!(call, AvailableCall::Ron) {
            continue;
        }
        let x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
        match call {
            AvailableCall::Ron => unreachable!(),
            AvailableCall::Pon { .. } => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "ポン", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
            }
            AvailableCall::Daiminkan => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "カン", x + 18.0, base_y + 27.0, SMALL_FONT, WHITE);
            }
            AvailableCall::Chi { .. } => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "チー", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
            }
        }
        btn_idx += 1;
    }

    let pass_x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
    draw_rectangle(pass_x, base_y, btn_w, btn_h, pass_btn_bg);
    draw_rectangle_lines(pass_x, base_y, btn_w, btn_h, 2.0, WHITE);
    draw_jp_text(font, "パス", pass_x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
}

// ─── チー／ポン選択オーバーレイ ──────────────────────────────────────────────

/// チー選択オーバーレイを描画する。
///
/// 複数のチーの組み合わせがある場合に、プレイヤーが選択できるパネルを右下に表示する。
/// レイアウト定数は CHI_SEL_* を使用し、game.rs の入力処理と一致させること。
fn draw_chi_selection_overlay(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) {
    if let Some(called_tile) = state.call_target_tile {
        draw_meld_selection_overlay(
            font,
            tile_textures,
            "チーの組み合わせを選択",
            called_tile,
            &state.chi_pending_options,
        );
    }
}

/// ポン選択オーバーレイを描画する。
///
/// 赤ドラの有無で2通りの刻子が作れる場合に、どちらでポンするかをプレイヤーが選択できるパネル。
fn draw_pon_selection_overlay(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) {
    if let Some(called_tile) = state.call_target_tile {
        draw_meld_selection_overlay(
            font,
            tile_textures,
            "ポンの組み合わせを選択",
            called_tile,
            &state.pon_pending_options,
        );
    }
}

/// チー／ポン選択オーバーレイの共通描画処理。
///
/// 各オプションは [手牌A, 手牌B] の2枚で、called_tile を加えた3枚をソートして表示する。
/// レイアウト定数は CHI_SEL_* を使用し、game.rs の入力処理と一致させること。
fn draw_meld_selection_overlay(
    font: Option<&Font>,
    tile_textures: &TileTextures,
    title: &str,
    called_tile: Tile,
    options: &[[Tile; 2]],
) {
    let tile_w = CHI_SEL_TILE_W;
    let tile_h = CHI_SEL_TILE_H;
    let tile_gap = CHI_SEL_TILE_GAP;
    let opt_spacing = CHI_SEL_OPT_SPACING;
    let panel_h = CHI_SEL_PANEL_H;

    let opt_count = options.len();
    let opt_w = tile_w * 3.0 + tile_gap * 2.0;
    let panel_w = opt_w * opt_count as f32 + opt_spacing * (opt_count as f32 - 1.0) + 80.0;
    // call_overlay と同じ右下に固定（右端・下端を揃える）
    let panel_x = CALL_PANEL_RIGHT_X_NO_RON - panel_w;
    let panel_y = CALL_PANEL_BOTTOM_Y_NO_RON - panel_h;

    // パネル背景
    draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::new(0.0, 0.0, 0.0, 0.88));
    draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 2.0, Color::new(1.0, 0.85, 0.3, 1.0));

    // タイトル
    draw_jp_text(
        font,
        title,
        panel_x + 20.0,
        panel_y + 30.0,
        FONT_SIZE,
        Color::new(1.0, 0.95, 0.5, 1.0),
    );

    let opts_start_x = panel_x + 40.0;
    let opts_y = panel_y + 52.0;

    let (mouse_x, mouse_y) = mouse_position();

    for (idx, &opt) in options.iter().enumerate() {
        let ox = opts_start_x + idx as f32 * (opt_w + opt_spacing);

        // マウスオーバーで明るいハイライト
        let hovered = mouse_x >= ox
            && mouse_x <= ox + opt_w
            && mouse_y >= opts_y
            && mouse_y <= opts_y + tile_h;
        if hovered {
            draw_rectangle(
                ox - 4.0,
                opts_y - 4.0,
                opt_w + 8.0,
                tile_h + 8.0,
                Color::new(1.0, 1.0, 0.4, 0.22),
            );
            draw_rectangle_lines(
                ox - 4.0,
                opts_y - 4.0,
                opt_w + 8.0,
                tile_h + 8.0,
                2.0,
                Color::new(1.0, 1.0, 0.4, 0.9),
            );
        }

        // 3枚の牌（[hand0, hand1, called_tile] をソートして表示）
        let mut display_tiles = [opt[0], opt[1], called_tile];
        display_tiles.sort();

        for (ti, tile) in display_tiles.iter().enumerate() {
            let tx = ox + ti as f32 * (tile_w + tile_gap);
            // called_tile には薄い黄色のティントで区別
            let tint = if *tile == called_tile {
                Color::new(1.0, 1.0, 0.6, 1.0)
            } else {
                WHITE
            };
            draw_tile_sprite(
                tile_textures.for_tile(tile),
                tx,
                opts_y,
                tile_w - 1.0,
                tile_h - 1.0,
                tint,
            );
        }
    }

    // キャンセルボタン
    let cancel_w: f32 = 120.0;
    let cancel_h: f32 = 36.0;
    let cancel_x = panel_x + (panel_w - cancel_w) / 2.0;
    let cancel_y = panel_y + panel_h - cancel_h - 14.0;
    let cancel_hovered = mouse_x >= cancel_x
        && mouse_x <= cancel_x + cancel_w
        && mouse_y >= cancel_y
        && mouse_y <= cancel_y + cancel_h;
    let cancel_bg = if cancel_hovered {
        Color::new(0.5, 0.5, 0.5, 1.0)
    } else {
        Color::new(0.3, 0.3, 0.3, 1.0)
    };
    draw_rectangle(cancel_x, cancel_y, cancel_w, cancel_h, cancel_bg);
    draw_rectangle_lines(cancel_x, cancel_y, cancel_w, cancel_h, 2.0, WHITE);
    draw_jp_text(
        font,
        "キャンセル",
        cancel_x + 10.0,
        cancel_y + 24.0,
        FONT_SIZE,
        WHITE,
    );
}
