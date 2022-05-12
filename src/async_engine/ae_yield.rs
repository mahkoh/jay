use {
    crate::async_engine::AsyncEngine,
    std::{
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll},
    },
};

pub struct Yield {
    pub(super) iteration: u64,
    pub(super) queue: Rc<AsyncEngine>,
}

impl Future for Yield {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.queue.iteration() > self.iteration {
            Poll::Ready(())
        } else {
            self.queue.push_yield(cx.waker().clone());
            Poll::Pending
        }
    }
}
