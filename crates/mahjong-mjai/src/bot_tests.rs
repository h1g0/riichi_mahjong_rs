//! End-to-end tests: the CPU opponent playing a real game over the mjai
//! protocol.

use mahjong_core::settings::Settings;
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::driver::GameDriver;
use mahjong_server::protocol::ServerEvent;
use mahjong_server::table::GameSettings;

use crate::bot::MjaiBot;
use crate::event::MjaiEvent;
use crate::host::MjaiHost;

fn names() -> Vec<String> {
    (0..4).map(|seat| format!("p{seat}")).collect()
}

/// What one hosted game produced, for the assertions below to pick over.
struct Played {
    /// Every mjai event the bot was sent.
    sent: Vec<MjaiEvent>,
    /// Every declaration the bot made that was not a pass.
    declared: Vec<MjaiEvent>,
    /// Seat 0's server events, as the game actually ran.
    server_events: Vec<ServerEvent>,
    hands: usize,
    /// Whether the game reached its own end rather than the loop's step cap.
    /// This is the real "the bot did not stall" signal: a bot that stops
    /// answering leaves the driver waiting forever.
    completed: bool,
}

/// Runs a full game with seat 0 played by an mjai bot over the protocol.
///
/// Nothing here reaches into the bot: it only ever sees mjai events, and only
/// ever answers with mjai events, exactly as a foreign bot would.
fn play_with_bot(level: CpuLevel) -> Played {
    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 1..4 {
        driver.set_cpu(
            seat,
            CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        );
    }
    driver.start_game_with_seed(42);

    let mut host = MjaiHost::new(names());
    let mut bot = MjaiBot::new(
        "bot",
        CpuConfig::new(level, CpuPersonality::Balanced),
        Settings::default(),
    );

    let mut played = Played {
        sent: Vec::new(),
        declared: Vec::new(),
        server_events: Vec::new(),
        hands: 0,
        completed: false,
    };

    // Handshake, then tell the bot which seat it holds.
    let hello = MjaiBot::hello();
    assert!(matches!(bot.respond(&hello), MjaiEvent::Join { .. }));

    let mut announced_seat = false;

    for _ in 0..20_000 {
        driver.run_until_blocked();

        for server_event in driver.drain_events(0) {
            played.server_events.push(server_event.clone());
            if matches!(server_event, ServerEvent::GameStarted { .. }) {
                played.hands += 1;
            }

            // mjai cannot put every server prompt to a player; whatever it
            // cannot ask still has to be answered or the hand stops.
            if let Some(action) = host.unrepresentable_prompt(&server_event) {
                driver.handle_action(0, action);
            }

            for event in host.encode(&server_event) {
                if !announced_seat && let MjaiEvent::StartGame { .. } = &event {
                    bot.respond(&event);
                    announced_seat = true;
                    continue;
                }
                played.sent.push(event.clone());
                let response = bot.respond(&event);
                if matches!(response, MjaiEvent::Pass) && !is_prompt(&event) {
                    continue;
                }
                if !matches!(response, MjaiEvent::Pass) {
                    played.declared.push(response.clone());
                }

                // A riichi is declared first and discarded next, so the
                // declaration goes back to the bot before anything is sent to
                // the server.
                if matches!(response, MjaiEvent::Reach { .. }) {
                    host.to_client_action(&response);
                    let discard = bot.respond(&response);
                    played.declared.push(discard.clone());
                    if let Some(action) = host.to_client_action(&discard) {
                        driver.handle_action(0, action);
                    }
                    continue;
                }

                if let Some(action) = host.to_client_action(&response) {
                    driver.handle_action(0, action);
                }
            }
        }

        if driver.is_round_over() {
            if driver.is_game_over() {
                played.completed = true;
                break;
            }
            driver.next_round();
        } else {
            driver.tick();
        }
    }
    played
}

/// Whether an event asks the bot for a decision.
fn is_prompt(event: &MjaiEvent) -> bool {
    matches!(event, MjaiEvent::Tsumo { .. } | MjaiEvent::Dahai { .. })
}

#[test]
fn the_bot_plays_a_whole_game_over_the_protocol() {
    let played = play_with_bot(CpuLevel::Strong);

    // A game can legitimately end early on a bankruptcy, so the number of
    // hands is not the thing to assert. Reaching the end at all is.
    assert!(
        played.completed,
        "the game never finished; the bot stalled it"
    );
    assert!(played.hands >= 1);
    assert!(
        played.sent.len() > 100,
        "the bot barely saw the game: {} events",
        played.sent.len()
    );

    // The game has to have actually progressed, not stalled on a seat that
    // never answered.
    let discards = played
        .server_events
        .iter()
        .filter(|event| matches!(event, ServerEvent::TileDiscarded { .. }))
        .count();
    assert!(discards > 40, "the game stalled after {discards} discards");
}

#[test]
fn the_bot_discards_for_itself() {
    // If the bot never declared anything the driver would have been falling
    // back to its own defaults, and the protocol round trip would prove
    // nothing.
    let played = play_with_bot(CpuLevel::Strong);
    let seat = 0;

    let bot_discards = played
        .declared
        .iter()
        .filter(|event| matches!(event, MjaiEvent::Dahai { actor, .. } if *actor == seat))
        .count();
    assert!(
        bot_discards > 20,
        "the bot only discarded {bot_discards} times"
    );
}

#[test]
fn the_bot_never_answers_with_a_move_that_is_not_its_own() {
    let played = play_with_bot(CpuLevel::Strong);
    for event in &played.declared {
        if let Some(actor) = event.actor() {
            assert_eq!(
                actor, 0,
                "the bot declared a move for another seat: {event:?}"
            );
        }
        assert!(
            event.is_player_response(),
            "the bot sent something only a host may send: {event:?}"
        );
        assert!(event.validate().is_ok(), "malformed declaration: {event:?}");
    }
}

#[test]
fn a_weak_and_a_strong_bot_both_complete_a_game() {
    // The pipeline must not depend on which decisions come back.
    for level in [CpuLevel::Weak, CpuLevel::Normal, CpuLevel::Strong] {
        let played = play_with_bot(level);
        assert!(played.completed, "{level:?} stalled the game");
    }
}

#[test]
fn the_handshake_answers_with_a_join() {
    let mut bot = MjaiBot::new(
        "my-bot",
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        Settings::default(),
    );
    let MjaiEvent::Join { name, room } = bot.respond(&MjaiBot::hello()) else {
        panic!("expected a join");
    };
    assert_eq!(name, "my-bot");
    assert_eq!(room, "default");
    // The seat is not known until the game starts.
    assert_eq!(bot.actor(), None);
}

#[test]
fn the_bot_takes_the_seat_the_host_gives_it() {
    let mut bot = MjaiBot::new(
        "bot",
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        Settings::default(),
    );
    bot.respond(&MjaiBot::hello());
    bot.respond(&MjaiEvent::StartGame {
        id: Some(2),
        names: names(),
    });
    assert_eq!(bot.actor(), Some(2));
}

#[test]
fn nothing_is_declared_before_the_game_starts() {
    // A host that opens with anything unexpected must not make the bot act.
    let mut bot = MjaiBot::new(
        "bot",
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        Settings::default(),
    );
    let response = bot.respond(&MjaiEvent::Dora {
        dora_marker: mahjong_core::tile::Tile::new(mahjong_core::tile::Tile::P1),
    });
    assert_eq!(response, MjaiEvent::Pass);
}
