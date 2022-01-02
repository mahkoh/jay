use std::mem::MaybeUninit;
use std::ops::Range;
use std::slice;

pub trait VecExt<T> {
    fn split_at_spare_mut_ext(&mut self) -> (&mut [T], &mut [MaybeUninit<T>]);
}

impl<T> VecExt<T> for Vec<T> {
    fn split_at_spare_mut_ext(&mut self) -> (&mut [T], &mut [MaybeUninit<T>]) {
        let Range {
            start: ptr,
            end: spare_ptr,
        } = self.as_mut_ptr_range();
        let spare_ptr = spare_ptr.cast::<MaybeUninit<T>>();
        let spare_len = self.capacity() - self.len();
        unsafe {
            let initialized = slice::from_raw_parts_mut(ptr, self.len());
            let spare = slice::from_raw_parts_mut(spare_ptr, spare_len);
            (initialized, spare)
        }
    }
}
