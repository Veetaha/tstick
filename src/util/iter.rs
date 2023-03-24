pub(crate) fn strs<'a>(
    input: impl IntoIterator<Item = impl AsRef<str>> + 'a,
) -> impl Iterator<Item = String> + 'a {
    input.into_iter().map(|val| val.as_ref().to_owned())
}
