use {
    crate::{io_uring::IoUringData, utils::numcell::NumCell},
    std::{cell::Cell, future::poll_fn, rc::Rc, task::Poll},
};

pub struct Debouncer {
    pub(super) cur: NumCell<u64>,
    pub(super) max: u64,
    pub(super) iteration: Cell<u64>,
    pub(super) ring: Rc<IoUringData>,
}

impl Debouncer {
    #[expect(dead_code)]
    pub async fn debounce(&self) {
        let iteration = self.ring.iteration.get();
        if self.iteration.replace(iteration) != iteration {
            self.cur.set(0);
        }
        if self.cur.fetch_add(1) > self.max {
            poll_fn(|ctx| {
                if self.ring.iteration.get() > iteration {
                    Poll::Ready(())
                } else {
                    self.ring.yields.push(ctx.waker().clone());
                    Poll::Pending
                }
            })
            .await;
            self.cur.set(0);
        }
    }
}
