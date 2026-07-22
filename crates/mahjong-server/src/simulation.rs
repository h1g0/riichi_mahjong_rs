//! Automated CPU-vs-CPU simulations.
//!
//! Seats four CPUs at a table, runs many games, and aggregates win rates,
//! deal-in rates, etc. Used as a regression check for the CPU heuristics
//! work (issue #142).
//!
//! - Walls are derived deterministically from the base seed, so the same
//!   seed always yields the same results and aggregates can be compared
//!   directly across branches.
//! - CPU configs rotate seats between games to cancel out seat advantage;
//!   stats are aggregated per config, not per seat.
//! - Mixing in a `CpuConfig::without_heuristics()` CPU enables A/B
//!   comparison of the heuristics at the same table.
//!
//! Example:
//! ```sh
//! cargo run -p mahjong-server --release --example cpu_simulation -- 100 42
//! ```

use std::fmt;

use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;

use crate::cpu::client::{CpuClient, CpuConfig, CpuLevel, CpuPersonality};
use crate::round::{RoundResult, TurnPhase};
use crate::table::{GameSettings, Table};

/// Step budget per hand; exceeding it is treated as a stall.
const MAX_STEPS_PER_ROUND: usize = 5000;

/// Simulation configuration.
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Number of games to run
    pub games: usize,
    /// Base seed, combined with game/hand numbers to derive wall seeds
    pub base_seed: u64,
    /// The four competing CPU configs
    pub cpu_configs: [CpuConfig; 4],
    /// Game settings (length, starting scores, ...)
    pub game_settings: GameSettings,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        SimulationConfig {
            games: 100,
            base_seed: 42,
            cpu_configs: default_simulation_configs(),
            game_settings: GameSettings::default(),
        }
    }
}

/// Default match-up: weak/normal/strong to gauge level differences, plus
/// a heuristics-disabled strong CPU for before/after comparison.
pub fn default_simulation_configs() -> [CpuConfig; 4] {
    [
        CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics(),
    ]
}

/// Aggregated stats for one CPU config.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpuStats {
    /// Config label (level/personality/heuristics)
    pub label: String,
    /// Wins by tsumo
    pub tsumo_wins: u32,
    /// Wins by ron
    pub ron_wins: u32,
    /// Deal-ins
    pub deal_ins: u32,
    /// Riichi declarations
    pub riichi_count: u32,
    /// Melds called (pon/chii/kan, summed across hands)
    pub meld_count: u32,
    /// Times tenpai at an exhaustive draw
    pub tenpai_at_draw: u32,
    /// Finishes per placement ([1st, 2nd, 3rd, 4th])
    pub placements: [u32; 4],
    /// Sum of final scores, for averaging
    pub total_final_score: i64,
}

impl CpuStats {
    /// Total wins.
    pub fn total_wins(&self) -> u32 {
        self.tsumo_wins + self.ron_wins
    }

    /// Average placement.
    pub fn average_placement(&self, games: u32) -> f64 {
        if games == 0 {
            return 0.0;
        }
        let weighted: u32 = self
            .placements
            .iter()
            .enumerate()
            .map(|(rank, &count)| (rank as u32 + 1) * count)
            .sum();
        weighted as f64 / games as f64
    }
}

/// Aggregated stats for the whole simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationStats {
    /// Per-config stats, in `SimulationConfig::cpu_configs` order
    pub per_cpu: [CpuStats; 4],
    /// Games played
    pub games: u32,
    /// Hands played
    pub rounds: u32,
    /// Exhaustive draws
    pub exhaustive_draws: u32,
    /// Abortive draws
    pub special_draws: u32,
}

impl fmt::Display for SimulationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "games: {}, rounds: {}, exhaustive draws: {}, special draws: {}",
            self.games, self.rounds, self.exhaustive_draws, self.special_draws
        )?;
        writeln!(
            f,
            "{:<32} {:>6} {:>6} {:>6} {:>6} {:>6} {:>8} {:>8} {:>10}",
            "cpu",
            "win%",
            "deal%",
            "riichi",
            "melds",
            "tenpai",
            "avg rank",
            "rank dist",
            "avg score"
        )?;
        for stats in &self.per_cpu {
            let rounds = self.rounds.max(1) as f64;
            writeln!(
                f,
                "{:<32} {:>5.1}% {:>5.1}% {:>6} {:>6} {:>6} {:>8.2} {:>2}-{}-{}-{} {:>10.0}",
                stats.label,
                stats.total_wins() as f64 / rounds * 100.0,
                stats.deal_ins as f64 / rounds * 100.0,
                stats.riichi_count,
                stats.meld_count,
                stats.tenpai_at_draw,
                stats.average_placement(self.games),
                stats.placements[0],
                stats.placements[1],
                stats.placements[2],
                stats.placements[3],
                if self.games > 0 {
                    stats.total_final_score as f64 / self.games as f64
                } else {
                    0.0
                },
            )?;
        }
        Ok(())
    }
}

/// Builds the label for a CPU config.
fn config_label(config: &CpuConfig) -> String {
    let mut label = format!("{:?}/{:?}", config.level, config.personality);
    if !config.heuristics_enabled {
        label.push_str(" (no heuristics)");
    }
    label
}

/// Derives a wall seed from the base seed, game number, and hand serial.
///
/// The splitmix64 finalizer scrambles the bits so nearby inputs do not
/// produce correlated walls.
fn derive_wall_seed(base_seed: u64, game: u64, round_serial: u64) -> u64 {
    let mut x = base_seed
        ^ game.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ round_serial.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x
}

/// Runs the simulation.
pub fn run_simulation(config: &SimulationConfig) -> Result<SimulationStats, String> {
    let mut stats = SimulationStats {
        per_cpu: std::array::from_fn(|i| CpuStats {
            label: config_label(&config.cpu_configs[i]),
            ..CpuStats::default()
        }),
        games: 0,
        rounds: 0,
        exhaustive_draws: 0,
        special_draws: 0,
    };

    let player_count = config.game_settings.rules.player_count();

    for game in 0..config.games {
        // Seat rotation: in game g, config c sits at seat (c + g) % n.
        // In three-player games dummy seat 3 keeps config 3; it receives
        // no events, so it never actually acts.
        let config_for_seat: [usize; 4] = std::array::from_fn(|seat| {
            if seat < player_count {
                (seat + player_count - game % player_count) % player_count
            } else {
                seat
            }
        });

        let mut cpus: [CpuClient; 4] = std::array::from_fn(|seat| {
            CpuClient::new_with_rules(
                config.cpu_configs[config_for_seat[seat]].clone(),
                &config.game_settings.rules,
            )
        });

        let mut table = Table::new(config.game_settings.clone());
        let mut round_serial = 0u64;

        while !table.is_game_over {
            let seed = derive_wall_seed(config.base_seed, game as u64, round_serial);
            round_serial += 1;

            table.start_round_with_seed(seed);
            play_round(&mut table, &mut cpus)
                .map_err(|e| format!("game {game}, round {round_serial}: {e}"))?;

            collect_round_stats(&table, &config_for_seat, &mut stats)?;
            stats.rounds += 1;

            table.finish_round();
        }

        // Placements; ties go to the seat closer to the starting dealer.
        let mut order: Vec<usize> = (0..player_count).collect();
        order.sort_by_key(|&seat| (std::cmp::Reverse(table.scores[seat]), seat));
        for (rank, &seat) in order.iter().enumerate() {
            let cpu_stats = &mut stats.per_cpu[config_for_seat[seat]];
            cpu_stats.placements[rank] += 1;
            cpu_stats.total_final_score += table.scores[seat] as i64;
        }
        stats.games += 1;
    }

    Ok(stats)
}

/// Plays one hand to completion.
fn play_round(table: &mut Table, cpus: &mut [CpuClient; 4]) -> Result<(), String> {
    // Recent rejected actions, kept for stall diagnostics.
    let mut rejected_log: Vec<String> = Vec::new();

    process_events(table, cpus, &mut rejected_log);

    for _ in 0..MAX_STEPS_PER_ROUND {
        let phase = {
            let round = table
                .current_round()
                .ok_or_else(|| "round disappeared during play".to_string())?;
            if round.is_over() {
                return Ok(());
            }
            round.phase.clone()
        };

        if phase == TurnPhase::Draw {
            table
                .current_round_mut()
                .ok_or_else(|| "round disappeared during draw".to_string())?
                .do_draw();
        }

        process_events(table, cpus, &mut rejected_log);
    }

    let detail = table
        .current_round()
        .map(|r| {
            let cp = r.current_player;
            let player = &r.players[cp];
            format!(
                "phase={:?} current_player={cp} hand={:?} drawn={:?} melds={} is_riichi={}",
                r.phase,
                player.hand.tiles(),
                player.hand.drawn(),
                player.hand.melds().len(),
                player.is_riichi,
            )
        })
        .unwrap_or_else(|| "round missing".to_string());
    Err(format!(
        "round did not finish within {MAX_STEPS_PER_ROUND} steps (stalled: {detail}; recent rejected actions: {rejected_log:?})"
    ))
}

/// Delivers events to the CPUs and feeds their actions back to the server.
///
/// Actions can generate further events, so this loops until the queue is
/// empty. Rejected actions go into `rejected_log` for diagnostics only:
/// rejections can be legitimate (e.g. losing a call race), so they are
/// not errors.
fn process_events(table: &mut Table, cpus: &mut [CpuClient; 4], rejected_log: &mut Vec<String>) {
    loop {
        let events = table.drain_events();
        if events.is_empty() {
            break;
        }

        let mut actions = Vec::new();
        for (player_idx, event) in &events {
            if let Some(action) = cpus[*player_idx].handle_event(event) {
                if let Some((_, queued)) = actions
                    .iter_mut()
                    .find(|(queued_player, _)| queued_player == player_idx)
                {
                    *queued = action;
                } else {
                    actions.push((*player_idx, action));
                }
            }
        }

        if actions.is_empty() {
            break;
        }

        for (player_idx, action) in actions {
            if !table.handle_action(player_idx, action.clone()) {
                if rejected_log.len() >= 8 {
                    rejected_log.remove(0);
                }
                rejected_log.push(format!("player {player_idx}: {action:?}"));
            }
        }
    }
}

/// Collects stats from a finished hand; must run before finish_round.
fn collect_round_stats(
    table: &Table,
    config_for_seat: &[usize; 4],
    stats: &mut SimulationStats,
) -> Result<(), String> {
    let round = table
        .current_round()
        .ok_or_else(|| "round missing during stats collection".to_string())?;

    match &round.result {
        Some(RoundResult::Tsumo { winner, .. }) => {
            stats.per_cpu[config_for_seat[*winner]].tsumo_wins += 1;
        }
        Some(RoundResult::Ron { winners, loser, .. }) => {
            for winner in winners {
                stats.per_cpu[config_for_seat[*winner]].ron_wins += 1;
            }
            stats.per_cpu[config_for_seat[*loser]].deal_ins += 1;
        }
        Some(RoundResult::NagashiMangan { winners }) => {
            for winner in winners {
                stats.per_cpu[config_for_seat[*winner]].tsumo_wins += 1;
            }
        }
        Some(RoundResult::ExhaustiveDraw { .. }) => {
            stats.exhaustive_draws += 1;
            for (seat, player) in round.players.iter().enumerate().take(round.player_count) {
                if calc_shanten_number(&player.hand).is_ready_or_won() {
                    stats.per_cpu[config_for_seat[seat]].tenpai_at_draw += 1;
                }
            }
        }
        Some(RoundResult::SpecialDraw) => {
            stats.special_draws += 1;
        }
        None => {
            return Err("round over without result".to_string());
        }
    }

    for (seat, player) in round.players.iter().enumerate().take(round.player_count) {
        let cpu_stats = &mut stats.per_cpu[config_for_seat[seat]];
        if player.is_riichi {
            cpu_stats.riichi_count += 1;
        }
        cpu_stats.meld_count += player.hand.melds().len() as u32;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fast smoke-test config: weak CPUs skip tile-acceptance analysis.
    fn fast_config(games: usize, base_seed: u64) -> SimulationConfig {
        SimulationConfig {
            games,
            base_seed,
            cpu_configs: [
                CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced),
                CpuConfig::new(CpuLevel::Weak, CpuPersonality::Speedy),
                CpuConfig::new(CpuLevel::Weak, CpuPersonality::HighValue),
                CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced).without_heuristics(),
            ],
            game_settings: GameSettings::default(),
        }
    }

    /// Fast smoke-test config for three-player games.
    fn fast_sanma_config(games: usize, base_seed: u64) -> SimulationConfig {
        SimulationConfig {
            game_settings: GameSettings::sanma_default(),
            ..fast_config(games, base_seed)
        }
    }

    #[test]
    fn test_sanma_simulation_completes_and_is_consistent() {
        let stats =
            run_simulation(&fast_sanma_config(2, 42)).expect("sanma simulation should complete");

        assert_eq!(stats.games, 2);
        assert!(
            stats.rounds >= 3 * 2,
            "three-player East-only games should have at least three hands"
        );

        // Three players means placements 1-3 only; the dummy seat
        // (config 3) must never place.
        for rank in 0..3 {
            let total: u32 = stats.per_cpu.iter().map(|c| c.placements[rank]).sum();
            assert_eq!(
                total,
                stats.games,
                "total finishes at rank {} do not match the game count",
                rank + 1
            );
        }
        let fourth_total: u32 = stats.per_cpu.iter().map(|c| c.placements[3]).sum();
        assert_eq!(
            fourth_total, 0,
            "a fourth-place finish occurred in a three-player game"
        );
        assert_eq!(
            stats.per_cpu[3].placements, [0; 4],
            "the dummy seat was included in the statistics"
        );

        // Total final scores cannot exceed games x starting score x 3
        // (riichi deposits can only leak out).
        let total_score: i64 = stats.per_cpu.iter().map(|c| c.total_final_score).sum();
        assert!(total_score <= stats.games as i64 * 35000 * 3);
    }

    #[test]
    fn test_sanma_simulation_is_deterministic_with_same_seed() {
        let first = run_simulation(&fast_sanma_config(1, 7)).expect("first run should complete");
        let second = run_simulation(&fast_sanma_config(1, 7)).expect("second run should complete");
        assert_eq!(
            first, second,
            "three-player results differ for the same seed"
        );
    }

    #[test]
    fn test_simulation_completes_and_is_consistent() {
        let stats = run_simulation(&fast_config(1, 42)).expect("simulation should complete");

        assert_eq!(stats.games, 1);
        assert!(
            stats.rounds >= 4,
            "East-only games should have at least four hands"
        );

        for rank in 0..4 {
            let total: u32 = stats.per_cpu.iter().map(|c| c.placements[rank]).sum();
            assert_eq!(
                total,
                stats.games,
                "total finishes at rank {} do not match the game count",
                rank + 1
            );
        }

        // Total final scores cannot exceed games x starting score x 4
        // (riichi deposits can only leak out).
        let total_score: i64 = stats.per_cpu.iter().map(|c| c.total_final_score).sum();
        assert!(total_score <= stats.games as i64 * 25000 * 4);
    }

    #[test]
    fn test_simulation_is_deterministic_with_same_seed() {
        let first = run_simulation(&fast_config(2, 7)).expect("first run should complete");
        let second = run_simulation(&fast_config(2, 7)).expect("second run should complete");
        assert_eq!(first, second, "results differ for the same seed");
    }

    #[test]
    fn test_simulation_differs_with_different_seed() {
        // Different seeds change the walls, so something in the stats
        // should differ. A coincidental match would not be a bug, but this
        // backs up the determinism test.
        let a = run_simulation(&fast_config(2, 1)).expect("run a should complete");
        let b = run_simulation(&fast_config(2, 2)).expect("run b should complete");
        assert_ne!(
            a, b,
            "results are identical for different seeds; the seed may have no effect"
        );
    }

    #[test]
    fn test_derive_wall_seed_is_deterministic_and_spread() {
        assert_eq!(derive_wall_seed(42, 0, 0), derive_wall_seed(42, 0, 0));
        // Nearby inputs must still produce distinct seeds.
        assert_ne!(derive_wall_seed(42, 0, 0), derive_wall_seed(42, 0, 1));
        assert_ne!(derive_wall_seed(42, 0, 0), derive_wall_seed(42, 1, 0));
        assert_ne!(derive_wall_seed(42, 0, 0), derive_wall_seed(43, 0, 0));
    }

    #[test]
    fn test_config_label() {
        let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced);
        assert_eq!(config_label(&config), "Strong/Balanced");
        assert_eq!(
            config_label(&config.without_heuristics()),
            "Strong/Balanced (no heuristics)"
        );
    }

    /// Larger simulation for regression checks.
    ///
    /// Run: `cargo test -p mahjong-server --release -- --ignored simulation_regression --nocapture`
    /// Compare same-seed results before and after a heuristics PR to catch
    /// unintended regressions.
    #[test]
    #[ignore = "slow: run explicitly with --ignored --nocapture for regression checks"]
    fn simulation_regression_metrics() {
        let config = SimulationConfig {
            games: 100,
            ..Default::default()
        };
        let stats = run_simulation(&config).expect("regression simulation should complete");
        println!("{stats}");

        assert_eq!(stats.games, 100);
    }
}
