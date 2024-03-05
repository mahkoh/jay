//! Tools for async task management.

use std::{
    cell::Cell,
    fmt::{Debug, Formatter},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// Spawns an asynchronous task that will run in the background.
pub fn spawn<T, F>(f: F) -> JoinHandle<T>
where
    T: 'static,
    F: Future<Output = T> + 'static,
{
    let slot = match try_get!() {
        None => Rc::new(JoinSlot {
            task_id: 0,
            slot: Cell::new(None),
            waker: Cell::new(None),
        }),
        Some(c) => c.spawn_task(f),
    };
    JoinHandle { slot }
}

pub(crate) struct JoinSlot<T> {
    pub task_id: u64,
    pub slot: Cell<Option<T>>,
    pub waker: Cell<Option<Waker>>,
}

/// A handle to join or abort a spawned task.
///
/// When the handle is dropped, the task continues to run in the background.
pub struct JoinHandle<T> {
    slot: Rc<JoinSlot<T>>,
}

impl<T> Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinHandle")
            .field("task_id", &self.slot.task_id)
            .finish_non_exhaustive()
    }
}

impl<T> Unpin for JoinHandle<T> {}

impl<T> JoinHandle<T> {
    /// Aborts the task immediately.
    pub fn abort(self) {
        get!().abort_task(self.slot.task_id);
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(t) = self.slot.slot.take() {
            return Poll::Ready(t);
        }
        self.slot.waker.set(Some(cx.waker().clone()));
        Poll::Pending
    }
}
