use std::{
    ops::Deref,
    rc::{Rc, Weak},
};

pub fn rc_eq<T: ?Sized>(a: &Rc<T>, b: &Rc<T>) -> bool {
    Rc::as_ptr(a) as *const u8 == Rc::as_ptr(b) as *const u8
}

pub fn rc_weak_eq<T: ?Sized>(a: &Rc<T>, b: &Weak<T>) -> bool {
    Rc::as_ptr(a) as *const u8 == b.as_ptr() as *const u8
}

#[derive(Default)]
pub struct RcEq<T>(pub Rc<T>);

impl<T> Clone for RcEq<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> PartialEq for RcEq<T> {
    fn eq(&self, other: &Self) -> bool {
        rc_eq(&self.0, &other.0)
    }
}

impl<T> Eq for RcEq<T> {}

impl<T> Deref for RcEq<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
