use crate::prelude::*;
use easy_ext::ext;
use std::path::Path;

#[ext(PathExt)]
pub(crate) impl Path {
    fn unwrap_utf8(&self) -> &Utf8Path {
        Utf8Path::from_path(self).unwrap_or_else(|| panic!("BUG: Path is not UTF8: {self:?}"))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Utf8StemmedPathBuf(Utf8PathBuf);

impl TryFrom<Utf8PathBuf> for Utf8StemmedPathBuf {
    type Error = anyhow::Error;

    fn try_from(value: Utf8PathBuf) -> Result<Self> {
        value
            .file_stem()
            .with_context(|| format!("Path has no file stem: {value:?}"))?;

        Ok(Self(value))
    }
}

impl Utf8StemmedPathBuf {
    pub(crate) fn file_stem(&self) -> &str {
        self.0.file_stem().unwrap()
    }

    pub(crate) fn as_path(&self) -> &Utf8Path {
        self.0.as_path()
    }
}
