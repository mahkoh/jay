use crate::NumCell;
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

#[derive(Default)]
pub struct AsyncEvent {
    triggers: NumCell<u32>,
    waker: Cell<Option<Waker>>,
}

impl AsyncEvent {
    pub fn clear(&self) {
        self.waker.take();
    }

    pub fn trigger(&self) {
        self.triggers.fetch_add(1);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    pub fn triggered(&self) -> AsyncEventTriggered {
        AsyncEventTriggered { ae: self }
    }
}

pub struct AsyncEventTriggered<'a> {
    ae: &'a AsyncEvent,
}

impl<'a> Future for AsyncEventTriggered<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.ae.triggers.replace(0) == 0 {
            self.ae.waker.set(Some(cx.waker().clone()));
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}
