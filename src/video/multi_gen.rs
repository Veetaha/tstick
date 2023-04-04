use super::single_gen::{SingleVideoGenContext, SingleVideoGenOptions};
use super::PackEntryKind;
use crate::ffmpeg::Ffmpeg;
use crate::prelude::*;
use crate::util::path::Utf8StemmedPathBuf;
use buildstructor::buildstructor;
use futures::prelude::*;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct MultiVideoGenContext {
    pack_entry_kinds: Vec<PackEntryKind>,

    inputs: Vec<Utf8PathBuf>,
    output: Option<Utf8PathBuf>,

    concurrency: NonZeroUsize,

    overwrite: bool,
    options: Arc<SingleVideoGenOptions>,
}

#[buildstructor]
impl MultiVideoGenContext {
    #[builder]
    pub(crate) fn new(
        pack_entry_kinds: Vec<PackEntryKind>,

        inputs: Vec<Utf8PathBuf>,
        output: Option<Utf8PathBuf>,

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
    fn contexts_for_pack_entry_kind(
        &self,
        inputs: &[Utf8StemmedPathBuf],
        pack_entry_kind: PackEntryKind,
    ) -> Result<Vec<SingleVideoGenContext>> {
        // This hack with `cloned()` is needed due to a compiler bug (rust/issues/102211)
        inputs
            .iter()
            .map(move |input| {
                let output = self.out_file(pack_entry_kind, input.as_path())?;
                Ok(SingleVideoGenContext {
                    options: self.options.clone(),
                    pack_entry_kind,
                    input: input.clone(),
                    output,
                })
            })
            .collect()
    }

    async fn input_files(&self) -> Result<Vec<Utf8StemmedPathBuf>> {
        stream::iter(self.inputs.iter().cloned())
            .map(crate::fs::files)
            .buffer_unordered(10)
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten()
            .map(TryInto::try_into)
            .try_collect()
    }

    pub(crate) async fn run(self) -> Result {
        let input_files = self.input_files().await?;

        crate::fs::validate_duplicate_input_names(&input_files)?;

        let contexts: Vec<_> = self
            .pack_entry_kinds
            .iter()
            .map(|&kind| self.contexts_for_pack_entry_kind(&input_files, kind))
            .flatten_ok()
            .try_collect()?;

        crate::fs::validate_output_files_overwriting(
            self.overwrite,
            contexts.iter().map(|ctx| ctx.output.clone()),
        )
        .await?;

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

    fn out_file(&self, pack_entry_kind: PackEntryKind, input: &Utf8Path) -> Result<Utf8PathBuf> {
        let out_dir = self.output.as_deref().map(Ok).unwrap_or_else(|| {
            input.parent().with_context(|| {
                format!("There is no parent directory for the input file {}", input)
            })
        })?;

        let file_name = input
            .file_stem()
            .with_context(|| format!("Input must have a file name, but got `{input:?}`"))?;

        Ok(out_dir.join(format!("{file_name}-{pack_entry_kind}.webm")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing;
    use crate::video::testing::SharedMockFfmpeg;
    use lazy_regex::regex_replace;

    #[test_log::test(tokio::test)]
    async fn smoke_test() {
        FfmpegCall::builder()
            .expected("smoke_all_options")
            .begin(Duration::from_secs_f64(1.5))
            .end(Duration::from_secs_f64(2.5))
            .filter("custom_filter")
            .ffmpeg_arg("custom_ffmpeg_arg")
            .publisher("custom publisher")
            .assert()
            .await;
    }

    struct FfmpegCall;

    #[buildstructor]
    impl FfmpegCall {
        #[builder(exit = "assert")]
        async fn new(
            expected: String,

            begin: Option<Duration>,
            end: Option<Duration>,

            filter: Option<String>,
            ffmpeg_args: Vec<String>,

            publisher: Option<String>,
        ) {
            let input = tempfile::NamedTempFile::new().unwrap().into_temp_path();
            fs::write(&input, "hello").await.unwrap();

            let pack_entry_kind = PackEntryKind::Emoji;

            let mock_ffmpeg = SharedMockFfmpeg::with_best_crf(25, pack_entry_kind);

            let ctx = MultiVideoGenContext::builder()
                .input(Utf8PathBuf::try_from(input.to_path_buf()).unwrap())
                .pack_entry_kind(pack_entry_kind)
                .overwrite(false)
                .ffmpeg(mock_ffmpeg.clone());

            let ctx = ctx
                .and_begin(begin)
                .and_end(end)
                .and_filter(filter)
                .ffmpeg_args(ffmpeg_args)
                .and_publisher(publisher)
                .build()
                .unwrap();

            ctx.run().await.unwrap();

            let mut ffmpeg_call = mock_ffmpeg.unwrap().args_log.into_iter().next().unwrap();

            // Sanitize the random temp directory path
            for arg in &mut ffmpeg_call {
                *arg = regex_replace!(r".*\.tmp\w*(?:(?:\W)(.*))?", arg, |_, rest| format!(
                    "{{temp_dir}}/{rest}"
                ))
                .into_owned();
            }

            let ffmpeg_call = ffmpeg_call.iter().join("\n");

            let expected = testing::expect_file(&format!("ffmpeg_calls/{expected}.txt")).await;

            expected.assert_eq(&ffmpeg_call);
        }
    }
}
