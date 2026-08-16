use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr::NonNull;

#[cfg(test)]
mod tests;

pub struct AliasableBox<T>(NonNull<T>)
where
    T: ?Sized;

pub trait AliasableBoxExt {
    type T: ?Sized;

    fn into_aliasable(self) -> AliasableBox<Self::T>;
}

impl<T> AliasableBoxExt for Box<T>
where
    T: ?Sized,
{
    type T = T;

    fn into_aliasable(self) -> AliasableBox<Self::T> {
        unsafe { AliasableBox(NonNull::new_unchecked(Box::into_raw(self))) }
    }
}

impl<T> AliasableBox<T>
where
    T: ?Sized,
{
    pub fn into_box(self) -> Box<T> {
        let slf = ManuallyDrop::new(self);
        unsafe { Box::from_raw(slf.0.as_ptr()) }
    }
}

impl<T> Drop for AliasableBox<T>
where
    T: ?Sized,
{
    fn drop(&mut self) {
        let _box = unsafe { Box::from_raw(self.0.as_ptr()) };
    }
}

impl<T> Deref for AliasableBox<T>
where
    T: ?Sized,
{
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

impl<T> DerefMut for AliasableBox<T>
where
    T: ?Sized,
{
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}

impl<T> Debug for AliasableBox<T>
where
    T: Debug + ?Sized,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        T::fmt(self, f)
    }
}

unsafe impl<T> Send for AliasableBox<T> where T: Send + ?Sized {}
unsafe impl<T> Sync for AliasableBox<T> where T: Sync + ?Sized {}

impl<T> Default for AliasableBox<T>
where
    T: Default,
{
    fn default() -> Self {
        Box::<T>::default().into_aliasable()
    }
}

impl<T> Clone for AliasableBox<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Box::new(self.deref().clone()).into_aliasable()
    }

    fn clone_from(&mut self, source: &Self) {
        (**self).clone_from(&**source);
    }
}
