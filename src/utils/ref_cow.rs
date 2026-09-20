use std::ops::Deref;

pub enum RefCow<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<T> Deref for RefCow<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            RefCow::Borrowed(v) => v,
            RefCow::Owned(v) => v,
        }
    }
}

impl<T> PartialEq for RefCow<'_, T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T> Default for RefCow<'_, T>
where
    T: Default,
{
    fn default() -> Self {
        Self::Owned(Default::default())
    }
}

impl<T> RefCow<'_, T> {
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
