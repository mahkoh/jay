use std::mem;
use std::mem::MaybeUninit;

pub trait MaybeUninitSliceExt2<const N: usize> {
    fn cast_mut<U>(&mut self) -> &mut [MaybeUninit<U>; N];
}

impl<T, const N: usize> MaybeUninitSliceExt2<N> for [MaybeUninit<T>; N] {
    fn cast_mut<U>(&mut self) -> &mut [MaybeUninit<U>; N] {
        const {
            assert!(size_of::<U>() <= size_of::<T>());
            assert!(align_of::<U>() <= align_of::<T>());
        }
        unsafe { mem::transmute(self) }
    }
}
