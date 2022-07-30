pub trait OptionExt<T> {
    fn get_or_insert_default_ext(&mut self) -> &mut T;
}

impl<T: Default> OptionExt<T> for Option<T> {
    fn get_or_insert_default_ext(&mut self) -> &mut T {
        self.get_or_insert_with(|| Default::default())
    }
}
