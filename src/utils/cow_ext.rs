use std::borrow::Cow;

#[expect(dead_code)]
pub trait OptionCowExt<'a, T>
where
    T: ToOwned + ?Sized,
{
    fn borrowed(&self) -> Option<Cow<'a, T>>;
}

impl<'a, T> OptionCowExt<'a, T> for Option<&'a T>
where
    T: ToOwned + ?Sized,
{
    fn borrowed(&self) -> Option<Cow<'a, T>> {
        self.map(|v| Cow::Borrowed(v))
    }
}
