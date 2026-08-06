use std::mem;
use std::mem::MaybeUninit;

#[expect(dead_code)]
pub trait MaybeUninitSliceExt1 {
    type T;

    fn init<const N: usize>(&mut self, val: [Self::T; N]) -> &mut [Self::T; N];
}

impl<T> MaybeUninitSliceExt1 for [MaybeUninit<T>] {
    type T = T;

    fn init<const N: usize>(&mut self, val: [Self::T; N]) -> &mut [Self::T; N] {
        unsafe {
            let slf: *mut [Self::T; N] = self[..N].as_mut_ptr() as _;
            std::ptr::write(slf, val);
            &mut *slf
        }
    }
}

#[expect(dead_code)]
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
