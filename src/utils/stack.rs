use std::cell::UnsafeCell;
use crate::utils::ptr_ext::MutPtrExt;

pub struct Stack<T> {
    vec: UnsafeCell<Vec<T>>,
}

impl<T> Default for Stack<T> {
    fn default() -> Self {
        Self {
            vec: Default::default(),
        }
    }
}

impl<T> Stack<T> {
    pub fn push(&self, v: T) {
        unsafe {
            self.vec.get().deref_mut().push(v);
        }
    }

    pub fn pop(&self) -> Option<T> {
        unsafe {
            self.vec.get().deref_mut().pop()
        }
    }
}
