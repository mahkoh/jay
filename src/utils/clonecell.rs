use {
    crate::utils::{
        markers::JayClone,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    std::{
        cell::UnsafeCell,
        fmt::{Debug, Formatter},
        mem,
    },
};

#[derive(Default)]
pub struct CloneCell<T> {
    data: UnsafeCell<T>,
}

impl<T: JayClone> Clone for CloneCell<T> {
    fn clone(&self) -> Self {
        Self {
            data: UnsafeCell::new(self.get()),
        }
    }
}

impl<T: JayClone + Debug> Debug for CloneCell<T> {
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
        T: JayClone,
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
    pub fn is_none(&self) -> bool {
        unsafe { self.data.get().deref().is_none() }
    }
}
