use {
    crate::utils::{
        clonecell::UnsafeCellCloneSafe,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    derivative::Derivative,
    std::{
        cell::{Cell, UnsafeCell},
        mem,
        pin::Pin,
        task::{Context, Poll, Waker},
    },
};

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct Stack<T> {
    vec: UnsafeCell<Vec<T>>,
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[expect(dead_code)]
    pub fn clear(&self) {
        let mut vec = vec![];
        self.swap(&mut vec);
        vec.clear();
        if self.is_empty() {
            self.swap(&mut vec);
        }
    }
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct AsyncStack<T> {
    stack: Stack<T>,
    waiter: Cell<Option<Waker>>,
}

impl<T> AsyncStack<T> {
    pub fn push(&self, v: T) {
        self.stack.push(v);
        if let Some(waker) = self.waiter.take() {
            waker.wake();
        }
    }

    pub fn non_empty(&self) -> AsyncStackNonEmpty<'_, T> {
        AsyncStackNonEmpty { stack: self }
    }

    pub fn swap(&self, vec: &mut Vec<T>) {
        self.stack.swap(vec);
    }

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
