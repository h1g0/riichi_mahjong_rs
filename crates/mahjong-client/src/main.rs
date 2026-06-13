//! 麻雀クライアント（Macroquad）
//!
//! ブラウザ上で動作する4人打ち日本式リーチ麻雀。
//! GameAdapterを通してサーバとやり取りする
//! （ローカル対戦はLocalAdapter、オンライン対戦はRemoteAdapter）。

use macroquad::prelude::*;

mod adapter;
mod game;
mod renderer;
mod transport;

// WASM用カスタム乱数バックエンド（wasm-bindgen不要）
#[cfg(target_arch = "wasm32")]
mod wasm_rng;

use adapter::{ConnStatus, GameAdapter, LocalAdapter, RemoteAdapter, RoomView, error_code_message};
use game::{GamePhase, GameState, RoomViewUi};
use mahjong_server::protocol::net::SeatInfo;
use renderer::{OnlineLobbyAction, OnlineMenuAction, SetupAction, TileTextures};

fn window_conf() -> Conf {
    Conf {
        window_title: "麻雀".to_owned(),
        window_width: 1280,
        window_height: 800,
        ..Default::default()
    }
}

/// 入力された表示名を整形する（空なら既定値）
fn display_name(state: &GameState) -> String {
    let name = state.online_state.name_input.trim();
    if name.is_empty() {
        "プレイヤー".to_string()
    } else {
        name.to_string()
    }
}

/// ステータス行を設定する
fn set_status(state: &mut GameState, message: &str, is_error: bool) {
    state.online_state.status_line = Some(message.to_string());
    state.online_state.status_is_error = is_error;
}

/// ロビー画面用の座席表示文言を組み立てる
fn build_seat_labels(room: &RoomView) -> [String; 4] {
    let winds = ["東", "南", "西", "北"];
    std::array::from_fn(|i| {
        let who = match &room.seats[i] {
            SeatInfo::Empty => "空席".to_string(),
            SeatInfo::Cpu => "CPU".to_string(),
            SeatInfo::Human { name, connected } => {
                if *connected {
                    name.clone()
                } else {
                    format!("{name}（切断中）")
                }
            }
        };
        let mut marks = String::new();
        if i == room.your_seat {
            marks.push_str("（あなた）");
        }
        if i == room.host_seat {
            marks.push_str("（ホスト）");
        }
        format!("{}: {}{}", winds[i], who, marks)
    })
}

/// リモートアダプターの状態をUI表示用にコピーする
fn sync_online_ui(remote: &mut RemoteAdapter, state: &mut GameState) {
    state.online_state.room = remote.room().map(|room| RoomViewUi {
        code: room.code.clone(),
        seat_labels: build_seat_labels(room),
        is_host: room.is_host(),
    });

    if let Some(err) = remote.take_error() {
        let message = match err.code {
            Some(code) => error_code_message(code).to_string(),
            None => err.message,
        };
        set_status(state, &message, true);
        return;
    }

    // エラー表示中は接続ステータスで上書きしない
    if state.online_state.status_is_error {
        return;
    }

    match remote.status() {
        ConnStatus::Connecting => set_status(state, "サーバに接続中...", false),
        ConnStatus::Connected => {
            state.online_state.status_line = None;
        }
        ConnStatus::Disconnected => set_status(state, "サーバとの接続が切れました", true),
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

    // 対局中のアダプター（ローカル or リモート）
    let mut adapter: Option<Box<dyn GameAdapter>> = None;
    // ロビー段階のリモート接続（対局開始時に adapter へ引き継ぐ）
    let mut online: Option<RemoteAdapter> = None;
    let mut game_state = GameState::new();

    loop {
        clear_background(Color::from_rgba(0, 100, 0, 255));

        let overlay_click = renderer::draw_game(&game_state, font.as_ref(), &tile_textures);

        // ロビー段階のリモート接続を進める
        if let Some(remote) = &mut online {
            remote.tick();
            sync_online_ui(remote, &mut game_state);

            if remote.game_started() {
                // 対局開始: ゲームアダプターとして引き継ぐ
                game_state.online_state.status_line = None;
                game_state.online_state.status_is_error = false;
                adapter = Some(Box::new(online.take().expect("checked above")));
            }
        }

        // アダプターを毎フレーム進め、イベントを反映する
        if let Some(adp) = &mut adapter {
            adp.tick();
            for event in adp.poll_events() {
                game_state.handle_event(event);
            }
            // 対局中の接続バナー（ローカル対戦では常に None）
            game_state.online_state.status_line = adp.status_text();
            game_state.online_state.status_is_error = true;
        }

        match game_state.phase {
            GamePhase::Setup => {
                // 設定画面の入力処理
                if let Some(action) = renderer::handle_setup_input(&mut game_state, font.as_ref()) {
                    match action {
                        SetupAction::StartLocal(configs) => {
                            // ローカル対局開始
                            let mut new_adapter = LocalAdapter::with_cpu_configs(configs);
                            new_adapter.start_game();
                            let events = new_adapter.poll_events();
                            for event in events {
                                game_state.handle_event(event);
                            }
                            adapter = Some(Box::new(new_adapter));
                        }
                        SetupAction::GoOnline => {
                            game_state.online_state.status_line = None;
                            game_state.online_state.status_is_error = false;
                            game_state.online_state.room = None;
                            game_state.phase = GamePhase::OnlineMenu;
                        }
                    }
                }
            }

            GamePhase::OnlineMenu => {
                if let Some(action) = renderer::handle_online_menu_input(&mut game_state) {
                    match action {
                        OnlineMenuAction::CreateRoom => {
                            let url = transport::default_server_url();
                            let name = display_name(&game_state);
                            online = Some(RemoteAdapter::create_room(&url, &name, 1));
                            set_status(&mut game_state, "サーバに接続中...", false);
                        }
                        OnlineMenuAction::JoinRoom => {
                            let code = game_state.online_state.code_input.clone();
                            if code.chars().count() != 6 {
                                set_status(
                                    &mut game_state,
                                    "ルームコードを6文字で入力してください",
                                    true,
                                );
                            } else {
                                let url = transport::default_server_url();
                                let name = display_name(&game_state);
                                online = Some(RemoteAdapter::join_room(&url, &name, &code));
                                set_status(&mut game_state, "サーバに接続中...", false);
                            }
                        }
                        OnlineMenuAction::Back => {
                            online = None;
                            game_state.online_state.status_line = None;
                            game_state.online_state.status_is_error = false;
                            game_state.phase = GamePhase::Setup;
                        }
                    }
                }

                // 入室できたらロビーへ
                if game_state.online_state.room.is_some() {
                    game_state.online_state.status_line = None;
                    game_state.online_state.status_is_error = false;
                    game_state.phase = GamePhase::OnlineLobby;
                }
            }

            GamePhase::OnlineLobby => {
                if let Some(action) = renderer::handle_online_lobby_input(&game_state) {
                    match action {
                        OnlineLobbyAction::StartGame => {
                            if let Some(remote) = &mut online {
                                remote.start_game();
                            }
                        }
                        OnlineLobbyAction::Leave => {
                            if let Some(remote) = &mut online {
                                remote.leave_room();
                            }
                            online = None;
                            game_state.online_state.room = None;
                            game_state.online_state.status_line = None;
                            game_state.online_state.status_is_error = false;
                            game_state.phase = GamePhase::OnlineMenu;
                        }
                    }
                }
            }

            GamePhase::Playing => {
                if let Some(ref mut adp) = adapter {
                    let action = game_state.handle_input(overlay_click);
                    if let Some(act) = action {
                        adp.send_action(act);
                    }
                }
            }

            GamePhase::RoundResult => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    // まだ表示していない和了者がいれば次のページへ、なければ次の局へ
                    if !game_state.advance_win_result()
                        && let Some(ref mut adp) = adapter
                    {
                        if adp.is_game_over() {
                            game_state.phase = GamePhase::GameOver;
                        } else {
                            adp.request_next_round();
                        }
                    }
                }
            }

            GamePhase::GameOver => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    // 設定画面に戻る
                    game_state = GameState::new();
                    adapter = None;
                    online = None;
                }
            }

            GamePhase::WaitingForStart => {}
        }

        next_frame().await;
    }
}
