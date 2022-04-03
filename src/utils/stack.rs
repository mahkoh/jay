use crate::utils::ptr_ext::{MutPtrExt, PtrExt};
use std::cell::UnsafeCell;

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
        unsafe { self.vec.get().deref_mut().pop() }
    }

    pub fn to_vec(&self) -> Vec<T>
        where T: Clone,
    {
        unsafe {
            let v = self.vec.get().deref();
            (*v).clone()
        }
    }
}
