//! An mjai bot speaking newline-delimited JSON over stdin and stdout.
//!
//! This is the shape mjai hosts expect, so the project's CPU opponent can be
//! run and reviewed by existing mjai tooling:
//!
//! ```sh
//! cargo build --release -p mahjong-mjai --bin mjai-bot
//! ./target/release/mjai-bot --level strong --name my-bot
//! ```
//!
//! One line in, one line out. Anything that cannot be parsed is answered with
//! `none` and reported on stderr rather than killing the process, because a
//! host that gets no reply will simply hang waiting for one.

use std::io::{BufRead, Write};

use mahjong_core::settings::Settings;
use mahjong_mjai::bot::MjaiBot;
use mahjong_mjai::{MjaiEvent, from_json, to_json};
use mahjong_server::cpu::client::{CpuConfig, CpuLevel, CpuPersonality};

fn main() -> std::io::Result<()> {
    let options = Options::from_args();
    let mut bot = MjaiBot::new(
        options.name,
        CpuConfig::new(options.level, options.personality),
        Settings::default(),
    );

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match from_json(&line) {
            Ok(event) => {
                let response = bot.respond(&event);
                if matches!(event, MjaiEvent::EndGame) {
                    write_event(&mut stdout, &response)?;
                    break;
                }
                response
            }
            Err(error) => {
                eprintln!("mjai-bot: could not parse event: {error}");
                MjaiEvent::Pass
            }
        };
        write_event(&mut stdout, &response)?;
    }
    Ok(())
}

fn write_event(out: &mut impl Write, event: &MjaiEvent) -> std::io::Result<()> {
    let json = to_json(event).unwrap_or_else(|_| r#"{"type":"none"}"#.to_owned());
    writeln!(out, "{json}")?;
    // Hosts read a line at a time and block until it arrives, so every reply
    // has to leave the buffer immediately.
    out.flush()
}

struct Options {
    name: String,
    level: CpuLevel,
    personality: CpuPersonality,
}

impl Options {
    fn from_args() -> Self {
        let mut options = Options {
            name: "riichi-mahjong-rs".to_owned(),
            level: CpuLevel::Strong,
            personality: CpuPersonality::Balanced,
        };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut index = 0;
        while index + 1 < args.len() {
            let value = args[index + 1].as_str();
            match args[index].as_str() {
                "--name" => options.name = value.to_owned(),
                "--level" => {
                    options.level = match value {
                        "weak" => CpuLevel::Weak,
                        "normal" => CpuLevel::Normal,
                        _ => CpuLevel::Strong,
                    }
                }
                "--personality" => {
                    options.personality = match value {
                        "speedy" => CpuPersonality::Speedy,
                        "high-value" => CpuPersonality::HighValue,
                        "defensive" => CpuPersonality::Defensive,
                        _ => CpuPersonality::Balanced,
                    }
                }
                _ => {}
            }
            index += 2;
        }
        options
    }
}
