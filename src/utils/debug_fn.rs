use std::fmt::{Debug, Formatter};

pub fn debug_fn<F>(f: F) -> impl Debug
where
    F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
{
    DebugFn { f }
}

struct DebugFn<F> {
    f: F,
}

impl<F> Debug for DebugFn<F>
where
    F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (self.f)(f)
    }
}
