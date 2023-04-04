use super::{PackEntryKind, MAX_CRF};
use crate::prelude::*;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) struct SharedMockFfmpeg(Mutex<MockFfmpeg>);

#[derive(Debug)]
pub(crate) struct MockFfmpeg {
    pub(crate) crfs_ret_lens: Vec<(usize, usize)>,
    pub(crate) args_log: Vec<Vec<String>>,
    pub(crate) crfs_log: Vec<usize>,
}

impl SharedMockFfmpeg {
    fn new(crfs_ret_lens: impl IntoIterator<Item = (usize, usize)>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(MockFfmpeg {
            crfs_ret_lens: Vec::from_iter(crfs_ret_lens),
            args_log: Default::default(),
            crfs_log: Default::default(),
        })))
    }

    pub(crate) fn with_best_crf(best_crf: usize, kind: PackEntryKind) -> Arc<Self> {
        Self::new((0..=MAX_CRF).map(|crf| (crf, kind.max_bytes() + best_crf - crf)))
    }

    pub(crate) fn unwrap(self: Arc<Self>) -> MockFfmpeg {
        Arc::try_unwrap(self).unwrap().0.into_inner().unwrap()
    }
}

#[async_trait]
impl crate::ffmpeg::Ffmpeg for SharedMockFfmpeg {
    async fn run(&self, args: Vec<String>) -> Result<Vec<u8>> {
        let crf_pos = args.iter().position(|arg| arg == "-crf").unwrap();
        let crf = args[crf_pos + 1].parse().unwrap();

        let mut me = self.0.lock().unwrap();

        me.crfs_log.push(crf);
        me.args_log.push(args.clone());

        let len = me
            .crfs_ret_lens
            .iter()
            .find(|(suspect_crf, _)| *suspect_crf == crf)
            .unwrap()
            .1;

        Ok(vec![0; len as usize])
    }

    async fn run_with_output_file(
        &self,
        args: Vec<String>,
        _output_file: &Utf8Path,
    ) -> Result<Vec<u8>> {
        self.run(args).await
    }
}
