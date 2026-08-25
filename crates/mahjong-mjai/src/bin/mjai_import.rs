//! Imports game logs and audits this engine against them.
//!
//! Logs arrive as mjai. Tenhou and Mahjong Soul records are converted first, by
//! one of the existing converters, which is why there is no site-specific
//! parser here:
//!
//! ```sh
//! cargo run -p mahjong-mjai --bin mjai-import -- --rules tenhou game.mjson
//! ```
//!
//! Each hand is replayed through the engine and every result is compared with
//! the one the log reports: han, minipoints, and the point transfers on a win,
//! and the ready declarations at an exhaustive draw. A finding means this
//! project and the site that produced the log disagree — usually a bug here,
//! occasionally a rule the log was played under that `--rules` was not told
//! about.
//!
//! Exits non-zero when anything disagreed, so a corpus of logs can be run as a
//! regression check.

use std::io::Read;

use mahjong_core::settings::Settings;
use mahjong_mjai::replay::{LogSource, ReplayReport, audit_log};

fn main() -> std::process::ExitCode {
    let options = match Options::from_args() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("mjai-import: {message}");
            eprintln!("{USAGE}");
            return std::process::ExitCode::from(2);
        }
    };

    let mut total = ReplayReport::default();
    let mut failed = false;
    for path in &options.paths {
        let text = match read_log(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("mjai-import: {path}: {error}");
                failed = true;
                continue;
            }
        };
        let log = match mahjong_mjai::from_json_lines(&text) {
            Ok(log) => log,
            Err(error) => {
                eprintln!("mjai-import: {path}: not an mjai log: {error}");
                failed = true;
                continue;
            }
        };

        let report = audit_log(&log, options.rules.clone());
        println!("{path}: {report}");
        for finding in &report.findings {
            println!("  {finding}");
        }
        accumulate(&mut total, &report);
    }

    if options.paths.len() > 1 {
        println!("total: {total}");
    }
    if failed || !total.is_clean() {
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn read_log(path: &str) -> std::io::Result<String> {
    if path == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        return Ok(text);
    }
    std::fs::read_to_string(path)
}

/// Rolls one file's report into the running total.
fn accumulate(total: &mut ReplayReport, report: &ReplayReport) {
    total.hands += report.hands;
    total.hands_skipped += report.hands_skipped;
    total.wins += report.wins;
    total.wins_checked += report.wins_checked;
    total.draws += report.draws;
    total.draws_checked += report.draws_checked;
    total.findings.extend(report.findings.iter().cloned());
}

const USAGE: &str = "usage: mjai-import [--rules tenhou|mahjong-soul|default] <log.mjson>...
       a path of - reads the log from standard input";

struct Options {
    rules: Settings,
    paths: Vec<String>,
}

impl Options {
    fn from_args() -> Result<Self, String> {
        let mut rules = Settings::default();
        let mut paths = Vec::new();

        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--rules" => {
                    let Some(value) = args.get(index + 1) else {
                        return Err("--rules needs a value".to_owned());
                    };
                    rules = match value.as_str() {
                        "tenhou" => LogSource::Tenhou.settings(),
                        "mahjong-soul" | "mjsoul" => LogSource::MahjongSoul.settings(),
                        "default" => Settings::default(),
                        other => return Err(format!("unknown rule set {other:?}")),
                    };
                    index += 2;
                }
                "--help" | "-h" => return Err("nothing to do".to_owned()),
                other => {
                    paths.push(other.to_owned());
                    index += 1;
                }
            }
        }

        if paths.is_empty() {
            return Err("no log given".to_owned());
        }
        Ok(Options { rules, paths })
    }
}
