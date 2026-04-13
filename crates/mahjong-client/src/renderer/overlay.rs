//! 鳴き・和了選択オーバーレイの描画とクリック判定
//!
//! 各関数が描画とクリック判定を同時に行い、クリックされた場合に `Some(OverlayClick)` を返す。

use macroquad::prelude::*;
use mahjong_core::tile::Tile;
use mahjong_server::protocol::{AvailableCall, ClientAction};

use super::{AGARI_FONT, FONT_SIZE, SMALL_FONT, TileTextures, draw_jp_text, draw_tile_sprite};
use crate::game::GameState;

// ─── チー／ポン選択UI定数 ─────────────────────────────────────────────────────

const CHI_SEL_TILE_W: f32 = 44.0;
const CHI_SEL_TILE_H: f32 = 62.0;
const CHI_SEL_TILE_GAP: f32 = 2.0;
const CHI_SEL_OPT_SPACING: f32 = 24.0;
const CHI_SEL_PANEL_H: f32 = 180.0;

// ─── 鳴きパネル定数 ──────────────────────────────────────────────────────────

const CALL_BTN_W: f32 = 100.0;
const CALL_BTN_H: f32 = 40.0;
const CALL_BTN_SPACING: f32 = 10.0;
const CALL_PANEL_PAD: f32 = 14.0;
const CALL_PANEL_TILE_W: f32 = 44.0;
const CALL_PANEL_TILE_H: f32 = 62.0;
/// ノーロン時：鳴きパネル右端 X 座標（下家の手牌右端に合わせる）
const CALL_PANEL_RIGHT_X_NO_RON: f32 = 820.0;
/// ノーロン時：鳴きパネル下端 Y 座標（手牌 y=680 のわずか上）
const CALL_PANEL_BOTTOM_Y_NO_RON: f32 = 672.0;
/// ノーロン時：鳴きパネルのボタン基準 Y 座標
const CALL_BTN_BASE_Y_NO_RON: f32 = 624.0;
/// 鳴きオーバーレイパネルの高さ
const CALL_OVERLAY_PANEL_H: f32 = 96.0;

// ─── 和了ボタン定数 ──────────────────────────────────────────────────────────

const AGARI_BTN_W: f32 = 200.0;
const AGARI_BTN_H: f32 = 60.0;
const AGARI_BTN_X: f32 = CALL_PANEL_RIGHT_X_NO_RON - AGARI_BTN_W; // 620
const AGARI_BTN_Y: f32 = CALL_PANEL_BOTTOM_Y_NO_RON - AGARI_BTN_H; // 612
const AGARI_BTN_GAP: f32 = 8.0;

// ─── ヒット判定ヘルパー ───────────────────────────────────────────────────────

fn hit_rect(mx: f32, my: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

// ─── 公開型 ──────────────────────────────────────────────────────────────────

/// オーバーレイボタンのクリック結果
pub enum OverlayClick {
    /// サーバに送信するアクション
    Action(ClientAction),
    /// リーチモードを切り替える
    ToggleRiichi,
    /// チー選択UIを表示する（複数の組み合わせがある場合）
    ShowChiSelection { options: Vec<[Tile; 2]> },
    /// ポン選択UIを表示する（複数の組み合わせがある場合）
    ShowPonSelection { options: Vec<[Tile; 2]> },
    /// 選択UIをキャンセルして鳴きパネルに戻る
    CancelMeldSelection,
    /// 九種九牌を宣言して流局する
    NineTerminalsDeclare,
    /// 九種九牌を宣言せず続行する
    NineTerminalsPass,
}

// ─── エントリポイント ─────────────────────────────────────────────────────────

/// アクションボタン群を描画し、クリックされたボタンを返す。mod.rs の draw_game から呼ばれる。
pub(super) fn draw_action_buttons(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
) -> Option<OverlayClick> {
    let clicked = is_mouse_button_pressed(MouseButton::Left);
    let (mx, my) = mouse_position();

    // 九種九牌選択オーバーレイ
    if state.nine_terminals_pending {
        return draw_nine_terminals_overlay(font, clicked, mx, my);
    }

    // 選択オーバーレイ表示中は call_overlay の代わりに選択UIを右下に表示
    if state.chi_option_selecting {
        return draw_chi_selection_overlay(state, font, tile_textures, clicked, mx, my);
    }
    if state.pon_option_selecting {
        return draw_pon_selection_overlay(state, font, tile_textures, clicked, mx, my);
    }
    if !state.available_calls.is_empty() {
        return draw_call_overlay(state, font, tile_textures, clicked, mx, my);
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
        return None;
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

    state.drawn?;

    let mut result = None;

    // 和了ボタン（ツモ）
    if state.can_tsumo {
        draw_agari_button(font, AGARI_BTN_X, AGARI_BTN_Y);
        if clicked
            && result.is_none()
            && hit_rect(mx, my, AGARI_BTN_X, AGARI_BTN_Y, AGARI_BTN_W, AGARI_BTN_H)
        {
            result = Some(OverlayClick::Action(ClientAction::Tsumo));
        }
    }

    // リーチボタン
    if state.can_riichi {
        const RIICHI_BTN_W: f32 = 80.0;
        const RIICHI_BTN_H: f32 = 40.0;
        // ツモボタンと重ならないよう、ツモボタンがある場合は上にずらす
        let riichi_y = if state.can_tsumo {
            AGARI_BTN_Y - RIICHI_BTN_H - AGARI_BTN_GAP
        } else {
            AGARI_BTN_Y
        };
        let riichi_bg = Color::new(0.8, 0.2, 0.2, 1.0);
        draw_rectangle(AGARI_BTN_X, riichi_y, RIICHI_BTN_W, RIICHI_BTN_H, riichi_bg);
        draw_rectangle_lines(
            AGARI_BTN_X,
            riichi_y,
            RIICHI_BTN_W,
            RIICHI_BTN_H,
            2.0,
            WHITE,
        );
        draw_jp_text(
            font,
            "リーチ",
            AGARI_BTN_X + 8.0,
            riichi_y + RIICHI_BTN_H - 8.0,
            SMALL_FONT,
            WHITE,
        );
        if clicked
            && result.is_none()
            && hit_rect(mx, my, AGARI_BTN_X, riichi_y, RIICHI_BTN_W, RIICHI_BTN_H)
        {
            result = Some(OverlayClick::ToggleRiichi);
        }
    }

    // 暗カンボタン
    for (idx, tile) in state.self_kan_options.iter().enumerate() {
        let x = 720.0 + idx as f32 * 110.0;
        const KAN_BTN_W: f32 = 100.0;
        const KAN_BTN_H: f32 = 40.0;
        let kan_bg = Color::new(0.1, 0.3, 0.8, 1.0);
        draw_rectangle(x, 670.0, KAN_BTN_W, KAN_BTN_H, kan_bg);
        draw_rectangle_lines(x, 670.0, KAN_BTN_W, KAN_BTN_H, 2.0, WHITE);
        draw_jp_text(
            font,
            &format!("{tile}カン"),
            x + 10.0,
            697.0,
            SMALL_FONT,
            WHITE,
        );
        if clicked && result.is_none() && hit_rect(mx, my, x, 670.0, KAN_BTN_W, KAN_BTN_H) {
            result = Some(OverlayClick::Action(ClientAction::Kan {
                tile_index: tile.get() as usize,
            }));
        }
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

    result
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

fn draw_call_overlay(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
    clicked: bool,
    mx: f32,
    my: f32,
) -> Option<OverlayClick> {
    let has_ron = state
        .available_calls
        .iter()
        .any(|c| matches!(c, AvailableCall::Ron));
    let has_non_ron = state
        .available_calls
        .iter()
        .any(|c| !matches!(c, AvailableCall::Ron));

    if !has_non_ron {
        // ロンのみ：和了ボタンを右下に単独表示
        if has_ron {
            draw_agari_button(font, AGARI_BTN_X, AGARI_BTN_Y);
            if clicked && hit_rect(mx, my, AGARI_BTN_X, AGARI_BTN_Y, AGARI_BTN_W, AGARI_BTN_H) {
                return Some(OverlayClick::Action(ClientAction::Ron));
            }
        }
        return None;
    }

    let btn_w = CALL_BTN_W;
    let btn_h = CALL_BTN_H;
    let btn_spacing = CALL_BTN_SPACING;
    let tile_w = CALL_PANEL_TILE_W;
    let tile_h = CALL_PANEL_TILE_H;
    let tile_gap = 12.0_f32;

    let non_ron_call_count = state
        .available_calls
        .iter()
        .filter(|c| !matches!(c, AvailableCall::Ron))
        .count();
    let total_btn_count = non_ron_call_count + 1;
    let btns_w = total_btn_count as f32 * btn_w + (total_btn_count - 1) as f32 * btn_spacing;

    let pad = CALL_PANEL_PAD;
    let tile_area_w = tile_w + tile_gap;
    let panel_w = tile_area_w + btns_w + pad * 2.0;
    let panel_h = CALL_OVERLAY_PANEL_H;
    let panel_x = CALL_PANEL_RIGHT_X_NO_RON - panel_w;
    let panel_y = CALL_PANEL_BOTTOM_Y_NO_RON - panel_h;
    let base_x = panel_x + pad + tile_area_w;
    let base_y = CALL_BTN_BASE_Y_NO_RON;

    let mut result = None;

    // ロン＋鳴き同時：和了ボタンをパネルの上に表示
    if has_ron {
        let agari_y = panel_y - AGARI_BTN_GAP - AGARI_BTN_H;
        draw_agari_button(font, AGARI_BTN_X, agari_y);
        if clicked && hit_rect(mx, my, AGARI_BTN_X, agari_y, AGARI_BTN_W, AGARI_BTN_H) {
            result = Some(OverlayClick::Action(ClientAction::Ron));
        }
    }

    // パネル背景
    draw_rectangle(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        Color::new(0.0, 0.0, 0.0, 0.88),
    );
    draw_rectangle_lines(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        2.0,
        Color::new(1.0, 0.85, 0.3, 1.0),
    );

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
            AvailableCall::Pon { options } => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "ポン", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
                if clicked && result.is_none() && hit_rect(mx, my, x, base_y, btn_w, btn_h) {
                    result = if options.len() == 1 {
                        Some(OverlayClick::Action(ClientAction::Pon {
                            tiles: options[0],
                        }))
                    } else if !options.is_empty() {
                        Some(OverlayClick::ShowPonSelection {
                            options: options.clone(),
                        })
                    } else {
                        None
                    };
                }
            }
            AvailableCall::Daiminkan => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "カン", x + 18.0, base_y + 27.0, SMALL_FONT, WHITE);
                if clicked
                    && result.is_none()
                    && hit_rect(mx, my, x, base_y, btn_w, btn_h)
                    && let Some(tile) = state.call_target_tile
                {
                    result = Some(OverlayClick::Action(ClientAction::Kan {
                        tile_index: tile.get() as usize,
                    }));
                }
            }
            AvailableCall::Chi { options } => {
                draw_rectangle(x, base_y, btn_w, btn_h, call_btn_bg);
                draw_rectangle_lines(x, base_y, btn_w, btn_h, 2.0, WHITE);
                draw_jp_text(font, "チー", x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
                if clicked && result.is_none() && hit_rect(mx, my, x, base_y, btn_w, btn_h) {
                    result = if options.len() == 1 {
                        Some(OverlayClick::Action(ClientAction::Chi {
                            tiles: options[0],
                        }))
                    } else if !options.is_empty() {
                        Some(OverlayClick::ShowChiSelection {
                            options: options.clone(),
                        })
                    } else {
                        None
                    };
                }
            }
        }
        btn_idx += 1;
    }

    // パスボタン
    let pass_x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
    draw_rectangle(pass_x, base_y, btn_w, btn_h, pass_btn_bg);
    draw_rectangle_lines(pass_x, base_y, btn_w, btn_h, 2.0, WHITE);
    draw_jp_text(font, "パス", pass_x + 28.0, base_y + 27.0, FONT_SIZE, WHITE);
    if clicked && result.is_none() && hit_rect(mx, my, pass_x, base_y, btn_w, btn_h) {
        result = Some(OverlayClick::Action(ClientAction::Pass));
    }

    result
}

// ─── チー／ポン選択オーバーレイ ──────────────────────────────────────────────

fn draw_chi_selection_overlay(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
    clicked: bool,
    mx: f32,
    my: f32,
) -> Option<OverlayClick> {
    let called_tile = state.call_target_tile?;
    draw_meld_selection_overlay(
        MeldSelectionOverlay {
            font,
            tile_textures,
            title: "チーの組み合わせを選択",
            called_tile,
            options: &state.chi_pending_options,
            click: ClickState { clicked, mx, my },
        },
        |opt| ClientAction::Chi { tiles: opt },
    )
}

fn draw_pon_selection_overlay(
    state: &GameState,
    font: Option<&Font>,
    tile_textures: &TileTextures,
    clicked: bool,
    mx: f32,
    my: f32,
) -> Option<OverlayClick> {
    let called_tile = state.call_target_tile?;
    draw_meld_selection_overlay(
        MeldSelectionOverlay {
            font,
            tile_textures,
            title: "ポンの組み合わせを選択",
            called_tile,
            options: &state.pon_pending_options,
            click: ClickState { clicked, mx, my },
        },
        |opt| ClientAction::Pon { tiles: opt },
    )
}

/// チー／ポン選択オーバーレイの共通描画処理。描画しながらクリックを判定する。
struct ClickState {
    clicked: bool,
    mx: f32,
    my: f32,
}

struct MeldSelectionOverlay<'a> {
    font: Option<&'a Font>,
    tile_textures: &'a TileTextures,
    title: &'a str,
    called_tile: Tile,
    options: &'a [[Tile; 2]],
    click: ClickState,
}

fn draw_meld_selection_overlay(
    overlay: MeldSelectionOverlay<'_>,
    make_action: impl Fn([Tile; 2]) -> ClientAction,
) -> Option<OverlayClick> {
    let MeldSelectionOverlay {
        font,
        tile_textures,
        title,
        called_tile,
        options,
        click,
    } = overlay;
    let ClickState { clicked, mx, my } = click;

    let tile_w = CHI_SEL_TILE_W;
    let tile_h = CHI_SEL_TILE_H;
    let tile_gap = CHI_SEL_TILE_GAP;
    let opt_spacing = CHI_SEL_OPT_SPACING;
    let panel_h = CHI_SEL_PANEL_H;

    let opt_count = options.len();
    let opt_w = tile_w * 3.0 + tile_gap * 2.0;
    let panel_w = opt_w * opt_count as f32 + opt_spacing * (opt_count as f32 - 1.0) + 80.0;
    let panel_x = CALL_PANEL_RIGHT_X_NO_RON - panel_w;
    let panel_y = CALL_PANEL_BOTTOM_Y_NO_RON - panel_h;

    draw_rectangle(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        Color::new(0.0, 0.0, 0.0, 0.88),
    );
    draw_rectangle_lines(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        2.0,
        Color::new(1.0, 0.85, 0.3, 1.0),
    );

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

    let mut result = None;

    for (idx, &opt) in options.iter().enumerate() {
        let ox = opts_start_x + idx as f32 * (opt_w + opt_spacing);
        let hovered = hit_rect(mx, my, ox, opts_y, opt_w, tile_h);

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

        let mut display_tiles = [opt[0], opt[1], called_tile];
        display_tiles.sort();

        for (ti, tile) in display_tiles.iter().enumerate() {
            let tx = ox + ti as f32 * (tile_w + tile_gap);
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

        if clicked && result.is_none() && hovered {
            result = Some(OverlayClick::Action(make_action(opt)));
        }
    }

    // キャンセルボタン
    let cancel_w = 120.0_f32;
    let cancel_h = 36.0_f32;
    let cancel_x = panel_x + (panel_w - cancel_w) / 2.0;
    let cancel_y = panel_y + panel_h - cancel_h - 14.0;
    let cancel_hovered = hit_rect(mx, my, cancel_x, cancel_y, cancel_w, cancel_h);
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

    if clicked && result.is_none() && cancel_hovered {
        result = Some(OverlayClick::CancelMeldSelection);
    }

    result
}

// ─── 九種九牌オーバーレイ ──────────────────────────────────────────────────────

fn draw_nine_terminals_overlay(
    font: Option<&Font>,
    clicked: bool,
    mx: f32,
    my: f32,
) -> Option<OverlayClick> {
    const PANEL_W: f32 = 360.0;
    const PANEL_H: f32 = 140.0;
    const PANEL_X: f32 = CALL_PANEL_RIGHT_X_NO_RON - PANEL_W;
    const PANEL_Y: f32 = CALL_PANEL_BOTTOM_Y_NO_RON - PANEL_H;

    draw_rectangle(
        PANEL_X,
        PANEL_Y,
        PANEL_W,
        PANEL_H,
        Color::new(0.0, 0.0, 0.0, 0.90),
    );
    draw_rectangle_lines(
        PANEL_X,
        PANEL_Y,
        PANEL_W,
        PANEL_H,
        2.0,
        Color::new(1.0, 0.85, 0.3, 1.0),
    );

    draw_jp_text(
        font,
        "九種九牌",
        PANEL_X + 16.0,
        PANEL_Y + 30.0,
        FONT_SIZE,
        Color::new(1.0, 0.95, 0.5, 1.0),
    );
    draw_jp_text(
        font,
        "流局しますか？",
        PANEL_X + 16.0,
        PANEL_Y + 54.0,
        FONT_SIZE,
        WHITE,
    );

    const BTN_W: f32 = 140.0;
    const BTN_H: f32 = 38.0;
    const BTN_Y: f32 = PANEL_Y + PANEL_H - BTN_H - 14.0;
    const BTN_GAP: f32 = 12.0;
    let declare_x = PANEL_X + 16.0;
    let pass_x = declare_x + BTN_W + BTN_GAP;

    let declare_hovered = hit_rect(mx, my, declare_x, BTN_Y, BTN_W, BTN_H);
    let declare_bg = if declare_hovered {
        Color::new(0.9, 0.15, 0.15, 1.0)
    } else {
        Color::new(0.7, 0.1, 0.1, 1.0)
    };
    draw_rectangle(declare_x, BTN_Y, BTN_W, BTN_H, declare_bg);
    draw_rectangle_lines(declare_x, BTN_Y, BTN_W, BTN_H, 2.0, WHITE);
    draw_jp_text(
        font,
        "流局する",
        declare_x + 22.0,
        BTN_Y + 26.0,
        FONT_SIZE,
        WHITE,
    );

    let pass_hovered = hit_rect(mx, my, pass_x, BTN_Y, BTN_W, BTN_H);
    let pass_bg = if pass_hovered {
        Color::new(0.4, 0.4, 0.4, 1.0)
    } else {
        Color::new(0.25, 0.25, 0.25, 1.0)
    };
    draw_rectangle(pass_x, BTN_Y, BTN_W, BTN_H, pass_bg);
    draw_rectangle_lines(pass_x, BTN_Y, BTN_W, BTN_H, 2.0, WHITE);
    draw_jp_text(
        font,
        "続ける",
        pass_x + 26.0,
        BTN_Y + 26.0,
        FONT_SIZE,
        WHITE,
    );

    if clicked {
        if declare_hovered {
            return Some(OverlayClick::NineTerminalsDeclare);
        }
        if pass_hovered {
            return Some(OverlayClick::NineTerminalsPass);
        }
    }

    None
}
