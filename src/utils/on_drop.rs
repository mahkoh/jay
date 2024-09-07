use std::{mem, mem::ManuallyDrop};

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

pub struct OnDrop2<F>
where
    F: FnOnce(),
{
    f: ManuallyDrop<F>,
}

impl<F: FnOnce()> OnDrop2<F> {
    #[expect(dead_code)]
    pub fn new(f: F) -> Self {
        Self {
            f: ManuallyDrop::new(f),
        }
    }

    #[expect(dead_code)]
    pub fn forget(mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.f);
        }
        mem::forget(self);
    }
}

impl<F: FnOnce()> Drop for OnDrop2<F> {
    fn drop(&mut self) {
        let f = unsafe { ManuallyDrop::take(&mut self.f) };
        f();
    }
}
