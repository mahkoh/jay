use std::fmt::{Debug, Display, Formatter};

pub fn debug_fn<F>(f: F) -> Printable<F>
where
    F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
{
    Printable { f }
}

pub struct Printable<F> {
    f: F,
}

impl<F> Debug for Printable<F>
where
    F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (self.f)(f)
    }
}

impl<F> Display for Printable<F>
where
    F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (self.f)(f)
    }
}
