use crate::prelude::*;
use crate::video::{MultiVideoGenContext, PackKind};
use async_trait::async_trait;
use clap::{Args, Parser};
use std::num::NonZeroUsize;
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
    #[clap(flatten)]
    pack_kinds: PackKindArgs,

    /// Path to the input media file(s) or directory(ies) containing media files
    /// to be processed.
    ///
    /// If the input is a directory, all files inside of it will be processed.
    /// Make sure there are no other files in the directory other than the ones to
    /// generate emoji/stickers for.
    #[clap(long, short)]
    input: Vec<Utf8PathBuf>,

    /// Path to the output directory where the generated emoji/stickers will be put.
    /// The output files will be named after the input files using the following pattern:
    ///
    /// `{input_file_name}.emoji.webm` or `{input_file_name}.sticker.webm`
    ///
    /// If this options is not specified, the output files will be put into the same
    /// directories where the input files are located, even if they are in different
    /// directories.
    ///
    /// Make sure all input file names are unique, otherwise there will be conflicts
    /// when writing to the output directory.
    #[clap(long, short)]
    output: Option<Utf8PathBuf>,

    /// Overwrite the output files if they already exist, without asking for confirmation
    #[clap(long)]
    overwrite: bool,

    /// Set the `publisher` metadata of the generated emoji/sticker WEBM file.
    /// It is recommended to set this to the URL of the Telegram channel or other
    /// resource where emojis/stickers are promoted. This helps with keeping the
    /// source of the emoji/sticker file even if it's copied to another pack by
    /// someone else.
    #[clap(long)]
    publisher: Option<String>,

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

    /// Maximum number of inputs to be proceesed in parallel.
    #[clap(long, default_value_t = default_concurrency())]
    concurrency: NonZeroUsize,

    /// Additional arguments that will be passed to ffmpeg between the input and output args.
    /// Beware that they may break the internal logic of generating the `ffmpeg` command.
    /// For example, if you need additional video filter use `--filter` flag instead.
    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    ffmpeg_args: Vec<String>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = true)]
struct PackKindArgs {
    /// Generate an emoji WEBM file
    #[clap(long)]
    emoji: bool,

    /// Generate a sticker WEBM file
    #[clap(long)]
    sticker: bool,
}

fn default_concurrency() -> NonZeroUsize {
    MultiVideoGenContext::default_concurrency(
        " HINT: The value of concurrency may be overriden with the \
        `--concurrency` flag.",
    )
}

#[async_trait]
impl crate::cmd::Cmd for Video {
    async fn run(self) -> Result {
        let pack_kinds = [
            self.pack_kinds.emoji.then_some(PackKind::Emoji),
            self.pack_kinds.sticker.then_some(PackKind::Sticker),
        ]
        .into_iter()
        .flatten()
        .collect();

        let context = MultiVideoGenContext::builder()
            .pack_kinds(pack_kinds)
            .inputs(self.input)
            .ffmpeg_args(self.ffmpeg_args)
            .concurrency(self.concurrency)
            .overwrite(self.overwrite)
            .and_output(self.output)
            .and_begin(self.begin)
            .and_end(self.end)
            .and_filter(self.filter)
            .and_publisher(self.publisher)
            .build()?;

        context.run().await?;

        Ok(())
    }
}
