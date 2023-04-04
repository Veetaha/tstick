use crate::prelude::*;
use anyhow::{bail, Context, Result};
use itertools::Itertools;
use nu_ansi_term::{Color, Style};
use std::iter;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::debug;

const DEFAULT_FF_OPTIONS: &[&str] = &["-loglevel", "error"];

/// If the CLI display string length exceeds this value, then the command
/// will be printed using multiline format.
const LONG_CMD_THRESHOLD: usize = 100;

// FIXME: this may be useful?
#[allow(unused)]
pub(crate) async fn get_media_duration(path: &Utf8Path) -> Result<Duration> {
    let args = [
        "-show_entries",
        "format=duration",
        "-print_format",
        "csv=print_section=0",
        "-i",
        path.as_str(),
    ];

    let output = ffprobe(args).await?;

    let duration = String::from_utf8(output)?.trim().parse::<f64>()?;

    Ok(Duration::from_secs_f64(duration))
}

pub(crate) async fn ffmpeg(args: impl IntoIterator<Item = impl Into<String>>) -> Result<Vec<u8>> {
    run_ff("ffmpeg", args).await
}

pub(crate) async fn ffprobe(args: impl IntoIterator<Item = impl Into<String>>) -> Result<Vec<u8>> {
    run_ff("ffprobe", args).await
}

async fn run_ff(
    program: &str,
    args: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Vec<u8>> {
    let args = DEFAULT_FF_OPTIONS
        .iter()
        .copied()
        .map(ToOwned::to_owned)
        .chain(args.into_iter().map(Into::into));

    run_cmd(program, args).await
}

async fn run_cmd(
    program: &str,
    args: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Vec<u8>> {
    let args: Vec<_> = args.into_iter().map(Into::into).collect();

    let cli = render_cli(program, args.iter().map(String::as_str));
    debug!("{cli}");

    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?
        .wait_with_output();

    let output = tokio::select! {
        ctrlc = tokio::signal::ctrl_c() => {
            ctrlc.context("couldn't Ctrl+C")?;
            bail!("Process was killed with Ctrl+C");
        }
        output = output => {
            output.context("couldn't run command")?
        }
    };

    if !output.status.success() {
        let status = output.status;

        bail!("Process `{program}` failed with {status}");
    }

    Ok(output.stdout)
}

fn render_cli<'a>(
    program: &'a str,
    args: impl ExactSizeIterator<Item = &'a str> + Clone,
) -> String {
    let program = Color::Blue.paint(shlex::quote(program));

    let args = args.map(|arg| {
        let arg = shlex::quote(arg);
        if arg.starts_with('-') {
            Color::Blue.paint(arg)
        } else {
            Style::new().paint(arg)
        }
    });

    let parts = iter::once(program).chain(args);

    let compact = parts.clone().join(" ");
    if compact.len() <= LONG_CMD_THRESHOLD {
        return compact;
    }
    format!("(\n  {}\n)", { parts }.format(" \n    "))
}
