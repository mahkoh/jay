use {
    crate::utils::ptr_ext::{MutPtrExt, PtrExt},
    derivative::Derivative,
    std::{cell::UnsafeCell, collections::VecDeque, mem},
};

#[derive(Debug, Derivative)]
#[derivative(Default(bound = ""))]
pub struct SyncQueue<T> {
    el: UnsafeCell<VecDeque<T>>,
}

impl<T> SyncQueue<T> {
    pub fn push(&self, t: T) {
        unsafe {
            self.el.get().deref_mut().push_back(t);
        }
    }

    pub fn push_front(&self, t: T) {
        unsafe {
            self.el.get().deref_mut().push_front(t);
        }
    }

    #[expect(dead_code)]
    pub fn append(&self, src: &mut Vec<T>) {
        unsafe {
            self.el.get().deref_mut().extend(src.drain(..));
        }
    }

    #[inline]
    pub fn pop(&self) -> Option<T> {
        unsafe { self.el.get().deref_mut().pop_front() }
    }

    #[inline]
    pub fn pop_back(&self) -> Option<T> {
        unsafe { self.el.get().deref_mut().pop_back() }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.el.get().deref_mut().is_empty() }
    }

    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    pub fn swap(&self, queue: &mut VecDeque<T>) {
        unsafe {
            mem::swap(self.el.get().deref_mut(), queue);
        }
    }

    pub fn take(&self) -> VecDeque<T> {
        let mut res = VecDeque::new();
        self.swap(&mut res);
        res
    }

    pub fn clear(&self) {
        unsafe {
            self.el.get().deref_mut().clear();
        }
    }

    pub fn len(&self) -> usize {
        unsafe { self.el.get().deref().len() }
    }
}
