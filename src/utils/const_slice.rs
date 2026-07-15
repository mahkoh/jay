use std::{ops::Range, slice};

#[expect(dead_code)]
pub const fn const_slice<T>(array: &[T], range: Range<usize>) -> &[T] {
    let lo = range.start;
    let hi = range.end;
    assert!(lo <= hi);
    assert!(array.len() >= hi);
    unsafe { slice::from_raw_parts(array.as_ptr().add(lo), hi - lo) }
}
