use anyhow::Result;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tracing::debug;

const DEFAULT_FF_OPTIONS: &[&str] = &["-loglevel", "warning"];

pub(crate) fn get_media_duration(path: &Path) -> Result<Duration> {
    let args = [
        "-show_entries",
        "format=duration",
        "-print_format",
        "csv=\"print_section=0\"",
        "-i",
        &path.to_string_lossy(),
    ];

    let output = ffprobe(args)?;

    let duration = String::from_utf8(output)?.trim().parse::<f64>()?;

    Ok(Duration::from_secs_f64(duration))
}

pub(crate) fn ffmpeg(args: impl IntoIterator<Item = impl Into<String>>) -> Result<Vec<u8>> {
    run_ff("ffmpeg", args)
}

pub(crate) fn ffprobe(args: impl IntoIterator<Item = impl Into<String>>) -> Result<Vec<u8>> {
    run_ff("ffprobe", args)
}

fn run_ff(program: &str, args: impl IntoIterator<Item = impl Into<String>>) -> Result<Vec<u8>> {
    let args = DEFAULT_FF_OPTIONS
        .iter()
        .copied()
        .map(ToOwned::to_owned)
        .chain(args.into_iter().map(Into::into));

    run_cmd(program, args)
}

fn run_cmd(program: &str, args: impl IntoIterator<Item = impl Into<String>>) -> Result<Vec<u8>> {
    let args: Vec<_> = args.into_iter().map(Into::into).collect();

    debug!("{program} {}", shlex::join(args.iter().map(String::as_str)));

    Ok(Command::new(program).args(args).output()?.stdout)
}
