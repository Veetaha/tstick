use super::webm_vp9_two_pass::TwoPassContext;
use super::{PackEntryKind, MAX_CRF};
use crate::display;
use crate::ffmpeg::Ffmpeg;
use crate::prelude::*;
use crate::util::iter;
use crate::util::path::Utf8StemmedPathBuf;
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct SingleVideoGenOptions {
    pub(crate) begin: Option<Duration>,
    pub(crate) end: Option<Duration>,

    pub(crate) filter: Option<String>,
    pub(crate) ffmpeg_args: Vec<String>,
    pub(crate) ffmpeg: Arc<dyn Ffmpeg>,

    pub(crate) publisher: Option<String>,
}

pub(crate) struct SingleVideoGenContext {
    // It's theoreically possible to replace this `Arc` with a bare shared reference
    // but there is a bug in rust compiler that prevents it from working.
    // We would stumble with errors like this:
    // - higher-ranked lifetime error, could not prove {Pin<Box<big_future_type>>: constraint}
    // - implementation of `std::marker::Send` is not general enough
    // - implementation of `FnOnce` is not general enough
    //
    // See: https://github.com/rust-lang/rust/issues/102211
    pub(crate) options: Arc<SingleVideoGenOptions>,
    pub(crate) pack_entry_kind: PackEntryKind,
    pub(crate) input: Utf8StemmedPathBuf,
    pub(crate) output: Utf8PathBuf,
}

impl SingleVideoGenContext {
    #[instrument(
        name = "gen",
        skip_all,
        fields(
            pack = %self.pack_entry_kind,
            input = %self.input.as_path(),
        )
    )]
    pub(crate) async fn generate_file(self) -> Result {
        let output = self.output.clone();
        let bytes = self.generate_bytes().await?;

        fs::write(&output, bytes).await?;

        let out_file = nu_ansi_term::Color::Magenta.bold().paint(output.as_str());

        info!("🔥 Saved output at {out_file}");

        Ok(())
    }

    pub(crate) async fn generate_bytes(self) -> Result<Arc<[u8]>> {
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

        let prefix_args = iter::strs(["-y", "-i", self.input.as_path().as_str()])
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
    use super::PackEntryKind;
    use super::*;
    use crate::util::path::Utf8StemmedPathBuf;
    use crate::video::testing::SharedMockFfmpeg;
    use expect_test::{expect, Expect};
    use std::sync::Arc;

    #[test_log::test(tokio::test)]
    async fn smoke_test_binary_crf_search() {
        assert_crf_search(0, expect!["[31, 15, 7, 3, 1, 0]"]).await;
        assert_crf_search(1, expect!["[31, 15, 7, 3, 1, 0]"]).await;
        assert_crf_search(31, expect!["[31, 15, 23, 27, 29, 30]"]).await;
        assert_crf_search(62, expect!["[31, 47, 55, 59, 61, 62]"]).await;
        assert_crf_search(63, expect!["[31, 47, 55, 59, 61, 62, 63]"]).await;
    }

    async fn assert_crf_search(best_crf: usize, snap: Expect) {
        let pack_entry_kind = PackEntryKind::Sticker;

        let mock_ffmpeg = SharedMockFfmpeg::with_best_crf(best_crf, pack_entry_kind);
        let options = SingleVideoGenOptions {
            begin: None,
            end: None,
            filter: None,
            ffmpeg_args: vec![],
            ffmpeg: mock_ffmpeg.clone(),
            publisher: None,
        };

        let ctx = SingleVideoGenContext {
            options: Arc::new(options),
            pack_entry_kind: pack_entry_kind,
            input: Utf8StemmedPathBuf::try_from(Utf8PathBuf::from("input")).unwrap(),
            output: Utf8PathBuf::from("output"),
        };

        let output = ctx.generate_bytes().await.unwrap();

        assert_eq!(output.len(), pack_entry_kind.max_bytes());

        let mock_ffmpeg = mock_ffmpeg.unwrap();

        // There must be two invocations per crf because we do two passes
        let invocations = mock_ffmpeg.crfs_log.iter().tuples().map(|(a, b)| {
            assert_eq!(a, b);
            a
        });

        let actual = invocations.collect::<Vec<_>>();

        snap.assert_eq(&format!("{actual:?}"));
    }
}
