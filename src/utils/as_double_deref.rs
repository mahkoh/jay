use std::ops::Deref;

pub trait AsDoubleDeref {
    type Target;

    fn as_double_deref(self) -> Self::Target;
}

impl<'a, T> AsDoubleDeref for Option<&'a T>
where
    T: Deref,
{
    type Target = Option<&'a <T as Deref>::Target>;

    fn as_double_deref(self) -> Self::Target {
        match self {
            None => None,
            Some(v) => Some(&**v),
        }
    }
}
