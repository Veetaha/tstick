use easy_ext::ext;

#[ext(ResultExt)]
pub(crate) impl<T, E> Result<T, E> {
    fn err_into<U>(self) -> Result<T, U>
    where
        E: Into<U>,
    {
        self.map_err(Into::into)
    }
}
