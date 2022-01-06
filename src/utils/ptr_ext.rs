pub trait PtrExt<T: ?Sized> {
    unsafe fn deref<'a>(self) -> &'a T;
}

pub trait MutPtrExt<T: ?Sized> {
    unsafe fn deref_mut<'a>(self) -> &'a mut T;
}

impl<T: ?Sized> PtrExt<T> for *const T {
    unsafe fn deref<'a>(self) -> &'a T {
        &*self
    }
}

impl<T: ?Sized> PtrExt<T> for *mut T {
    unsafe fn deref<'a>(self) -> &'a T {
        &*self
    }
}

impl<T: ?Sized> MutPtrExt<T> for *mut T {
    unsafe fn deref_mut<'a>(self) -> &'a mut T {
        &mut *self
    }
}
