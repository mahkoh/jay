use std::{cell::Cell, ops::Deref};

#[derive(Default)]
pub struct DoubleBuffered<T> {
    bufs: [T; 2],
    front: Cell<usize>,
}

impl<T> DoubleBuffered<T> {
    pub fn new(bufs: [T; 2]) -> Self {
        Self {
            bufs,
            front: Cell::new(0),
        }
    }

    pub fn front(&self) -> &T {
        unsafe { self.bufs.get_unchecked(self.front.get()) }
    }

    pub fn back(&self) -> &T {
        unsafe { self.bufs.get_unchecked(1 - self.front.get()) }
    }

    pub fn flip(&self) {
        self.front.set(1 - self.front.get());
    }
}

impl<T> Deref for DoubleBuffered<T> {
    type Target = [T; 2];

    fn deref(&self) -> &Self::Target {
        &self.bufs
    }
}
