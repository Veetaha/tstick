use crate::prelude::*;
use anyhow::Result;
use async_trait::async_trait;
use std::fmt;

#[async_trait]
pub(crate) trait Ffmpeg: fmt::Debug + Send + Sync {
    /// Invoke ffmpeg process with the given arguments.
    async fn run(&self, args: Vec<String>) -> Result<Vec<u8>>;

    /// Same as [`Self::run`], but expects automatically appends the
    /// output path to the arguments and returns the contents of the
    /// file at that path.
    ///
    /// This is useful for mocking to avoid reading files from disk,
    /// especially when they aren't written by the mock.
    async fn run_with_output_file(
        &self,
        args: Vec<String>,
        output_file: &Utf8Path,
    ) -> Result<Vec<u8>> {
        let mut args = args;
        args.push(output_file.to_string());

        self.run(args).await?;

        fs::read(output_file).await.err_into()
    }
}

#[derive(Debug)]
pub(crate) struct FfmpegProcess;

#[async_trait]
impl Ffmpeg for FfmpegProcess {
    async fn run(&self, args: Vec<String>) -> Result<Vec<u8>> {
        crate::util::cmd::ffmpeg(args).await
    }
}
