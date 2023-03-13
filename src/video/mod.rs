use anyhow::{Context, Result};
use clap::ValueEnum;
use fs_err as fs;
use itertools::Itertools;
use std::path::PathBuf;
use crate::util::cmd::{ffmpeg, get_media_duration};
use tracing::{debug, info};

const MAX_EMOJI_BYTES: usize = 64 * KIB;
const MAX_STICKER_BYTES: usize = 256 * KIB;

const EMOJI_BOUNDING_BOX: usize = 100;
const STICKER_BOUNDING_BOX: usize = 512;

/// Max value of CRF according to [the docs](https://trac.ffmpeg.org/wiki/Encode/VP9)
const MAX_CRF: usize = 63;

const KIB: usize = 1024;

#[derive(strum::Display, Debug, Clone, Copy, ValueEnum)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum StickerKind {
    Emoji,
    Sticker,
}

pub(crate) struct VideoGenContext {
    pub(crate) input: PathBuf,
    pub(crate) start_crf: usize,
    pub(crate) filter: Option<String>,
    pub(crate) ffmpeg_args: Vec<String>,
    pub(crate) sticker_kind: StickerKind,
}

impl StickerKind {
    fn max_bytes(&self) -> usize {
        match self {
            Self::Emoji => MAX_EMOJI_BYTES,
            Self::Sticker => MAX_STICKER_BYTES,
        }
    }

    fn bounding_box(&self) -> usize {
        match self {
            Self::Emoji => EMOJI_BOUNDING_BOX,
            Self::Sticker => STICKER_BOUNDING_BOX,
        }
    }
}

impl VideoGenContext {
    pub(crate) fn generate_output(&self) -> Result<()> {
        let dir = self.input.parent().with_context(|| {
            format!(
                "There is no parent directory for the input file {:?}",
                self.input
            )
        })?;

        let bytes = self.ffmpeg_vp9_two_pass()?;

        let file = dir.join(format!("{}.webm", self.sticker_kind));

        fs::write(&file, bytes)?;

        info!("📄 Generated {}", file.display());

        Ok(())
    }

    fn ffmpeg_vp9_two_pass(&self) -> Result<Vec<u8>> {
        // We need to make sure the image fits into the bounding box.
        // The scale filter expression is inspired by this answer:
        // https://superuser.com/a/547406
        //
        // FIXME: return an error if we are generating an emoji, but
        // the input's dimensions are not square.
        let ultimate_scale = format!(
            "scale=\
            iw * min({out} / iw\\, {out} / ih):\
            ih * min({out} / iw\\, {out} / ih):\
            flags=lanczos",
            out = self.sticker_kind.bounding_box()
        );

        let video_filter = self.filter.iter().chain([&ultimate_scale]).join(",");

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
            // Audio streams must be removed from the output
            "-an",
            "-filter:v",
            &video_filter,
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

        let max_bytes = self.sticker_kind.max_bytes();

        let max_bytes_display = crate::display::human_size(max_bytes);

        let bitrate = max_bytes /

        // for crf in self.start_crf..MAX_CRF {
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
    // }
}
