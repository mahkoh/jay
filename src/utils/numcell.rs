use std::cell::Cell;
use std::ops::{Add, Sub};

#[derive(Default)]
pub struct NumCell<T> {
    t: Cell<T>,
}

impl<T> NumCell<T> {
    pub fn new(t: T) -> Self {
        Self { t: Cell::new(t) }
    }

    pub fn load(&self) -> T
    where
        T: Copy,
    {
        self.t.get()
    }

    pub fn fetch_add(&self, n: T) -> T
    where
        T: Copy + Add<T, Output = T>,
    {
        let res = self.t.get();
        self.t.set(res + n);
        res
    }

    pub fn fetch_sub(&self, n: T) -> T
    where
        T: Copy + Sub<T, Output = T>,
    {
        let res = self.t.get();
        self.t.set(res - n);
        res
    }
}
