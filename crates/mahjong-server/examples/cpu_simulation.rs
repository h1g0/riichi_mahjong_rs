//! Runs automated CPU-vs-CPU game simulations.
//!
//! Usage:
//! ```sh
//! cargo run -p mahjong-server --release --example cpu_simulation -- [games] [seed]
//! ```
//!
//! Defaults to 100 games with seed 42. Results are deterministic for a
//! given seed, so running this before and after an AI-heuristics PR and
//! comparing the aggregate stats works as a regression check.
//!
//! Notes:
//! - The server writes diagnostics to stderr; silence it with `2>/dev/null`.
//! - Wall generation uses `SmallRng`, so a `rand` crate upgrade can change
//!   results even for the same seed. Compare runs only on identical
//!   environments and dependency versions.

use mahjong_server::simulation::{SimulationConfig, run_simulation};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(SimulationConfig::default().games);
    let base_seed = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(SimulationConfig::default().base_seed);

    let config = SimulationConfig {
        games,
        base_seed,
        ..Default::default()
    };

    println!("running {games} games (base seed: {base_seed})...");
    match run_simulation(&config) {
        Ok(stats) => print!("{stats}"),
        Err(e) => {
            eprintln!("simulation failed: {e}");
            std::process::exit(1);
        }
    }
}
