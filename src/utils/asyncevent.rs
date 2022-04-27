use {
    crate::utils::numcell::NumCell,
    std::{
        cell::Cell,
        fmt::{Debug, Formatter},
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    },
};

#[derive(Default)]
pub struct AsyncEvent {
    triggers: NumCell<u32>,
    waker: Cell<Option<Waker>>,
}

impl Debug for AsyncEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncEvent")
            .field("triggers", &self.triggers.get())
            .finish_non_exhaustive()
    }
}

impl AsyncEvent {
    pub fn clear(&self) {
        self.waker.take();
    }

    pub fn trigger(&self) {
        if self.triggers.fetch_add(1) == 0 {
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
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
