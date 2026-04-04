//! 麻雀クライアント（Macroquad）
//!
//! ブラウザ上で動作する4人打ち日本式リーチ麻雀。
//! LocalAdapterを通してサーバと直接通信する。

use macroquad::prelude::*;

mod adapter;
mod game;
mod renderer;

// WASM用カスタム乱数バックエンド（wasm-bindgen不要）
#[cfg(target_arch = "wasm32")]
mod wasm_rng;

use adapter::LocalAdapter;
use game::{GamePhase, GameState};
use renderer::TileTextures;

fn window_conf() -> Conf {
    Conf {
        window_title: "麻雀".to_owned(),
        window_width: 1280,
        window_height: 800,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let font_bytes: &[u8] = include_bytes!("../../../assets/fonts/NotoSansJP-Regular.ttf");
    let font = load_ttf_font_from_bytes(font_bytes).ok();
    let tile_textures = TileTextures::load();

    if font.is_none() {
        eprintln!("警告: 日本語フォントを読み込めませんでした。デフォルトフォントで表示します。");
    }

    let mut adapter: Option<LocalAdapter> = None;
    let mut game_state = GameState::new();

    loop {
        clear_background(Color::from_rgba(0, 100, 0, 255));

        let overlay_click = renderer::draw_game(&game_state, font.as_ref(), &tile_textures);

        match game_state.phase {
            GamePhase::Setup => {
                // 設定画面の入力処理
                if let Some(configs) = renderer::handle_setup_input(&mut game_state, font.as_ref()) {
                    // 対局開始
                    let mut new_adapter = LocalAdapter::with_cpu_configs(configs);
                    new_adapter.start_game();
                    let events = new_adapter.poll_events(0);
                    for event in events {
                        game_state.handle_event(event);
                    }
                    adapter = Some(new_adapter);
                }
            }

            GamePhase::Playing => {
                if let Some(ref mut adp) = adapter {
                    let action = game_state.handle_input(overlay_click);
                    if let Some(act) = action {
                        adp.send_action(act);
                    }

                    adp.tick();

                    let events = adp.poll_events(0);
                    for event in events {
                        game_state.handle_event(event);
                    }
                }
            }

            GamePhase::RoundResult => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    // まだ表示していない和了者がいれば次のページへ、なければ次の局へ
                    if !game_state.advance_win_result() {
                        if let Some(ref mut adp) = adapter {
                            if adp.is_game_over() {
                                game_state.phase = GamePhase::GameOver;
                            } else {
                                adp.next_round();
                                let events = adp.poll_events(0);
                                for event in events {
                                    game_state.handle_event(event);
                                }
                            }
                        }
                    }
                }
            }

            GamePhase::GameOver => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    // 設定画面に戻る
                    game_state = GameState::new();
                    adapter = None;
                }
            }

            GamePhase::WaitingForStart => {}
        }

        next_frame().await;
    }
}
