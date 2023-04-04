mod cmd;
mod display;
mod ffmpeg;
mod fs;
mod util;
mod video;

// We don't care if some of the imports here are not used. They may be used
// at some point. It's just convenient not to import them manually all the
// time a new logging macro is needed.
#[allow(unused_imports)]
mod prelude {
    pub(crate) use anyhow::{bail, Context};
    pub(crate) use camino::{Utf8Path, Utf8PathBuf};
    pub(crate) use fs_err::tokio as fs;
    pub(crate) use itertools::Itertools;
    pub(crate) use tracing::{
        debug, debug_span, error, error_span, info, info_span, instrument, trace, trace_span, warn,
        warn_span, Instrument as _,
    };

    pub(crate) use crate::util::duration::DurationExt;
    pub(crate) use crate::util::error::ResultExt;
    pub(crate) use crate::util::path::PathExt;

    pub(crate) type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;
}

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
