use anyhow::{Context, Result};
use clap::Parser;
use fs_err as fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info};

const MAX_EMOJI_BYTES: usize = 64 * KIB;
const MAX_STICKER_BYTES: usize = 256 * KIB;

const KIB: usize = 1024;

/// Generate telegram emoji and sticker from a video using ffmpeg
///
/// The output files will be put into the same directory where the input file
/// is located, but with names `emoji.webm` and `sticker.webm` respectively.
///
/// This command implements the two-pass method described in the following docs:
/// <https://trac.ffmpeg.org/wiki/Encode/VP9>
#[derive(Parser, Debug)]
pub struct Video {
    /// Path to the input file to convert into a sticker and emoji
    #[clap(short, long)]
    input: PathBuf,

    /// Don't generate an emoji
    #[clap(long)]
    no_emoji: bool,

    /// Don't generate a sticker
    #[clap(long)]
    no_sticker: bool,

    /// Set a custom CRF value to start search for the most optimal one from
    #[clap(long, default_value_t = 18)]
    start_crf: usize,

    /// Arguments that will be passed to ffmpeg between the input and output args
    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    ffmpeg_args: Vec<String>,
}

impl crate::cmd::Cmd for Video {
    fn run(self) -> anyhow::Result<()> {
        if !self.no_emoji {
            self.generate_output("emoji", MAX_EMOJI_BYTES)?;
        }

        if !self.no_sticker {
            self.generate_output("sticker", MAX_STICKER_BYTES)?;
        }

        Ok(())
    }
}

impl Video {
    fn generate_output(&self, file_name: &str, max_bytes: usize) -> Result<()> {
        let dir = self.input.parent().with_context(|| {
            format!(
                "There is no parent directory for the input file {:?}",
                self.input
            )
        })?;

        let bytes = self.ffmpeg_vp9_two_pass(max_bytes)?;

        let file = dir.join(format!("{file_name}.webm"));

        fs::write(&file, bytes)?;

        info!("📄 Generated {}", file.display());

        Ok(())
    }

    fn ffmpeg_vp9_two_pass(&self, max_bytes: usize) -> Result<Vec<u8>> {
        let args = [
            "-y",
            "-i",
            &self.input.to_string_lossy(),
            "-vcodec",
            "libvpx-vp9",
            // From the docs: constant quality 2-pass is invoked by setting
            // -b:v to zero and specifiying a quality level using the -crf switch
            "-b:v",
            "0",
            // Audio streams are must be removed from the output
            "-an",
        ];

        let run_ffmpeg = |trailing_args: &[&str]| {
            let args = args
                .into_iter()
                .map(str::to_owned)
                .chain(self.ffmpeg_args.iter().cloned())
                .chain(trailing_args.iter().copied().map(str::to_owned))
                .collect();

            ffmpeg(args)
        };

        let max_bytes_display = crate::display::human_size(max_bytes);

        for crf in self.start_crf..63 {
            info!("Trying CRF {} to fit into {}", crf, max_bytes_display);

            let crf = &crf.to_string();

            let null_output = if cfg!(windows) { "NUL" } else { "/dev/null" };

            // First pass
            run_ffmpeg(&["-crf", crf, "-pass", "1", "-f", "null", null_output])?;

            // Second pass
            //
            // `pipe:1` instructs ffmpeg not to save the output to a file, but instead
            // write it to the stdout of the process
            //
            // Because we don't specify the file name, but instead use the stdout then
            // `ffmpeg` can no longer infer the output format from the file extension,
            // so we have to pass `-f webm` to specify that separately explicitly
            let output = run_ffmpeg(&["-crf", crf, "-pass", "2", "-f", "webm", "pipe:1"])?;

            let output_size = crate::display::human_size(output.len());

            if output.len() <= max_bytes {
                info!("🎉 Found a fitting CRF {crf}, which generates {output_size}",);
                return Ok(output);
            }

            info!("The output for CRF {crf} is too big: {output_size}");
        }

        anyhow::bail!(
            "The output file can not possibly fit into the telegram limits with any crf value"
        );
    }
}

fn ffmpeg(args: Vec<String>) -> Result<Vec<u8>> {
    debug!("ffmpeg {}", shlex::join(args.iter().map(String::as_str)));

    let output = Command::new("ffmpeg").args(args).output()?.stdout;

    Ok(output)
}
