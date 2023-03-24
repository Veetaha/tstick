mod cmd;
mod display;
mod ffmpeg;
mod util;
mod video;

use clap::Parser;
use cmd::Cmd;

/// A tool that automates the management of telegram stickers and emojis
#[derive(Parser, Debug)]
#[command(version)]
enum Args {
    Video(cmd::Video),
}

pub async fn run() -> anyhow::Result<()> {
    match Args::parse() {
        Args::Video(cmd) => cmd.run().await,
    }
}
