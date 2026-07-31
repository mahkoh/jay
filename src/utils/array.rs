pub fn from_fn<F, T, const N: usize>(mut cb: F) -> [T; N]
where
    F: FnMut(usize) -> T,
{
    let mut idx = 0;
    [(); N].map(|_| {
        let res = cb(idx);
        idx += 1;
        res
    })
}

#[expect(dead_code)]
pub trait SliceExt {
    type T;

    fn slice_array<const N: usize>(&self, lo: usize) -> Option<&[Self::T; N]>;
    fn split_array<const N: usize>(&self) -> Option<(&[Self::T; N], &[Self::T])>;
}

impl<T> SliceExt for [T] {
    type T = T;

    fn slice_array<const N: usize>(&self, lo: usize) -> Option<&[Self::T; N]> {
        if lo > self.len() || self.len() - lo < N {
            return None;
        }
        let array = unsafe { &*self.as_ptr().add(lo).cast::<[Self::T; N]>() };
        Some(array)
    }

    fn split_array<const N: usize>(&self) -> Option<(&[Self::T; N], &[Self::T])> {
        if self.len() < N {
            return None;
        }
        let array = unsafe { &*self.as_ptr().cast::<[Self::T; N]>() };
        let tail = unsafe { self.get_unchecked(N..) };
        Some((array, tail))
    }
}
