use crate::io_uring::IoUringData;
use crate::utils::numcell::NumCell;
use std::cell::Cell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;

pub struct Debouncer {
    pub(super) cur: NumCell<u64>,
    pub(super) max: u64,
    pub(super) iteration: Cell<u64>,
    pub(super) ring: Rc<IoUringData>,
}

impl Debouncer {
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
