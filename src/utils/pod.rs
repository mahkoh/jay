use jay_proc::Pod;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicU32;
use uapi::Pod;

#[expect(unused)]
pub struct AssertPod<T: Pod>(pub PhantomData<T>);

macro_rules! atomic_ptr {
    ($ty:ident, $atomic:ty, $inner:ty) => {
        #[derive(Pod)]
        #[repr(transparent)]
        pub struct $ty(MaybeUninit<UnsafeCell<$inner>>);

        impl Deref for $ty {
            type Target = $atomic;

            fn deref(&self) -> &Self::Target {
                unsafe { <$atomic>::from_ptr(self.0.as_ptr().cast_mut().cast()) }
            }
        }

        impl Debug for $ty {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                <$atomic>::fmt(self, f)
            }
        }
    };
}

atomic_ptr!(PodAtomicU32, AtomicU32, u32);
atomic_ptr!(PodAtomicI32, AtomicI32, i32);
