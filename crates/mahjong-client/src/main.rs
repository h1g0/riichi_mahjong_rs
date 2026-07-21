//! Mahjong client (Macroquad).
//!
//! Japanese riichi mahjong playable in the browser. Talks to the server
//! through GameAdapter: LocalAdapter for local play, RemoteAdapter for
//! online play.

use macroquad::miniquad::conf::Icon;
use macroquad::prelude::*;

mod adapter;
mod game;
mod i18n;
mod loading;
mod persistence;
mod renderer;
#[cfg(not(target_arch = "wasm32"))]
mod screenshot;
mod transport;

// Custom WASM randomness backend (no wasm-bindgen).
#[cfg(target_arch = "wasm32")]
mod wasm_rng;

use adapter::{ConnStatus, GameAdapter, LocalAdapter, RemoteAdapter, RoomView, error_code_message};
use game::{GameMode, GamePhase, GameState, MenuOrigin, PlayerLabel, RoomViewUi};
use mahjong_core::settings::Lang;
use mahjong_server::protocol::net::SeatInfo;
use renderer::{
    ModeSelectAction, OnlineLobbyAction, OnlineMenuAction, RuleSettingsAction, SetupAction,
    TileTextures, TopMenuAction,
};

#[cfg(not(target_arch = "wasm32"))]
struct ScreenshotNotice {
    message: String,
    is_error: bool,
    expires_at: f64,
}

#[cfg(not(target_arch = "wasm32"))]
impl ScreenshotNotice {
    fn new(message: String, is_error: bool, now: f64) -> Self {
        Self {
            message,
            is_error,
            expires_at: now + 3.0,
        }
    }
}

/// Window/taskbar icon: 16/32/64px RGBA decoded from embedded PNGs.
fn app_icon() -> Icon {
    fn rgba(png: &[u8]) -> Vec<u8> {
        Image::from_file_with_format(png, Some(ImageFormat::Png))
            .expect("組み込みアイコンPNGのデコードに失敗")
            .bytes
    }

    Icon {
        small: rgba(include_bytes!("../../../assets/images/others/icon-16.png"))
            .try_into()
            .expect("16x16アイコンのサイズが不正"),
        medium: rgba(include_bytes!("../../../assets/images/others/icon-32.png"))
            .try_into()
            .expect("32x32アイコンのサイズが不正"),
        big: rgba(include_bytes!("../../../assets/images/others/icon-64.png"))
            .try_into()
            .expect("64x64アイコンのサイズが不正"),
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Riichi Mahjong RS".to_owned(),
        window_width: 1280,
        window_height: 800,
        icon: Some(app_icon()),
        ..Default::default()
    }
}

/// Normalizes the entered display name; empty falls back to the default.
fn display_name(state: &GameState) -> String {
    let name = state.online_state.name_input.trim();
    if name.is_empty() {
        state.tr().get(i18n::Key::DefaultPlayerName).to_string()
    } else {
        name.to_string()
    }
}

/// Sets the status line.
fn set_status(state: &mut GameState, message: &str, is_error: bool) {
    state.online_state.status_line = Some(message.to_string());
    state.online_state.status_is_error = is_error;
}

/// Builds the lobby's per-seat captions.
fn build_seat_labels(room: &RoomView, lang: Lang) -> [String; 4] {
    let tr = i18n::Translator::new(lang);
    std::array::from_fn(|i| {
        // Hide the unused seat 3 in three-player rooms.
        if i >= room.player_count() {
            return String::new();
        }
        let who = match &room.seats[i] {
            SeatInfo::Empty => match room.cpu_configs {
                // Attach the CPU config that would fill the seat, using
                // the server's config_for_seat rule (seats 1-3 map to
                // configs[0..3]).
                Some(configs) => {
                    let spec = configs[i.saturating_sub(1).min(2)];
                    tr.empty_seat_cpu_label(spec.level, spec.personality)
                }
                None => tr.get(i18n::Key::EmptySeat).to_string(),
            },
            SeatInfo::Cpu { level, personality } => tr.cpu_seat_label(*level, *personality),
            SeatInfo::Human { name, connected } => {
                if *connected {
                    name.clone()
                } else {
                    tr.disconnected_name(name)
                }
            }
        };
        let mut marks = String::new();
        if i == room.your_seat {
            marks.push_str(tr.get(i18n::Key::MarkerYou));
        }
        if i == room.host_seat {
            marks.push_str(tr.get(i18n::Key::MarkerHost));
        }
        tr.seat_row(i + 1, &who, &marks)
    })
}

/// Builds the per-seat player types from the room info.
fn build_online_player_labels(room: &RoomView) -> [PlayerLabel; 4] {
    std::array::from_fn(|s| {
        if s == room.your_seat {
            PlayerLabel::Me
        } else {
            match &room.seats[s] {
                SeatInfo::Human { name, .. } => PlayerLabel::Human(name.clone()),
                SeatInfo::Cpu { level, personality } => PlayerLabel::Cpu {
                    level: level.display_name().to_string(),
                    personality: personality.display_name().to_string(),
                },
                SeatInfo::Empty => PlayerLabel::Human("—".to_string()),
            }
        }
    })
}

/// Copies the remote adapter's state for UI display.
fn sync_online_ui(remote: &mut RemoteAdapter, state: &mut GameState) {
    let lang = state.lang;
    state.online_state.room = remote.room().map(|room| RoomViewUi {
        code: room.code.clone(),
        seat_labels: build_seat_labels(room, lang),
        is_host: room.is_host(),
        mode: GameMode::from_parts(room.three_player(), room.length),
    });

    if let Some(err) = remote.take_error() {
        let message = match err.code {
            Some(code) => error_code_message(code, lang).to_string(),
            None => {
                // Transport errors: log the detail, show a generic message.
                macroquad::logging::warn!("network error: {}", err.message);
                i18n::Key::NetworkError.text(lang).to_string()
            }
        };
        set_status(state, &message, true);
        return;
    }

    // Never overwrite a visible error with a mere status update.
    if state.online_state.status_is_error {
        return;
    }

    match remote.status() {
        ConnStatus::Connecting => {
            let msg = i18n::Key::Connecting.text(lang);
            set_status(state, msg, false);
        }
        ConnStatus::Connected => {
            state.online_state.status_line = None;
        }
        ConnStatus::Disconnected => {
            let msg = i18n::Key::Disconnected.text(lang);
            set_status(state, msg, true);
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    #[cfg(target_arch = "wasm32")]
    let loading_steps = 1 + renderer::TEXTURE_LOAD_STEPS;
    #[cfg(not(target_arch = "wasm32"))]
    let loading_steps = 1 + renderer::TEXTURE_LOAD_STEPS + renderer::FONT_PREWARM_STEPS;
    let mut loading_screen = loading::LoadingScreen::new(loading_steps);
    loading_screen.next_frame().await;

    let font_bytes: &[u8] = include_bytes!("../../../assets/fonts/ShipporiMincho-Regular.ttf");
    let font = load_ttf_font_from_bytes(font_bytes).ok();
    loading_screen.complete_step().await;
    let tile_textures = TileTextures::load(font_bytes, &mut loading_screen).await;

    if font.is_none() {
        eprintln!("警告: 日本語フォントを読み込めませんでした。デフォルトフォントで表示します。");
    }

    // Build the font atlas up front on native, where lazily cached glyphs
    // used to render as black squares in-game (see
    // renderer::prewarm_fonts).
    //
    // Never call this on WASM: caching many glyph/size combinations at
    // once makes macroquad 0.4.15's font atlas grow repeatedly, and on
    // wasm32 (32-bit usize) `Image::gen_image_color`'s
    // `width * height * 4` hits exactly 2^32 at 32768x32768 and
    // overflows, freezing on a black screen at startup (a macroquad bug).
    // Skipping the prewarm lets glyphs cache one at a time as they
    // appear, avoiding the mass repacking.
    #[cfg(not(target_arch = "wasm32"))]
    renderer::prewarm_fonts(font.as_ref(), &mut loading_screen).await;

    // The in-game adapter (local or remote).
    let mut adapter: Option<Box<dyn GameAdapter>> = None;
    // The lobby-stage remote connection, handed to `adapter` at
    // game start.
    let mut online: Option<RemoteAdapter> = None;
    let mut game_state = GameState::new();
    #[cfg(not(target_arch = "wasm32"))]
    let mut screenshot_notice: Option<ScreenshotNotice> = None;

    loop {
        #[cfg(not(target_arch = "wasm32"))]
        let screenshot_requested = is_key_pressed(KeyCode::F12);

        clear_background(Color::from_rgba(6, 14, 9, 255));

        // Scale the design coordinate system (DESIGN_W x DESIGN_H) to
        // the actual canvas.
        renderer::set_design_camera();

        // Cache glyphs of dynamic text (opponent names etc.) before
        // drawing.
        renderer::cache_dynamic_text(font.as_ref(), &game_state);

        #[cfg(not(target_arch = "wasm32"))]
        {
            if screenshot_notice
                .as_ref()
                .is_some_and(|notice| notice.expires_at <= get_time())
            {
                screenshot_notice = None;
            }
            if let Some(notice) = &screenshot_notice {
                renderer::cache_notification_text(font.as_ref(), &notice.message);
            }
        }

        let overlay_click = renderer::draw_game(&game_state, font.as_ref(), &tile_textures);

        #[cfg(not(target_arch = "wasm32"))]
        if !screenshot_requested && let Some(notice) = &screenshot_notice {
            renderer::draw_notification(font.as_ref(), &notice.message, notice.is_error);
        }

        if let Some(remote) = &mut online {
            remote.tick();
            sync_online_ui(remote, &mut game_state);

            if remote.game_started() {
                // Game start: promote to the in-game adapter.
                game_state.online_state.status_line = None;
                game_state.online_state.status_is_error = false;

                if let Some(room) = remote.room() {
                    let labels = build_online_player_labels(room);
                    let your_seat = room.your_seat;
                    game_state.set_online_players(&labels, your_seat);
                }
                adapter = Some(Box::new(online.take().expect("checked above")));
            }
        }

        // Advance the adapter every frame and apply its events;
        // process_events holds later events while a declaration banner
        // is showing.
        if let Some(adp) = &mut adapter {
            adp.tick();
            for event in adp.poll_events() {
                game_state.queue_event(event);
            }
            game_state.process_events(get_time());
            // The in-game connection banner (always None locally).
            game_state.online_state.status_line = adp.status_text(game_state.lang);
            game_state.online_state.status_is_error = game_state.online_state.status_line.is_some();
            // Turn timer (online only).
            game_state.online_state.turn_remaining = adp.turn_remaining_secs();
        }

        match game_state.phase {
            GamePhase::TopMenu => {
                if let Some(action) = renderer::handle_top_menu_input(&mut game_state) {
                    match action {
                        TopMenuAction::CpuBattle => {
                            game_state.phase = GamePhase::ModeSelect(MenuOrigin::Local);
                        }
                        TopMenuAction::Online => {
                            game_state.online_state.status_line = None;
                            game_state.online_state.status_is_error = false;
                            game_state.online_state.room = None;
                            game_state.phase = GamePhase::OnlineMenu;
                        }
                    }
                }
            }

            GamePhase::ModeSelect(origin) => {
                if let Some(action) = renderer::handle_mode_select_input(&mut game_state, origin) {
                    match (action, origin) {
                        (ModeSelectAction::ModeChosen(_), MenuOrigin::Local) => {
                            game_state.phase = GamePhase::CpuSetup(MenuOrigin::Local);
                        }
                        (ModeSelectAction::ModeChosen(_), MenuOrigin::Online) => {
                            let url = transport::default_server_url();
                            let name = display_name(&game_state);
                            let rules = game_state.online_state.build_rules();
                            let length = game_state.online_state.length();
                            online = Some(RemoteAdapter::create_room(&url, &name, length, rules));
                            let msg = i18n::Key::Connecting.text(game_state.lang);
                            set_status(&mut game_state, msg, false);
                            // The online menu handles status display
                            // and the join wait.
                            game_state.phase = GamePhase::OnlineMenu;
                        }
                        (ModeSelectAction::OpenRuleSettings, _) => {
                            game_state.phase = GamePhase::RuleSettings(origin);
                        }
                        (ModeSelectAction::Back, MenuOrigin::Local) => {
                            game_state.phase = GamePhase::TopMenu;
                        }
                        (ModeSelectAction::Back, MenuOrigin::Online) => {
                            game_state.phase = GamePhase::OnlineMenu;
                        }
                    }
                }
            }

            GamePhase::RuleSettings(origin) => {
                if let Some(RuleSettingsAction::Confirm) =
                    renderer::handle_rule_settings_input(&mut game_state, origin)
                {
                    game_state.phase = GamePhase::ModeSelect(origin);
                }
            }

            GamePhase::CpuSetup(origin) => {
                if let Some(action) =
                    renderer::handle_setup_input(&mut game_state, font.as_ref(), origin)
                {
                    match action {
                        SetupAction::StartLocal(mut configs) => {
                            let settings = game_state.setup_state.build_game_settings();
                            // Shuffle the participating CPUs' seats
                            // (GameDriver::start_game randomizes the
                            // dealer). Must happen before
                            // set_local_players so labels stay aligned
                            // with seats.
                            let cpu_count = settings.rules.player_count() - 1;
                            mahjong_server::cpu::client::shuffle_cpu_configs(
                                &mut configs[..cpu_count],
                            );
                            game_state.set_local_players(&configs);
                            let mut new_adapter = LocalAdapter::with_settings(settings, configs);
                            new_adapter.start_game();
                            let events = new_adapter.poll_events();
                            for event in events {
                                game_state.queue_event(event);
                            }
                            game_state.process_events(get_time());
                            adapter = Some(Box::new(new_adapter));
                        }
                        SetupAction::ApplyOnline => {
                            // Send the CPU configs so everyone's lobby
                            // reflects them.
                            if let Some(remote) = &mut online {
                                let specs = game_state.setup_state.build_cpu_specs();
                                remote.set_cpu_configs(specs);
                            }
                            game_state.phase = GamePhase::OnlineLobby;
                        }
                        SetupAction::Back => {
                            game_state.phase = match origin {
                                MenuOrigin::Local => GamePhase::ModeSelect(MenuOrigin::Local),
                                MenuOrigin::Online => GamePhase::OnlineLobby,
                            };
                        }
                    }
                }
            }

            GamePhase::OnlineMenu => {
                if let Some(action) = renderer::handle_online_menu_input(&mut game_state) {
                    match action {
                        OnlineMenuAction::CreateRoom => {
                            // Pick the game mode before creating the room.
                            game_state.phase = GamePhase::ModeSelect(MenuOrigin::Online);
                        }
                        OnlineMenuAction::JoinRoom => {
                            let code = game_state.online_state.code_input.clone();
                            if code.chars().count() != 6 {
                                let msg = i18n::Key::RoomCodeLengthError.text(game_state.lang);
                                set_status(&mut game_state, msg, true);
                            } else {
                                let url = transport::default_server_url();
                                let name = display_name(&game_state);
                                online = Some(RemoteAdapter::join_room(&url, &name, &code));
                                let msg = i18n::Key::Connecting.text(game_state.lang);
                                set_status(&mut game_state, msg, false);
                            }
                        }
                        OnlineMenuAction::Back => {
                            online = None;
                            game_state.online_state.status_line = None;
                            game_state.online_state.status_is_error = false;
                            game_state.phase = GamePhase::TopMenu;
                        }
                    }
                }

                if game_state.online_state.room.is_some() {
                    game_state.online_state.status_line = None;
                    game_state.online_state.status_is_error = false;
                    game_state.phase = GamePhase::OnlineLobby;
                }
            }

            GamePhase::OnlineLobby => {
                if let Some(action) = renderer::handle_online_lobby_input(&game_state) {
                    match action {
                        OnlineLobbyAction::OpenCpuSettings => {
                            // Match the CPU count to the room's mode
                            // before opening.
                            if let Some(room) = &game_state.online_state.room {
                                game_state.setup_state.mode = room.mode;
                            }
                            game_state.phase = GamePhase::CpuSetup(MenuOrigin::Online);
                        }
                        OnlineLobbyAction::StartGame => {
                            if let Some(remote) = &mut online {
                                // Send the host's chosen CPU configs.
                                let specs = game_state.setup_state.build_cpu_specs();
                                remote.start_game(Some(specs));
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
                    let action = game_state.handle_input(overlay_click, get_time());
                    if let Some(act) = action {
                        adp.send_action(act);
                    }
                }
            }

            GamePhase::RoundResult => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    // Show the next winner's page, or move to the
                    // next hand.
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
                    game_state = GameState::new();
                    adapter = None;
                    online = None;
                }
            }

            GamePhase::WaitingForStart => {}
        }

        #[cfg(not(target_arch = "wasm32"))]
        if screenshot_requested {
            let now = get_time();
            screenshot_notice = Some(match screenshot::capture() {
                Ok(path) => {
                    let path = path.to_string_lossy();
                    macroquad::logging::info!("screenshot saved: {}", path);
                    ScreenshotNotice::new(game_state.tr().screenshot_saved(&path), false, now)
                }
                Err(error) => {
                    macroquad::logging::warn!("failed to save screenshot: {}", error);
                    ScreenshotNotice::new(
                        game_state.tr().get(i18n::Key::ScreenshotFailed).to_string(),
                        true,
                        now,
                    )
                }
            });
        }

        next_frame().await;
    }
}
