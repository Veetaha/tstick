mod cmd;
mod display;

use clap::Parser;
use cmd::Cmd;

/// A tool that automates the management of telegram stickers and emojis
#[derive(Parser, Debug)]
#[command(version)]
enum Args {
    Video(cmd::Video),
}

pub fn run() -> anyhow::Result<()> {
    match Args::parse() {
        Args::Video(cmd) => cmd.run(),
    }
}
