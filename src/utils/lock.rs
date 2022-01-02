use std::cell::{RefCell, RefMut};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub struct AsyncLock<T> {
    data: RefCell<T>,
    waiters: RefCell<Vec<Waker>>,
}

impl<T> AsyncLock<T> {
    pub fn lock<'a>(&'a self) -> LockedFuture<'a, T> {
        LockedFuture { lock: self }
    }
}

pub struct LockedFuture<'a, T> {
    lock: &'a AsyncLock<T>,
}

impl<'a, T> Future for LockedFuture<'a, T> {
    type Output = Locked<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Ok(data) = self.lock.data.try_borrow_mut() {
            Poll::Ready(Locked {
                data,
                lock: self.lock,
            })
        } else {
            self.lock.waiters.borrow_mut().push(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct Locked<'a, T> {
    data: RefMut<'a, T>,
    lock: &'a AsyncLock<T>,
}

impl<'a, T> Deref for Locked<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}

impl<'a, T> DerefMut for Locked<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.deref_mut()
    }
}

impl<'a, T> Drop for Locked<'a, T> {
    fn drop(&mut self) {
        for waiter in self.lock.waiters.borrow_mut().drain(..) {
            waiter.wake();
        }
    }
}
