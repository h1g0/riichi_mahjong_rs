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

    let mut adapter = LocalAdapter::new();
    let mut game_state = GameState::new();

    adapter.start_game();

    let events = adapter.poll_events(0);
    for event in events {
        game_state.handle_event(event);
    }

    loop {
        clear_background(Color::from_rgba(0, 100, 0, 255));

        match game_state.phase {
            GamePhase::Playing => {
                let action = game_state.handle_input();
                if let Some(act) = action {
                    adapter.send_action(act);
                }

                adapter.tick();

                let events = adapter.poll_events(0);
                for event in events {
                    game_state.handle_event(event);
                }
            }

            GamePhase::RoundResult => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    if adapter.is_game_over() {
                        game_state.phase = GamePhase::GameOver;
                    } else {
                        adapter.next_round();

                        let events = adapter.poll_events(0);
                        for event in events {
                            game_state.handle_event(event);
                        }
                    }
                }
            }

            GamePhase::GameOver => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    adapter = LocalAdapter::new();
                    game_state = GameState::new();
                    adapter.start_game();

                    let events = adapter.poll_events(0);
                    for event in events {
                        game_state.handle_event(event);
                    }
                }
            }

            GamePhase::WaitingForStart => {}
        }

        renderer::draw_game(&game_state, font.as_ref(), &tile_textures);

        next_frame().await;
    }
}
