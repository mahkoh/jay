use {
    crate::utils::ptr_ext::{MutPtrExt, PtrExt},
    std::{
        cell::{Cell, UnsafeCell},
        collections::VecDeque,
        future::Future,
        mem,
        pin::Pin,
        task::{Context, Poll, Waker},
    },
};

pub struct AsyncQueue<T> {
    data: UnsafeCell<VecDeque<T>>,
    waiter: Cell<Option<Waker>>,
}

impl<T> Default for AsyncQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AsyncQueue<T> {
    pub fn new() -> Self {
        Self {
            data: Default::default(),
            waiter: Default::default(),
        }
    }

    pub fn push(&self, t: T) {
        unsafe {
            self.data.get().deref_mut().push_back(t);
        }
        if let Some(waiter) = self.waiter.take() {
            waiter.wake();
        }
    }

    pub fn try_pop(&self) -> Option<T> {
        unsafe { self.data.get().deref_mut().pop_front() }
    }

    pub fn pop<'a>(&'a self) -> AsyncQueuePop<'a, T> {
        AsyncQueuePop { queue: self }
    }

    pub fn non_empty<'a>(&'a self) -> AsyncQueueNonEmpty<'a, T> {
        AsyncQueueNonEmpty { queue: self }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.data.get().deref().is_empty() }
    }

    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    pub fn clear(&self) {
        unsafe {
            mem::take(self.data.get().deref_mut());
        }
        self.waiter.take();
    }

    pub fn move_to(&self, other: &mut VecDeque<T>) {
        unsafe {
            other.append(self.data.get().deref_mut());
        }
    }
}

pub struct AsyncQueuePop<'a, T> {
    queue: &'a AsyncQueue<T>,
}

impl<'a, T> Future for AsyncQueuePop<'a, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(t) = self.queue.try_pop() {
            Poll::Ready(t)
        } else {
            self.queue.waiter.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}

pub struct AsyncQueueNonEmpty<'a, T> {
    queue: &'a AsyncQueue<T>,
}

impl<'a, T> Future for AsyncQueueNonEmpty<'a, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if unsafe { self.queue.data.get().deref().len() } > 0 {
            Poll::Ready(())
        } else {
            self.queue.waiter.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}
