use std::cell::Cell;
use std::ops::{Add, BitAnd, BitOr, Sub};

#[derive(Default)]
pub struct NumCell<T> {
    t: Cell<T>,
}

impl<T> NumCell<T> {
    #[inline(always)]
    pub fn new(t: T) -> Self {
        Self { t: Cell::new(t) }
    }

    #[inline(always)]
    pub fn set(&self, n: T) {
        let _ = self.t.replace(n);
    }

    #[inline(always)]
    pub fn replace(&self, n: T) -> T {
        self.t.replace(n)
    }

    #[inline(always)]
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        self.t.get()
    }

    #[inline(always)]
    pub fn fetch_add(&self, n: T) -> T
    where
        T: Copy + Add<T, Output = T>,
    {
        let res = self.t.get();
        self.t.set(res + n);
        res
    }

    #[inline(always)]
    pub fn fetch_sub(&self, n: T) -> T
    where
        T: Copy + Sub<T, Output = T>,
    {
        let res = self.t.get();
        self.t.set(res - n);
        res
    }

    #[inline(always)]
    pub fn or_assign(&self, n: T)
    where
        T: Copy + BitOr<Output = T>,
    {
        self.t.set(self.t.get() | n);
    }

    #[inline(always)]
    pub fn and_assign(&self, n: T)
    where
        T: Copy + BitAnd<Output = T>,
    {
        self.t.set(self.t.get() & n);
    }
}

impl<T: BitOr<Output = T> + Copy> BitOr<T> for &'_ NumCell<T> {
    type Output = T;

    #[inline(always)]
    fn bitor(self, rhs: T) -> Self::Output {
        self.t.get() | rhs
    }
}

impl<T: BitAnd<Output = T> + Copy> BitAnd<T> for &'_ NumCell<T> {
    type Output = T;

    #[inline(always)]
    fn bitand(self, rhs: T) -> Self::Output {
        self.t.get() & rhs
    }
}
