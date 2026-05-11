use {
    crate::utils::{
        clonecell::UnsafeCellCloneSafe,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    std::{
        cell::{Cell, UnsafeCell},
        mem,
        pin::Pin,
        task::{Context, Poll, Waker},
    },
};

pub struct Stack<T> {
    vec: UnsafeCell<Vec<T>>,
}

impl<T> Default for Stack<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Stack<T> {
    pub const fn new() -> Stack<T> {
        Self {
            vec: UnsafeCell::new(vec![]),
        }
    }

    pub fn push(&self, v: T) {
        unsafe {
            self.vec.get().deref_mut().push(v);
        }
    }

    pub fn pop(&self) -> Option<T> {
        unsafe { self.vec.get().deref_mut().pop() }
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: UnsafeCellCloneSafe,
    {
        unsafe {
            let v = self.vec.get().deref();
            (*v).clone()
        }
    }

    pub fn take(&self) -> Vec<T> {
        unsafe { mem::take(self.vec.get().deref_mut()) }
    }

    pub fn len(&self) -> usize {
        unsafe { self.vec.get().deref().len() }
    }

    pub fn swap(&self, vec: &mut Vec<T>) {
        unsafe { mem::swap(self.vec.get().deref_mut(), vec) }
    }
}

pub struct AsyncStack<T> {
    stack: Stack<T>,
    waiter: Cell<Option<Waker>>,
}

impl<T> Default for AsyncStack<T> {
    fn default() -> Self {
        Self {
            stack: Default::default(),
            waiter: Default::default(),
        }
    }
}

impl<T> AsyncStack<T> {
    #[expect(dead_code)]
    pub fn push(&self, v: T) {
        self.stack.push(v);
        if let Some(waker) = self.waiter.take() {
            waker.wake();
        }
    }

    #[expect(dead_code)]
    pub fn non_empty(&self) -> AsyncStackNonEmpty<'_, T> {
        AsyncStackNonEmpty { stack: self }
    }

    #[expect(dead_code)]
    pub fn swap(&self, vec: &mut Vec<T>) {
        self.stack.swap(vec);
    }

    #[expect(dead_code)]
    pub fn clear(&self) {
        self.waiter.take();
        self.stack.take();
    }
}

pub struct AsyncStackNonEmpty<'a, T> {
    stack: &'a AsyncStack<T>,
}

impl<'a, T> Future for AsyncStackNonEmpty<'a, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.stack.stack.len() > 0 {
            Poll::Ready(())
        } else {
            self.stack.waiter.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}
