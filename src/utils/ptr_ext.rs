pub trait PtrExt<T> {
    unsafe fn deref<'a>(self) -> &'a T;
}

pub trait MutPtrExt<T> {
    unsafe fn deref_mut<'a>(self) -> &'a mut T;
}

impl<T> PtrExt<T> for *const T {
    unsafe fn deref<'a>(self) -> &'a T {
        &*self
    }
}

impl<T> PtrExt<T> for *mut T {
    unsafe fn deref<'a>(self) -> &'a T {
        &*self
    }
}

impl<T> MutPtrExt<T> for *mut T {
    unsafe fn deref_mut<'a>(self) -> &'a mut T {
        &mut *self
    }
}
