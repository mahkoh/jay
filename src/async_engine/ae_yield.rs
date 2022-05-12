use {
    crate::async_engine::ae_queue::DispatchQueue,
    std::{
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll},
    },
};

pub struct Yield {
    pub(super) iteration: u64,
    pub(super) queue: Rc<DispatchQueue>,
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
