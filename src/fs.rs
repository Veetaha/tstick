use crate::display;
use crate::prelude::*;
use crate::util::input;
use crate::util::path::Utf8StemmedPathBuf;
use futures::prelude::*;

/// Returns either a stream of files in the directory or a stream of a single
/// file depending on whether the path is a directory or a file.
pub(crate) async fn files(path: impl AsRef<Utf8Path>) -> Result<Vec<Utf8PathBuf>> {
    let path = path.as_ref();

    if !fs::metadata(path).await?.is_dir() {
        return Ok(vec![path.to_owned()]);
    }

    let dir = fs::read_dir(path).await?;

    read_dir_stream(dir)
        .map(|entry| entry?.path().try_into().err_into())
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

pub(crate) async fn validate_output_files_overwriting(
    overwrite: bool,
    paths: impl IntoIterator<Item = Utf8PathBuf>,
) -> Result {
    let existing_files: Vec<_> = paths
        .into_iter()
        .filter_map(|path| {
            path.try_exists()
                .with_context(|| format!("Failed to check if the output file exists: `{path}`"))
                .map(|exists| exists.then_some(path))
                .transpose()
        })
        .try_collect()?;

    if existing_files.is_empty() {
        return Ok(());
    }

    let files = existing_files.iter().format_with("\n", |path, f| {
        f(&format_args!("- {}", display::bold(&path)))
    });

    let message = format!("The following output files already exist.\n{files}\nOverwrite them?");

    input::read_confirmation(&message, overwrite).await?;

    Ok(())
}

pub(crate) fn validate_duplicate_input_names<'a>(
    inputs: impl IntoIterator<Item = &'a Utf8StemmedPathBuf>,
) -> Result {
    let mut duplicates = inputs
        .into_iter()
        .into_group_map_by(|path| path.file_stem())
        .into_iter()
        .filter(|(_, paths)| paths.len() >= 2)
        // Sort to make the test snapshots stable
        .sorted_by_key(|(stem, _)| *stem)
        .peekable();

    if duplicates.peek().is_none() {
        return Ok(());
    }

    let inputs = duplicates.format_with("\n", |(stem, contexts), f| {
        let paths = contexts.iter().map(|path| path.as_path()).format(", ");
        let len = contexts.len();
        f(&format_args!("- {stem} ({len} files): [{paths}]"))
    });

    bail!("The following input files have the same name, but they must be unique.\n{inputs}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::{expect, Expect};

    fn assert_validate_duplicate_input_names(inputs: &[&str], expected: Expect) {
        let inputs = inputs
            .iter()
            .map(|path| Utf8StemmedPathBuf::try_from(Utf8PathBuf::from(path)).unwrap())
            .collect_vec();

        let actual = validate_duplicate_input_names(&inputs)
            .map(|_| "Ok(())".to_owned())
            .unwrap_or_else(|err| format!("{:?}", err));

        expected.assert_eq(&actual);
    }

    #[test]
    fn duplicate_input_names_ok() {
        assert_validate_duplicate_input_names(&["a/b/c", "d/e"], expect!["Ok(())"]);
    }

    #[test]
    fn duplicate_input_names_err() {
        assert_validate_duplicate_input_names(
            &["a/b/c", "d/c"],
            expect![[r#"
                The following input files have the same name, but they must be unique.
                - c (2 files): [a/b/c, d/c]"#]],
        );
        assert_validate_duplicate_input_names(
            &["b", "b", "a", "d", "d"],
            expect![[r#"
                The following input files have the same name, but they must be unique.
                - b (2 files): [b, b]
                - d (2 files): [d, d]"#]],
        );
    }
}
