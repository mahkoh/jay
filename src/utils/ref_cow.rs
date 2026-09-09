use std::ops::Deref;

pub enum RefCow<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<'a, T> Deref for RefCow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            RefCow::Borrowed(v) => v,
            RefCow::Owned(v) => v,
        }
    }
}

impl<'a, T> PartialEq for RefCow<'a, T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<'a, T> Default for RefCow<'a, T>
where
    T: Default,
{
    fn default() -> Self {
        Self::Owned(Default::default())
    }
}

impl<'a, T> RefCow<'a, T> {
    pub fn to_static(self) -> RefCow<'static, T>
    where
        T: Clone,
    {
        let v = match self {
            RefCow::Borrowed(v) => v.clone(),
            RefCow::Owned(v) => v,
        };
        RefCow::Owned(v)
    }
}
