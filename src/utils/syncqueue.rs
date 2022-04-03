use crate::utils::ptr_ext::MutPtrExt;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::mem;

#[derive(Debug)]
pub struct SyncQueue<T> {
    el: UnsafeCell<VecDeque<T>>,
}

impl<T> Default for SyncQueue<T> {
    fn default() -> Self {
        Self {
            el: Default::default(),
        }
    }
}

impl<T> SyncQueue<T> {
    pub fn push(&self, t: T) {
        unsafe {
            self.el.get().deref_mut().push_back(t);
        }
    }

    pub fn pop(&self) -> Option<T> {
        unsafe { self.el.get().deref_mut().pop_front() }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.el.get().deref_mut().is_empty() }
    }

    pub fn swap(&self, queue: &mut VecDeque<T>) {
        unsafe {
            mem::swap(self.el.get().deref_mut(), queue);
        }
    }
}
