use {
    crate::wheel::{Wheel, WheelDispatcher, WheelId},
    std::{
        cell::{Cell, RefCell},
        error::Error,
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    },
};

pub(super) struct TimeoutData {
    pub expired: Cell<bool>,
    pub waker: RefCell<Option<Waker>>,
}

impl WheelDispatcher for TimeoutData {
    fn dispatch(self: Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.expired.set(true);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake();
        }
        Ok(())
    }
}

pub struct Timeout {
    pub(super) id: WheelId,
    pub(super) wheel: Rc<Wheel>,
    pub(super) data: Rc<TimeoutData>,
}

impl Future for Timeout {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.data.expired.get() {
            Poll::Ready(())
        } else {
            *self.data.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.wheel.remove(self.id);
    }
}
