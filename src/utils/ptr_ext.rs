#![allow(clippy::transmute_ptr_to_ref)]

use std::mem;
use std::ptr::NonNull;

pub trait PtrExt<T: ?Sized> {
    unsafe fn deref<'a>(self) -> &'a T;
}

pub trait MutPtrExt<T: ?Sized> {
    unsafe fn deref_mut<'a>(self) -> &'a mut T;
}

impl<T: ?Sized> PtrExt<T> for *const T {
    #[inline(always)]
    unsafe fn deref<'a>(self) -> &'a T {
        unsafe { mem::transmute::<*const T, &'a T>(self) }
    }
}

impl<T: ?Sized> PtrExt<T> for *mut T {
    #[inline(always)]
    unsafe fn deref<'a>(self) -> &'a T {
        unsafe { mem::transmute::<*mut T, &'a T>(self) }
    }
}

impl<T: ?Sized> MutPtrExt<T> for *mut T {
    #[inline(always)]
    unsafe fn deref_mut<'a>(self) -> &'a mut T {
        unsafe { mem::transmute::<*mut T, &'a mut T>(self) }
    }
}

impl<T: ?Sized> PtrExt<T> for NonNull<T> {
    #[inline(always)]
    unsafe fn deref<'a>(self) -> &'a T {
        unsafe { mem::transmute::<NonNull<T>, &'a T>(self) }
    }
}

impl<T: ?Sized> MutPtrExt<T> for NonNull<T> {
    #[inline(always)]
    unsafe fn deref_mut<'a>(self) -> &'a mut T {
        unsafe { mem::transmute::<NonNull<T>, &'a mut T>(self) }
    }
}
