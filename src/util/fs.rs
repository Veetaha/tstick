use crate::prelude::*;
use futures::prelude::*;
use std::path::{Path, PathBuf};

/// Returns either a stream of files in the directory or a stream of a single
/// file depending on whether the path is a directory or a file.
pub(crate) async fn files(path: &Path) -> Result<Vec<PathBuf>> {
    if !fs::metadata(path).await?.is_dir() {
        return Ok(vec![path.to_owned()]);
    }

    let dir = fs::read_dir(path).await?;
    read_dir_stream(dir)
        .map_ok(|entry| entry.path())
        .try_collect()
        .await
}

fn read_dir_stream(dir: fs::ReadDir) -> impl futures::Stream<Item = Result<fs::DirEntry>> {
    stream::unfold(dir, |mut dir| async move {
        dir.next_entry()
            .err_into()
            .await
            .transpose()
            .map(|entry| (entry, dir))
    })
}
