mod webm_vp9_two_pass;

use crate::ffmpeg::Ffmpeg;
use crate::prelude::*;
use crate::util::duration::DurationExt;
use crate::util::{iter, path};
use crate::{display, util};
use anyhow::bail;
use buildstructor::buildstructor;
use futures::prelude::*;
use itertools::Itertools;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use webm_vp9_two_pass::TwoPassContext;

const MAX_EMOJI_BYTES: usize = 64 * KIB;
const MAX_STICKER_BYTES: usize = 256 * KIB;

const EMOJI_BOUNDING_BOX: u64 = 100;
const STICKER_BOUNDING_BOX: u64 = 512;

/// Max value of CRF according to [the docs](https://trac.ffmpeg.org/wiki/Encode/VP9)
const MAX_CRF: usize = 63;

const KIB: usize = 1024;

#[derive(strum::Display, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum PackEntryKind {
    Emoji,
    Sticker,
}

impl PackEntryKind {
    fn max_bytes(&self) -> usize {
        match self {
            Self::Emoji => MAX_EMOJI_BYTES,
            Self::Sticker => MAX_STICKER_BYTES,
        }
    }

    /// Telegram supports rectangle stickers, but not emojis.
    fn must_be_square(&self) -> bool {
        match self {
            Self::Emoji => true,
            Self::Sticker => false,
        }
    }

    fn bounding_box(&self) -> u64 {
        match self {
            Self::Emoji => EMOJI_BOUNDING_BOX,
            Self::Sticker => STICKER_BOUNDING_BOX,
        }
    }
}

pub(crate) struct MultiVideoGenContext {
    pack_entry_kinds: Vec<PackEntryKind>,

    inputs: Vec<PathBuf>,
    output: Option<PathBuf>,

    concurrency: NonZeroUsize,

    overwrite: bool,
    options: Arc<SingleVideoGenOptions>,
}

#[buildstructor]
impl MultiVideoGenContext {
    #[builder]
    pub(crate) fn new(
        pack_entry_kinds: Vec<PackEntryKind>,

        inputs: Vec<PathBuf>,
        output: Option<PathBuf>,

        begin: Option<Duration>,
        end: Option<Duration>,

        filter: Option<String>,
        ffmpeg_args: Vec<String>,
        ffmpeg: Option<Arc<dyn Ffmpeg>>,

        concurrency: Option<NonZeroUsize>,
        overwrite: bool,
        publisher: Option<String>,
    ) -> Result<Self> {
        if pack_entry_kinds.is_empty() {
            bail!("No pack kinds were specified");
        }

        if !pack_entry_kinds.iter().all_unique() {
            bail!("Duplicate pack kinds found, but they must be unique: {pack_entry_kinds:?}");
        }

        let options = SingleVideoGenOptions {
            begin,
            end,
            filter,
            ffmpeg_args,
            ffmpeg: ffmpeg.unwrap_or_else(|| Arc::new(crate::ffmpeg::FfmpegProcess)),
            publisher,
        };

        Ok(Self {
            pack_entry_kinds,
            inputs,
            output,
            options: Arc::new(options),
            overwrite,
            concurrency: concurrency.unwrap_or_else(|| Self::default_concurrency("")),
        })
    }

    pub(crate) fn default_concurrency(err_suffix: &str) -> NonZeroUsize {
        std::thread::available_parallelism().unwrap_or_else(|err| {
            let default = NonZeroUsize::new(1).unwrap();
            warn!(
                err = &err as &dyn std::error::Error,
                "Failed to query the system's available parallelism. \
                Falling back to the default value of {default}.{err_suffix}",
            );
            default
        })
    }
}

impl MultiVideoGenContext {
    async fn contexts_for_pack_entry_kind(
        &self,
        pack_entry_kind: PackEntryKind,
    ) -> Result<Vec<SingleVideoGenContext>> {
        // This hack with `cloned()` is needed due to a compiler bug (rust/issues/102211)
        stream::iter(self.inputs.iter().cloned())
            .map(|input| async move { util::fs::files(&input).await })
            .buffer_unordered(10)
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten()
            .map(move |input| {
                let output = self.out_file(pack_entry_kind, &input)?;
                Ok(SingleVideoGenContext {
                    options: self.options.clone(),
                    pack_entry_kind,
                    input: input.clone(),
                    output,
                })
            })
            .collect()
    }

    pub(crate) async fn run(self) -> Result {
        let contexts: Vec<_> = future::try_join_all(
            self.pack_entry_kinds
                .iter()
                .map(|&pack_entry_kind| self.contexts_for_pack_entry_kind(pack_entry_kind)),
        )
        .await?;
        let contexts = contexts.into_iter().flatten().collect::<Vec<_>>();

        let existing_files: Vec<_> = contexts
            .iter()
            .filter_map(|ctx| {
                ctx.output
                    .try_exists()
                    .with_context(|| {
                        format!(
                            "Failed to check if the output file exists: `{}`",
                            ctx.output.display()
                        )
                    })
                    .map(|exists| exists.then_some(&ctx.output))
                    .transpose()
            })
            .try_collect()?;

        if !existing_files.is_empty() {
            let files = existing_files.iter().format_with("\n", |path, f| {
                f(&format_args!("- {}", display::bold(&path.display())))
            });

            let message = format!("The following files already exist:\n{files}\nOverwrite them?",);

            crate::util::input::read_confirmation(&message, self.overwrite).await?;
        }

        stream::iter(contexts)
            .enumerate()
            .map(|(id, context)| {
                context
                    .generate_file()
                    .instrument(info_span!("task", id = id + 1))
            })
            .buffer_unordered(self.concurrency.get())
            .try_collect::<Vec<()>>()
            .await?;

        Ok(())
    }

    fn out_file(&self, pack_entry_kind: PackEntryKind, input: &Path) -> Result<PathBuf> {
        let out_dir = self.output.as_deref().map(Ok).unwrap_or_else(|| {
            input.parent().with_context(|| {
                format!(
                    "There is no parent directory for the input file {}",
                    input.display()
                )
            })
        })?;

        Ok(out_dir.join(format!("{}.webm", pack_entry_kind)))
    }
}

struct SingleVideoGenOptions {
    begin: Option<Duration>,
    end: Option<Duration>,

    filter: Option<String>,
    ffmpeg_args: Vec<String>,
    ffmpeg: Arc<dyn Ffmpeg>,

    publisher: Option<String>,
}

struct SingleVideoGenContext {
    // It's theoreically possible to replace this `Arc` with a bare shared reference
    // but there is a bug in rust compiler that prevents it from working.
    // We would stumble with errors like this:
    // - higher-ranked lifetime error, could not prove {Pin<Box<big_future_type>>: constraint}
    // - implementation of `std::marker::Send` is not general enough
    // - implementation of `FnOnce` is not general enough
    //
    // See: https://github.com/rust-lang/rust/issues/102211
    options: Arc<SingleVideoGenOptions>,
    pack_entry_kind: PackEntryKind,
    input: PathBuf,
    output: PathBuf,
}

impl SingleVideoGenContext {
    #[instrument(
        name = "gen",
        skip_all,
        fields(
            pack = %self.pack_entry_kind,
            input = %self.input.display(),
        )
    )]
    async fn generate_file(self) -> Result {
        let output = self.output.clone();
        let bytes = self.generate_bytes().await?;

        fs::write(&output, bytes).await?;

        let out_file = nu_ansi_term::Color::Magenta
            .bold()
            .paint(path::to_str(&output)?);

        info!("🔥 Saved output at {out_file}");

        Ok(())
    }

    async fn generate_bytes(self) -> Result<Arc<[u8]>> {
        let start = std::time::Instant::now();

        let mut min = 0;
        let mut max = MAX_CRF;

        let max_bytes = self.pack_entry_kind.max_bytes();

        let max_bytes_display = &display::bold_human_size(max_bytes);

        info!("🚀 Trying to find best CRF to fit into {max_bytes_display}");

        let mut two_pass = self.two_pass_context()?;
        let mut i = 0u32;

        let (crf, output) = loop {
            let mid = (min + max) / 2;
            debug!(max, min, "Bounds");

            // Estimate the progreess as the ratio between the current iteration
            // and the total maximum number of iterations, which are the for the
            // binary search the log(N) and + 1 because the search isn't exact.
            // We search for an index where the generated file size becomes
            // greater than the limit, so for example if crf could take only two
            // values `[0, 1]`, then we would always need to do 2 iterations.
            // even though `log2(2) == 1`
            let percent = (f64::from(i) / (((MAX_CRF + 1) as f64).log2() + 1.0)) * 100.0;
            i += 1;

            let output = two_pass
                .run(mid)
                .instrument(info_span!(
                    "progress",
                    percent = format_args!("{percent:.1}%")
                ))
                .await?;

            // Repeat until we have a range of 1 value
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
        Ok(output)
    }

    fn two_pass_context(self) -> Result<TwoPassContext> {
        let temp_dir = tempfile::tempdir()?;
        let pass_log_file = temp_dir
            .path()
            .join("ffmpeg2pass")
            .to_string_lossy()
            .into_owned();

        let max_side = self.pack_entry_kind.bounding_box();

        let ultimate_padding = self
            .pack_entry_kind
            .must_be_square()
            .then(|| format!("pad={max_side}:{max_side}:-1:-1:color=0x00000000"));

        // We need to make sure the image fits into the bounding box.
        // The scale filter expression is inspired by this answer:
        // https://superuser.com/a/547406

        // We pad the image with transparent pixels to make it fit into
        // the bounding box exactly for emoji
        let ultimate_scale = format!(
            "scale=\
            iw * min({max_side} / iw\\, {max_side} / ih):\
            ih * min({max_side} / iw\\, {max_side} / ih):\
            flags=lanczos"
        );

        let video_filter = self
            .options
            .filter
            .iter()
            .chain([&ultimate_scale])
            .chain(&ultimate_padding)
            .join(",");

        let publisher = optional_named_arg(
            "-metadata",
            self.options
                .publisher
                .as_deref()
                .map(|publisher| format!("publisher={publisher}")),
        );

        let prefix_args = iter::strs(["-y", "-i", path::to_str(&self.input)?])
            .chain(optional_named_duration_arg("-ss", self.options.begin))
            .chain(optional_named_duration_arg("-to", self.options.end))
            .chain(publisher)
            .chain(iter::strs([
                "-metadata",
                "encoded_by=https://github.com/Veetaha/tstick",
                "-fps_mode",
                "passthrough",
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
            .chain(self.options.ffmpeg_args.iter().cloned())
            .collect();

        Ok(TwoPassContext::builder()
            .prefix_args(prefix_args)
            .ffmpeg(self.options.ffmpeg.clone())
            .max_bytes(self.pack_entry_kind.max_bytes())
            .temp_dir(temp_dir)
            .build())
    }
}

fn optional_named_duration_arg(
    name: &str,
    bound: Option<Duration>,
) -> impl Iterator<Item = String> + '_ {
    let bound = bound.map(|duration| duration.to_secs_f64().to_string());
    optional_named_arg(name, bound)
}

fn optional_named_arg(name: &str, option: Option<String>) -> impl Iterator<Item = String> + '_ {
    option
        .into_iter()
        .flat_map(move |value| [name.to_owned(), value])
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

        let ctx = MultiVideoGenContext {
            input: "foo.mp4".into(),
            begin: None,
            end: None,
            filter: None,
            ffmpeg_args: vec![],
            ffmpeg: mock_ffmpeg.clone(),
            pack_entry_kinds: PackEntryKind::Sticker,
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
