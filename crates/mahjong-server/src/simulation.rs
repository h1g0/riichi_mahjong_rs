//! CPU同士の自動対戦シミュレーション
//!
//! 4人のCPUを同卓させて複数ゲームを実行し、和了率・放銃率などの
//! 統計を集計する。CPU定石（issue #142）の回帰検知に使用する。
//!
//! - 牌山はベースシードから決定的に導出されるため、同じシードなら
//!   常に同じ結果になり、ブランチ間で集計値を直接比較できる。
//! - 席順による有利不利を除去するため、ゲームごとにCPU設定を
//!   席ローテーションし、統計は席ではなくCPU設定ごとに集計する。
//! - `CpuConfig::without_heuristics()` のCPUを混ぜることで、
//!   定石導入前後のA/B比較が同一卓でできる。
//!
//! 実行例:
//! ```sh
//! cargo run -p mahjong-server --release --example cpu_simulation -- 100 42
//! ```

use std::fmt;

use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;

use crate::cpu::client::{CpuClient, CpuConfig, CpuLevel, CpuPersonality};
use crate::round::{RoundResult, TurnPhase};
use crate::table::{GameSettings, Table};

/// 1局あたりの進行ステップ数の上限（これを超えたら進行不能とみなす）
const MAX_STEPS_PER_ROUND: usize = 5000;

/// シミュレーション設定
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// 実行するゲーム（半荘/東風戦）数
    pub games: usize,
    /// ベースシード（ゲーム番号・局番号と合成して牌山シードを導出する）
    pub base_seed: u64,
    /// 対戦させる4人のCPU設定
    pub cpu_configs: [CpuConfig; 4],
    /// ゲーム設定（東風/東南、初期持ち点など）
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

/// デフォルトの対戦カード
///
/// レベル差の確認用に弱・中・強を1人ずつ、新旧比較用に
/// 定石無効の強を1人配置する。
pub fn default_simulation_configs() -> [CpuConfig; 4] {
    [
        CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced),
        CpuConfig::new(CpuLevel::Strong, CpuPersonality::Balanced).without_heuristics(),
    ]
}

/// CPU設定1つ分の集計結果
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpuStats {
    /// CPU設定のラベル（レベル/性格/定石有無）
    pub label: String,
    /// ツモ和了数
    pub tsumo_wins: u32,
    /// ロン和了数
    pub ron_wins: u32,
    /// 放銃数
    pub deal_ins: u32,
    /// リーチ宣言数
    pub riichi_count: u32,
    /// 副露数（ポン・チー・カンの合計。局をまたいで合算）
    pub meld_count: u32,
    /// 荒牌流局時に聴牌していた回数
    pub tenpai_at_draw: u32,
    /// 着順ごとの回数（[1着, 2着, 3着, 4着]）
    pub placements: [u32; 4],
    /// 最終持ち点の合計（平均算出用）
    pub total_final_score: i64,
}

impl CpuStats {
    /// 和了数の合計
    pub fn total_wins(&self) -> u32 {
        self.tsumo_wins + self.ron_wins
    }

    /// 平均着順
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

/// シミュレーション全体の集計結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationStats {
    /// CPU設定ごとの集計（SimulationConfig::cpu_configs と同じ順）
    pub per_cpu: [CpuStats; 4],
    /// 実行したゲーム数
    pub games: u32,
    /// 実行した局数
    pub rounds: u32,
    /// 荒牌流局の回数
    pub exhaustive_draws: u32,
    /// 途中流局の回数
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

/// CPU設定のラベルを生成する
fn config_label(config: &CpuConfig) -> String {
    let mut label = format!("{:?}/{:?}", config.level, config.personality);
    if !config.heuristics_enabled {
        label.push_str(" (no heuristics)");
    }
    label
}

/// 牌山シードをベースシード・ゲーム番号・局通し番号から導出する
///
/// splitmix64 の finalizer でビットを攪拌し、近いシード同士でも
/// 牌山が相関しないようにする。
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

/// シミュレーションを実行する
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

    for game in 0..config.games {
        // 席ローテーション: ゲーム g では設定 c が席 (c + g) % 4 に座る
        let config_for_seat: [usize; 4] = std::array::from_fn(|seat| (seat + 4 - game % 4) % 4);

        let mut cpus: [CpuClient; 4] = std::array::from_fn(|seat| {
            CpuClient::new(config.cpu_configs[config_for_seat[seat]].clone())
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

        // 着順集計（同点は起家に近い席が上位）
        let mut order: Vec<usize> = (0..4).collect();
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

/// 1局を最後まで進行させる
fn play_round(table: &mut Table, cpus: &mut [CpuClient; 4]) -> Result<(), String> {
    // 直近の拒否されたアクション（スタック診断用）
    let mut rejected_log: Vec<String> = Vec::new();

    // 局開始イベント（GameStarted など）を配信
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

/// イベントをCPUに配信し、返ってきたアクションをサーバに送る
///
/// アクションが新たなイベントを生成しうるため、イベントが尽きるまでループする。
/// 拒否されたアクションは診断用に `rejected_log` へ記録する
/// （鳴きの競合などで正当に拒否される場合もあるため、エラーにはしない）。
fn process_events(table: &mut Table, cpus: &mut [CpuClient; 4], rejected_log: &mut Vec<String>) {
    loop {
        let events = table.drain_events();
        if events.is_empty() {
            break;
        }

        let mut actions = Vec::new();
        for (player_idx, event) in &events {
            if let Some(action) = cpus[*player_idx].handle_event(event) {
                actions.push((*player_idx, action));
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

/// 終了した局から統計を収集する（finish_round の前に呼ぶ）
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
        Some(RoundResult::ExhaustiveDraw { .. }) => {
            stats.exhaustive_draws += 1;
            for (seat, player) in round.players.iter().enumerate() {
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

    for (seat, player) in round.players.iter().enumerate() {
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

    /// 高速なスモークテスト用の設定（弱CPUは受入計算を省くため速い）
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

    #[test]
    fn test_simulation_completes_and_is_consistent() {
        let stats = run_simulation(&fast_config(1, 42)).expect("simulation should complete");

        assert_eq!(stats.games, 1);
        assert!(stats.rounds >= 4, "東風戦は最低4局のはず");

        // 着順の合計はゲーム数と一致する
        for rank in 0..4 {
            let total: u32 = stats.per_cpu.iter().map(|c| c.placements[rank]).sum();
            assert_eq!(total, stats.games, "{}着の合計がゲーム数と不一致", rank + 1);
        }

        // 最終持ち点の合計は ゲーム数 × 初期持ち点 × 4 から供託分を引いた値以下
        let total_score: i64 = stats.per_cpu.iter().map(|c| c.total_final_score).sum();
        assert!(total_score <= stats.games as i64 * 25000 * 4);
    }

    #[test]
    fn test_simulation_is_deterministic_with_same_seed() {
        let first = run_simulation(&fast_config(2, 7)).expect("first run should complete");
        let second = run_simulation(&fast_config(2, 7)).expect("second run should complete");
        assert_eq!(first, second, "同一シードの結果が一致しない");
    }

    #[test]
    fn test_simulation_differs_with_different_seed() {
        // 異なるシードでは（牌山が変わるので）少なくとも局数か統計のどこかが変わる。
        // 万一一致しても誤りではないが、決定性テストの裏付けとして確認する。
        let a = run_simulation(&fast_config(2, 1)).expect("run a should complete");
        let b = run_simulation(&fast_config(2, 2)).expect("run b should complete");
        assert_ne!(
            a, b,
            "異なるシードで結果が完全一致（シードが効いていない疑い）"
        );
    }

    #[test]
    fn test_derive_wall_seed_is_deterministic_and_spread() {
        assert_eq!(derive_wall_seed(42, 0, 0), derive_wall_seed(42, 0, 0));
        // 近い入力でも異なるシードになる
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

    /// 回帰検知用の大きめのシミュレーション
    ///
    /// 実行: `cargo test -p mahjong-server --release -- --ignored simulation_regression --nocapture`
    /// 定石PRの前後で同一シードの結果を比較し、意図しない劣化がないか確認する。
    #[test]
    #[ignore = "slow: run explicitly with --ignored --nocapture for regression checks"]
    fn simulation_regression_metrics() {
        let config = SimulationConfig {
            games: 100,
            ..Default::default()
        };
        let stats = run_simulation(&config).expect("regression simulation should complete");
        println!("{stats}");

        // 基本的な健全性: 全ゲームが完走している
        assert_eq!(stats.games, 100);
    }
}
