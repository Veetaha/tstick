use crate::prelude::*;
use expect_test::{ExpectFile, expect_file};

/// Version of [`expect_test::expect_file!`] macro that organizes the snapshots under
/// `tests/snapshots` folder. It automatically creates the folder if it doesn't exist.
pub(crate) async fn expect_file(file_path: &str) -> ExpectFile {
    let mut path = Utf8PathBuf::from_iter([
        &std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "tests",
        "snapshots",
    ]);

    path.push(file_path);

    let parent = path.parent().unwrap();

    fs::create_dir_all(parent)
        .await
        .expect("Failed to create a directory for test snapshots");

    expect_file![path]
}
