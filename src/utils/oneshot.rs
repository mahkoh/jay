use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

pub fn oneshot<T>() -> (OneshotTx<T>, OneshotRx<T>) {
    let os = Rc::new(Oneshot {
        data: Cell::new(None),
        waiter: Cell::new(None),
    });
    (OneshotTx { data: os.clone() }, OneshotRx { data: os })
}

struct Oneshot<T> {
    data: Cell<Option<T>>,
    waiter: Cell<Option<Waker>>,
}

pub struct OneshotTx<T> {
    data: Rc<Oneshot<T>>,
}

pub struct OneshotRx<T> {
    data: Rc<Oneshot<T>>,
}

impl<T> OneshotTx<T> {
    pub fn send(self, t: T) {
        self.data.data.set(Some(t));
        if let Some(waiter) = self.data.waiter.replace(None) {
            waiter.wake();
        }
    }
}

impl<T> Future for OneshotRx<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(data) = self.data.data.replace(None) {
            Poll::Ready(data)
        } else {
            self.data.waiter.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}
