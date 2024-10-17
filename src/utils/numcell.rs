use std::{
    cell::Cell,
    fmt::{Debug, Formatter},
    ops::{Add, BitAnd, BitOr, Sub},
};

#[derive(Default)]
pub struct NumCell<T> {
    t: Cell<T>,
}

impl<T: Copy + Debug> Debug for NumCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.t.get().fmt(f)
    }
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
    pub fn add_fetch(&self, n: T) -> T
    where
        T: Copy + Add<T, Output = T>,
    {
        let res = self.t.get() + n;
        self.t.set(res);
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

    #[inline(always)]
    pub fn is_zero(&self) -> bool
    where
        T: Eq + Copy + Default,
    {
        self.t.get() == T::default()
    }

    #[inline(always)]
    pub fn is_not_zero(&self) -> bool
    where
        T: Eq + Copy + Default,
    {
        !self.is_zero()
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
