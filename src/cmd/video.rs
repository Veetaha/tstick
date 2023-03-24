use crate::video::StickerKind;
use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Generate telegram emoji or sticker from a video using ffmpeg
///
/// The output file will be put into the same directory where the input file
/// is located, but with name `emoji.webm` or `sticker.webm` by default.
///
/// This command implements the two-pass method described in the following docs:
/// <https://trac.ffmpeg.org/wiki/Encode/VP9>
#[derive(Parser, Debug)]
pub struct Video {
    /// Kind of the output to generate
    #[clap(value_enum)]
    kind: StickerKind,

    /// Path to the input media file
    input: PathBuf,

    /// Path to the output. By default, the output will be put into the same
    /// directory under the name `emoji.webm` or `sticker.webm` depending on
    /// the kind of the output.
    output: Option<PathBuf>,

    /// The time from which the video will be cut.
    ///
    /// The total video duration must not exceed 3 seconds.
    #[clap(long, value_parser = crate::util::duration::parse)]
    begin: Option<Duration>,

    /// The time to which the video will be cut.
    ///
    /// The total video duration must not exceed 3 seconds.
    #[clap(long, value_parser = crate::util::duration::parse)]
    end: Option<Duration>,

    /// The value of the video filter flag that will be passed to ffmpeg
    /// before rescaling it to the needed size
    #[clap(long)]
    filter: Option<String>,

    /// Additional arguments that will be passed to ffmpeg between the input and output args.
    /// Beware that they may break the internal logic of generating the `ffmpeg` command.
    /// For example, if you need additional video filter use `--filter` flag instead.
    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    ffmpeg_args: Vec<String>,
}

#[async_trait]
impl crate::cmd::Cmd for Video {
    async fn run(self) -> anyhow::Result<()> {
        self.generate_output(self.kind).await?;

        Ok(())
    }
}

impl Video {
    async fn generate_output(&self, sticker_kind: StickerKind) -> Result<()> {
        crate::video::VideoGenContext {
            sticker_kind,
            input: self.input.clone(),
            begin: self.begin,
            end: self.end,
            filter: self.filter.clone(),
            ffmpeg_args: self.ffmpeg_args.clone(),
            ffmpeg: Arc::new(crate::ffmpeg::FfmpegProcess),
        }
        .generate_output()
        .await
    }
}
