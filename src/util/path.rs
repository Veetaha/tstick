use anyhow::{Context, Result};
use std::path::Path;

pub(crate) fn to_str(path: &Path) -> Result<&str> {
    path.to_str().with_context(|| {
        format!("Stumbled with a non-UTF8 path, which is not supported:\n{path:?}")
    })
}
