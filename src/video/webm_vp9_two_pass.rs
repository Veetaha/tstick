use crate::display;
use crate::ffmpeg::Ffmpeg;
use crate::prelude::*;
use crate::util::{iter, path};
use buildstructor::buildstructor;
use fs_err::tokio as fs;
use std::sync::Arc;

/// Context for running ffmpeg with two passes using VP9 encoding for webm
pub(crate) struct TwoPassContext {
    prefix_args: Vec<String>,
    ffmpeg: Arc<dyn Ffmpeg>,
    max_bytes: usize,
    /// The cached result of the best CRF found so far that fits into the `max_bytes`
    cached_best: Option<(usize, Arc<[u8]>)>,
    /// Temp dir where the log file and the output file are written
    temp_dir: tempfile::TempDir,
}

#[buildstructor]
impl TwoPassContext {
    #[builder]
    pub(crate) fn new(
        prefix_args: Vec<String>,
        ffmpeg: Arc<dyn Ffmpeg>,
        max_bytes: usize,
        temp_dir: tempfile::TempDir,
    ) -> Self {
        Self {
            prefix_args,
            ffmpeg,
            max_bytes,
            cached_best: None,
            temp_dir,
        }
    }
}

impl TwoPassContext {
    async fn run_ffmpeg(&self, trailing_args: &[&str]) -> Result<Vec<u8>> {
        let args = iter::strs(&self.prefix_args)
            .chain(iter::strs(trailing_args))
            .collect();

        self.ffmpeg.run(args).await
    }

    pub(crate) async fn run(&mut self, crf: usize) -> Result<Arc<[u8]>> {
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

        let output = self.temp_dir.path().join("output.webm");

        // Second pass
        //
        // We aren't piping the output directly to stdout, but using a temp file because
        // it influences the output somehow in such a way that the emoji generated
        // for Telegram is animated when used inside of a text message. The generated
        // webm file also has a perview on Telegram Desktop in contrast with the one
        // read from `stdout`. That's really weird... and needs deeper research
        // to understand what exactly causes such a difference.
        //
        // Note that the difference isn't observed when the emoji is sent without text
        // and thus displayed in bigger size.
        self.run_ffmpeg(&["-crf", crf_str, "-pass", "2", path::to_str(&output)?])
            .await?;

        let output = fs::read(output).await?;

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
