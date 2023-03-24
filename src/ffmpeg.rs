use anyhow::Result;
use async_trait::async_trait;
use std::fmt;

#[async_trait]
pub(crate) trait Ffmpeg: fmt::Debug + Send + Sync {
    async fn run(&self, args: Vec<String>) -> Result<Vec<u8>>;
}

#[derive(Debug)]
pub(crate) struct FfmpegProcess;

#[async_trait]
impl Ffmpeg for FfmpegProcess {
    async fn run(&self, args: Vec<String>) -> Result<Vec<u8>> {
        crate::util::cmd::ffmpeg(args).await
    }
}
