//! CPU同士の自動対戦シミュレーションを実行する
//!
//! 使い方:
//! ```sh
//! cargo run -p mahjong-server --release --example cpu_simulation -- [games] [seed]
//! ```
//!
//! デフォルトは 100 ゲーム、シード 42。
//! 同一シードなら結果は決定的なので、定石PRの前後で実行して
//! 集計値を比較することで回帰検知に使える。
//!
//! 注意:
//! - サーバの診断ログが stderr に出るため、`2>/dev/null` 等で抑制すると見やすい。
//! - 牌山生成は `SmallRng` を使うため、`rand` クレートのバージョンが変わると
//!   同一シードでも結果が変わりうる。比較は同一環境・同一依存バージョンで行うこと。

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
