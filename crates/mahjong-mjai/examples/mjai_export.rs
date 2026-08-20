//! Plays one CPU-vs-CPU game and writes it out as an mjai log.
//!
//! ```sh
//! cargo run -p mahjong-mjai --example mjai_export -- 42 > game.mjson
//! ```
//!
//! The output is newline-delimited JSON in replay mode: every hand and draw is
//! revealed, which is what mjai review tooling expects.

use mahjong_mjai::MjaiRecorder;
use mahjong_mjai::record::to_json_lines;
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};
use mahjong_server::driver::GameDriver;
use mahjong_server::table::GameSettings;

/// Step budget for one game; exceeding it means the game stalled.
const MAX_STEPS: usize = 20_000;

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(42);
    let level = match std::env::args().nth(2).as_deref() {
        Some("weak") => CpuLevel::Weak,
        Some("normal") => CpuLevel::Normal,
        _ => CpuLevel::Strong,
    };

    let mut driver = GameDriver::new(GameSettings::default());
    for seat in 0..4 {
        // A shadow CPU buffers events but does not act; handing it control
        // gives a seat that both plays and records, which is what a full
        // replay needs.
        driver.set_shadow_cpu(seat, CpuConfig::new(level, CpuPersonality::Balanced));
        driver.set_cpu_controlled(seat, true);
    }
    driver.start_game_with_seed(seed);

    let mut recorder =
        MjaiRecorder::new((0..4).map(|seat| format!("cpu{seat}")).collect::<Vec<_>>());

    let mut steps = 0;
    while steps < MAX_STEPS {
        steps += 1;
        driver.run_until_blocked();
        for seat in 0..4 {
            let events = driver.drain_events(seat);
            recorder.record(seat, events);
        }
        if driver.is_round_over() {
            if driver.is_game_over() {
                break;
            }
            driver.next_round();
        } else {
            driver.tick();
        }
    }

    let log = recorder.finish();
    eprintln!("seed {seed}: {} events over {steps} steps", log.len());
    print!("{}", to_json_lines(&log).expect("log should serialise"));
}
