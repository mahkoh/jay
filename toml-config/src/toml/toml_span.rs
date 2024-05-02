use std::{
    borrow::Borrow,
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
};

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Span {
    pub lo: usize,
    pub hi: usize,
}

impl Debug for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.lo, self.hi)
    }
}

#[derive(Copy, Clone, Eq)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

impl<T> Spanned<T> {
    pub fn as_ref(&self) -> Spanned<&T> {
        Spanned {
            span: self.span,
            value: &self.value,
        }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Spanned<U> {
        Spanned {
            span: self.span,
            value: f(self.value),
        }
    }

    pub fn into<U>(self) -> Spanned<U>
    where
        T: Into<U>,
    {
        Spanned {
            span: self.span,
            value: self.value.into(),
        }
    }
}

impl<T: Debug> Debug for Spanned<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)?;
        write!(f, " @ {:?}", self.span)
    }
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value)
    }
}

impl<T: Hash> Hash for Spanned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state)
    }
}

impl Borrow<str> for Spanned<String> {
    fn borrow(&self) -> &str {
        &self.value
    }
}

pub trait SpannedExt: Sized {
    fn spanned(self, span: Span) -> Spanned<Self>;
}

impl<T> SpannedExt for T {
    fn spanned(self, span: Span) -> Spanned<Self> {
        Spanned { span, value: self }
    }
}

pub trait DespanExt: Sized {
    type T;

    fn despan(self) -> Option<Self::T>;
    fn despan_into<U>(self) -> Option<U>
    where
        Self::T: Into<U>;
}

impl<T> DespanExt for Option<Spanned<T>> {
    type T = T;

    fn despan(self) -> Option<Self::T> {
        self.map(|v| v.value)
    }

    fn despan_into<U>(self) -> Option<U>
    where
        Self::T: Into<U>,
    {
        self.map(|v| v.value.into())
    }
}
