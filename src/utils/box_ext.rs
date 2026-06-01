use std::{mem::MaybeUninit, ptr};

pub trait BoxExt: Sized {
    type T;

    fn into_uninit(boxed: Self) -> Box<MaybeUninit<Self::T>>;
}

impl<T> BoxExt for Box<T> {
    type T = T;

    fn into_uninit(boxed: Self) -> Box<MaybeUninit<T>> {
        unsafe {
            let raw: *mut T = Box::into_raw(boxed);
            ptr::drop_in_place(raw);
            Box::from_raw(raw as *mut MaybeUninit<T>)
        }
    }
}
