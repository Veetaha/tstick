use crate::video::StickerKind;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
/// Generate telegram emoji and sticker from a video using ffmpeg
///
/// The output files will be put into the same directory where the input file
/// is located, but with names `emoji.webm` and `sticker.webm` respectively.
///
/// This command implements the two-pass method described in the following docs:
/// <https://trac.ffmpeg.org/wiki/Encode/VP9>
#[derive(Parser, Debug)]
pub struct Video {
    /// Kind of the output to generate
    #[clap(value_enum)]
    kind: StickerKind,

    /// Path to the input file to convert into a sticker and emoji
    input: PathBuf,

    /// Set a custom CRF value to start search for the most optimal one from
    #[clap(long, default_value_t = 18)]
    start_crf: usize,

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

impl crate::cmd::Cmd for Video {
    fn run(self) -> anyhow::Result<()> {
        self.generate_output(self.kind)?;

        Ok(())
    }
}

impl Video {
    fn generate_output(&self, sticker_kind: StickerKind) -> Result<()> {
        crate::video::VideoGenContext {
            input: self.input.clone(),
            start_crf: self.start_crf,
            filter: self.filter.clone(),
            ffmpeg_args: self.ffmpeg_args.clone(),
            sticker_kind,
        }
        .generate_output()
    }
}
