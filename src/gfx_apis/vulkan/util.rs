use std::mem;

pub struct OnDrop<F>(pub F)
where
    F: FnMut() + Copy;

impl<F: FnMut() + Copy> OnDrop<F> {
    pub fn forget(self) {
        mem::forget(self);
    }
}

impl<F: FnMut() + Copy> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
