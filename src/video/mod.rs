use crate::display;
use crate::ffmpeg::Ffmpeg;
use crate::util::duration::DurationExt;
use crate::util::iter;
use anyhow::{bail, Context, Ok, Result};
use clap::ValueEnum;
use fs_err as fs;
use itertools::Itertools;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

const MAX_EMOJI_BYTES: usize = 64 * KIB;
const MAX_STICKER_BYTES: usize = 256 * KIB;

const EMOJI_BOUNDING_BOX: u64 = 100;
const STICKER_BOUNDING_BOX: u64 = 512;

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
    pub(crate) sticker_kind: StickerKind,
    pub(crate) input: PathBuf,

    pub(crate) begin: Option<Duration>,
    pub(crate) end: Option<Duration>,

    pub(crate) filter: Option<String>,
    pub(crate) ffmpeg_args: Vec<String>,

    pub(crate) ffmpeg: Arc<dyn Ffmpeg>,
}

impl StickerKind {
    fn max_bytes(&self) -> usize {
        match self {
            Self::Emoji => MAX_EMOJI_BYTES,
            Self::Sticker => MAX_STICKER_BYTES,
        }
    }

    fn bounding_box(&self) -> u64 {
        match self {
            Self::Emoji => EMOJI_BOUNDING_BOX,
            Self::Sticker => STICKER_BOUNDING_BOX,
        }
    }
}

impl VideoGenContext {
    pub(crate) async fn generate_output(self) -> Result<()> {
        let dir = self.input.parent().with_context(|| {
            format!(
                "There is no parent directory for the input file {:?}",
                self.input
            )
        })?;

        let out_file = dir.join(format!("{}.webm", self.sticker_kind));

        let bytes = self.ffmpeg_vp9_two_pass().await?;

        fs::write(&out_file, bytes)?;

        let out_file = nu_ansi_term::Color::Magenta
            .bold()
            .paint(out_file.to_string_lossy());

        info!("🔥 Saved output at {out_file}");

        Ok(())
    }

    async fn ffmpeg_vp9_two_pass(self) -> Result<Arc<[u8]>> {
        let start = std::time::Instant::now();

        let mut min = 0;
        let mut max = MAX_CRF;

        let max_bytes = self.sticker_kind.max_bytes();

        let max_bytes_display = &display::bold_human_size(max_bytes);

        info!("🚀 Trying to find best CRF to fit into {max_bytes_display}");

        let mut two_pass = self.two_pass_context()?;

        let (crf, output) = loop {
            let mid = (min + max) / 2;
            debug!(max, min, "Bounds");

            let output = two_pass.run(mid).await?;

            // Repeat until we have a range of 1 or 2 values
            if min == max {
                break (mid, output);
            }

            if output.len() <= max_bytes {
                // This is the candidate for the ultimate output, because it fits,
                // so the range includes it
                max = mid;
            } else {
                // `mid` can not possibly fit into the limits, so the range is
                // moved to the right of `mid`
                min = mid + 1;
            }
        };

        let crf = display::bold(&crf);

        if output.len() > max_bytes {
            let size_display = display::bold_human_size(output.len());
            let msg = format!(
                "The output can't possibly fit into the limit of {max_bytes_display}. \
                The minimum generated file size with CRF {crf} is {size_display}",
            );
            debug!("{msg}");
            bail!("{msg}");
        }

        let size_display = display::bold_human_size(output.len());

        let elapsed = display::elpased(start);

        info!("🎉 Found a fitting CRF {crf}, which generates {size_display} in {elapsed}");
        return Ok(output);
    }

    fn two_pass_context(self) -> Result<TwoPassContext> {
        let log_file_dir = tempfile::tempdir()?;
        let pass_log_file = log_file_dir
            .path()
            .join("ffmpeg2pass")
            .to_string_lossy()
            .into_owned();

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

        let prefix_args = iter::strs(["-y", "-i"])
            .chain([self.input.to_string_lossy().into_owned()])
            .chain(time_bound_args("-ss", self.begin))
            .chain(time_bound_args("-to", self.end))
            .chain(iter::strs([
                "-vcodec",
                "libvpx-vp9",
                // From the docs: constant quality 2-pass is invoked by setting
                // -b:v to zero and specifiying a quality level using the -crf switch
                "-b:v",
                "0",
                // Audio streams must be removed from the output
                "-an",
                "-filter:v",
            ]))
            .chain([video_filter])
            .chain(iter::strs(["-passlogfile"]))
            .chain([pass_log_file])
            .chain(self.ffmpeg_args)
            .collect();

        Ok(TwoPassContext {
            prefix_args,
            ffmpeg: self.ffmpeg.clone(),
            max_bytes: self.sticker_kind.max_bytes(),
            cached_best: None,
            _log_file_dir: log_file_dir,
        })
    }
}

/// Context for running ffmpeg with two passes using VP9 encoding for webm
struct TwoPassContext {
    prefix_args: Vec<String>,
    ffmpeg: Arc<dyn Ffmpeg>,
    max_bytes: usize,
    /// The cached result of the best CRF found so far that fits into the `max_bytes`
    cached_best: Option<(usize, Arc<[u8]>)>,
    _log_file_dir: tempfile::TempDir,
}

impl TwoPassContext {
    async fn run_ffmpeg(&self, trailing_args: &[&str]) -> Result<Vec<u8>> {
        let args = iter::strs(&self.prefix_args)
            .chain(iter::strs(trailing_args))
            .collect();

        self.ffmpeg.run(args).await
    }

    async fn run(&mut self, crf: usize) -> Result<Arc<[u8]>> {
        if let Some((cached_crf, cached_output)) = &self.cached_best {
            if *cached_crf == crf {
                debug!(
                    %crf,
                    size = %display::human_size(cached_output.len()),
                    "Using cached output"
                );
                return Ok(cached_output.clone());
            }
        }

        let crf_str = &crf.to_string();

        let null_output = if cfg!(windows) { "NUL" } else { "/dev/null" };

        let start = std::time::Instant::now();

        // First pass
        self.run_ffmpeg(&["-crf", crf_str, "-pass", "1", "-f", "null", null_output])
            .await?;

        // Second pass
        //
        // `pipe:1` instructs ffmpeg not to save the output to a file, but instead
        // write it to the stdout of the process
        //
        // Because we don't specify the file name, but instead use the stdout then
        // `ffmpeg` can no longer infer the output format from the file extension,
        // so we have to pass `-f webm` to specify that separately explicitly
        let output = self
            .run_ffmpeg(&["-crf", crf_str, "-pass", "2", "-f", "webm", "pipe:1"])
            .await?;

        let elapsed = display::elpased(start);

        let output = Arc::<[_]>::from(output);

        let (checkbox, color) = if output.len() > self.max_bytes {
            ('❌', nu_ansi_term::Color::Red)
        } else {
            let generated_better_cache = matches!(
                self.cached_best, Some((cached_crf, _)) if crf < cached_crf
            );

            if self.cached_best.is_none() || generated_better_cache {
                self.cached_best = Some((crf, output.clone()));
            }

            ('✅', nu_ansi_term::Color::Green)
        };

        let size_display = color.bold().paint(display::human_size(output.len()));

        info!(
            "{checkbox} CRF {} generated {size_display} in {elapsed}",
            display::bold(&crf)
        );

        Ok(output)
    }
}

fn time_bound_args(name: &str, bound: Option<Duration>) -> impl Iterator<Item = String> + '_ {
    bound
        .into_iter()
        .flat_map(|duration| [name.to_owned(), duration.to_secs_f64().to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use expect_test::{expect, Expect};
    use itertools::Itertools;
    use std::sync::Mutex;

    #[test_log::test(tokio::test)]
    async fn smoke_test_binary_crf_search() {
        assert_crf_search(0, expect!["[31, 15, 7, 3, 1, 0]"]).await;
        assert_crf_search(1, expect!["[31, 15, 7, 3, 1, 0]"]).await;
        assert_crf_search(31, expect!["[31, 15, 23, 27, 29, 30]"]).await;
        assert_crf_search(62, expect!["[31, 47, 55, 59, 61, 62]"]).await;
        assert_crf_search(63, expect!["[31, 47, 55, 59, 61, 62, 63]"]).await;
    }

    #[derive(Debug)]
    struct MockFfmpeg {
        crfs_ret_lens: Vec<(usize, usize)>,
        crfs_log: Mutex<Vec<usize>>,
    }

    #[async_trait]
    impl crate::ffmpeg::Ffmpeg for MockFfmpeg {
        async fn run(&self, args: Vec<String>) -> Result<Vec<u8>> {
            let crf_pos = args.iter().position(|arg| arg == "-crf").unwrap();
            let crf = args[crf_pos + 1].parse().unwrap();
            self.crfs_log.lock().unwrap().push(crf);

            let len = self
                .crfs_ret_lens
                .iter()
                .find(|(suspect_crf, _)| *suspect_crf == crf)
                .unwrap()
                .1;

            Ok(vec![0; len * KIB as usize])
        }
    }

    async fn assert_crf_search(best_crf: usize, snap: Expect) {
        let crfs_lens = (0..=MAX_CRF).map(|crf| (crf, MAX_STICKER_BYTES / KIB + best_crf - crf));
        assert_crf_search_imp(crfs_lens, snap).await;
    }

    async fn assert_crf_search_imp(
        crfs_lens: impl IntoIterator<Item = (usize, usize)>,
        snap: Expect,
    ) {
        let lens = Vec::from_iter(crfs_lens);

        let mock_ffmpeg = Arc::new(MockFfmpeg {
            crfs_ret_lens: lens,
            crfs_log: Default::default(),
        });

        let ctx = VideoGenContext {
            input: "foo.mp4".into(),
            begin: None,
            end: None,
            filter: None,
            ffmpeg_args: vec![],
            ffmpeg: mock_ffmpeg.clone(),
            sticker_kind: StickerKind::Sticker,
        };

        let output = ctx.ffmpeg_vp9_two_pass().await.unwrap();

        assert_eq!(output.len(), MAX_STICKER_BYTES);

        let mock_ffmpeg = Arc::try_unwrap(mock_ffmpeg).unwrap();

        // There must be two invocations per crf because we do two passes
        let invocations = mock_ffmpeg.crfs_log.into_inner().unwrap();
        let invocations = invocations.iter().tuples().map(|(a, b)| {
            assert_eq!(a, b);
            a
        });

        let actual = invocations.collect::<Vec<_>>();

        snap.assert_eq(&format!("{actual:?}"));
    }
}
