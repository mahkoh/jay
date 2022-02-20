use crate::utils::linkedlist::NodeRef;
use crate::utils::ptr_ext::{MutPtrExt, PtrExt};
use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::mem;
use std::rc::Rc;

pub struct CloneCell<T: UnsafeCellCloneSafe> {
    data: UnsafeCell<T>,
}

impl<T: UnsafeCellCloneSafe> Clone for CloneCell<T> {
    fn clone(&self) -> Self {
        Self {
            data: UnsafeCell::new(self.get()),
        }
    }
}

impl<T: UnsafeCellCloneSafe + Debug> Debug for CloneCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe { self.data.get().deref().fmt(f) }
    }
}

impl<T: UnsafeCellCloneSafe> CloneCell<T> {
    pub fn new(t: T) -> Self {
        Self {
            data: UnsafeCell::new(t),
        }
    }

    #[inline(always)]
    pub fn get(&self) -> T {
        unsafe { self.data.get().deref().clone() }
    }

    #[inline(always)]
    pub fn set(&self, t: T) -> T {
        unsafe { mem::replace(self.data.get().deref_mut(), t) }
    }

    #[inline(always)]
    pub fn take(&self) -> T
    where
        T: Default,
    {
        unsafe { mem::take(self.data.get().deref_mut()) }
    }
}

impl<T: Default + UnsafeCellCloneSafe> Default for CloneCell<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

pub unsafe trait UnsafeCellCloneSafe: Clone {}

unsafe impl<T: UnsafeCellCloneSafe> UnsafeCellCloneSafe for Option<T> {}

unsafe impl<T: ?Sized> UnsafeCellCloneSafe for Rc<T> {}

unsafe impl<T> UnsafeCellCloneSafe for NodeRef<T> {}

unsafe impl UnsafeCellCloneSafe for () {}
