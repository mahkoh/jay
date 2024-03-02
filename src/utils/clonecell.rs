use {
    crate::utils::{
        linkedlist::NodeRef,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    jay_config::keyboard::mods::Modifiers,
    std::{
        cell::UnsafeCell,
        fmt::{Debug, Formatter},
        mem,
        rc::{Rc, Weak},
    },
};

pub struct CloneCell<T> {
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
        self.get().fmt(f)
    }
}

impl<T> CloneCell<T> {
    pub const fn new(t: T) -> Self {
        Self {
            data: UnsafeCell::new(t),
        }
    }

    #[inline(always)]
    pub fn get(&self) -> T
    where
        T: UnsafeCellCloneSafe,
    {
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
        self.set(T::default())
    }
}

impl<T> CloneCell<Option<T>> {
    #[inline(always)]
    pub fn is_some(&self) -> bool {
        unsafe { self.data.get().deref().is_some() }
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn is_none(&self) -> bool {
        unsafe { self.data.get().deref().is_none() }
    }
}

impl<T: Default> Default for CloneCell<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

pub unsafe trait UnsafeCellCloneSafe: Clone {}

unsafe impl<T: UnsafeCellCloneSafe> UnsafeCellCloneSafe for Option<T> {}

unsafe impl<T: ?Sized> UnsafeCellCloneSafe for Rc<T> {}
unsafe impl<T: ?Sized> UnsafeCellCloneSafe for Weak<T> {}

unsafe impl<T> UnsafeCellCloneSafe for NodeRef<T> {}

unsafe impl UnsafeCellCloneSafe for () {}
unsafe impl UnsafeCellCloneSafe for u64 {}
unsafe impl UnsafeCellCloneSafe for i32 {}
unsafe impl UnsafeCellCloneSafe for u32 {}
unsafe impl UnsafeCellCloneSafe for usize {}

unsafe impl<A: UnsafeCellCloneSafe, B: UnsafeCellCloneSafe> UnsafeCellCloneSafe for (A, B) {}

unsafe impl UnsafeCellCloneSafe for Modifiers {}
