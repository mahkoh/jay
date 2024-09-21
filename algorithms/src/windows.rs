pub trait WindowsExt<T> {
    type Windows<'a, const N: usize>: Iterator<Item = &'a [T; N]>
    where
        Self: 'a,
        T: 'a;

    fn array_windows_ext<'a, const N: usize>(&'a self) -> Self::Windows<'a, N>;
}

impl<T> WindowsExt<T> for [T] {
    type Windows<'a, const N: usize>
        = WindowsIter<'a, T, N>
    where
        T: 'a;

    fn array_windows_ext<'a, const N: usize>(&'a self) -> Self::Windows<'a, N> {
        WindowsIter { slice: self }
    }
}

pub struct WindowsIter<'a, T, const N: usize> {
    slice: &'a [T],
}

impl<'a, T, const N: usize> Iterator for WindowsIter<'a, T, N> {
    type Item = &'a [T; N];

    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.len() < N {
            return None;
        }
        let res = unsafe { &*self.slice.as_ptr().cast::<[T; N]>() };
        if N > 0 {
            self.slice = &self.slice[1..];
        }
        Some(res)
    }
}
