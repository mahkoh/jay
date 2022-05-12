use {
    crate::{
        async_engine::{AsyncEngine, AsyncError},
        event_loop::{EventLoop, EventLoopDispatcher, EventLoopError, EventLoopId},
        utils::numcell::NumCell,
    },
    std::{
        cell::{Cell, RefCell},
        error::Error,
        fmt::{Debug, Formatter},
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    },
    uapi::{c, OwnedFd},
};

type Queue = RefCell<Vec<(Waker, Rc<Cell<Option<FdStatus>>>)>>;

pub(super) struct AsyncFdData {
    pub(super) ref_count: NumCell<u64>,
    pub(super) fd: Rc<OwnedFd>,
    pub(super) id: EventLoopId,
    pub(super) el: Rc<EventLoop>,
    pub(super) write_registered: Cell<bool>,
    pub(super) read_registered: Cell<bool>,
    pub(super) readers: Queue,
    pub(super) writers: Queue,
}

impl AsyncFdData {
    fn update_interests(&self) -> Result<(), EventLoopError> {
        let mut events = 0;
        if self.write_registered.get() {
            events |= c::EPOLLOUT;
        }
        if self.read_registered.get() {
            events |= c::EPOLLIN;
        }
        let res = self.el.modify(self.id, events);
        if res.is_err() {
            if let Err(e) = self.el.remove(self.id) {
                log::error!(
                    "Fatal error: Cannot remove file descriptor from event loop: {:?}",
                    e
                );
                self.el.stop();
            }
        }
        res
    }

    fn poll(
        &self,
        woken: &Rc<Cell<Option<FdStatus>>>,
        cx: &mut Context<'_>,
        registered: impl Fn(&AsyncFdData) -> &Cell<bool>,
        queue: impl Fn(&AsyncFdData) -> &Queue,
    ) -> Poll<Result<FdStatus, AsyncError>> {
        if let Some(status) = woken.get() {
            return Poll::Ready(Ok(status));
        }
        if !registered(self).get() {
            registered(self).set(true);
            if let Err(e) = self.update_interests() {
                return Poll::Ready(Err(AsyncError::EventLoopError(e)));
            }
        }
        queue(self)
            .borrow_mut()
            .push((cx.waker().clone(), woken.clone()));
        Poll::Pending
    }
}

impl EventLoopDispatcher for AsyncFdData {
    fn dispatch(self: Rc<Self>, _fd: Option<i32>, events: i32) -> Result<(), Box<dyn Error>> {
        let mut status = FdStatus::Ok;
        if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
            status = FdStatus::Err;
            if let Err(e) = self.el.remove(self.id) {
                return Err(Box::new(e));
            }
        }
        let mut woke_any = false;
        if events & c::EPOLLIN != 0 || status == FdStatus::Err {
            let mut readers = self.readers.borrow_mut();
            woke_any |= !readers.is_empty();
            for (waker, woken) in readers.drain(..) {
                woken.set(Some(status));
                waker.wake();
            }
        }
        if events & c::EPOLLOUT != 0 || status == FdStatus::Err {
            let mut writers = self.writers.borrow_mut();
            woke_any |= !writers.is_empty();
            for (waker, woken) in writers.drain(..) {
                woken.set(Some(status));
                waker.wake();
            }
        }
        if !woke_any && status == FdStatus::Ok {
            self.read_registered.set(false);
            self.write_registered.set(false);
            if let Err(e) = self.update_interests() {
                return Err(Box::new(e));
            }
        }
        Ok(())
    }
}

pub struct AsyncFd {
    pub(super) engine: Rc<AsyncEngine>,
    pub(super) data: Rc<AsyncFdData>,
}

impl Debug for AsyncFd {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncFd").finish_non_exhaustive()
    }
}

impl Clone for AsyncFd {
    fn clone(&self) -> Self {
        self.data.ref_count.fetch_add(1);
        Self {
            engine: self.engine.clone(),
            data: self.data.clone(),
        }
    }
}

impl Drop for AsyncFd {
    fn drop(&mut self) {
        if self.data.ref_count.fetch_sub(1) == 1 {
            self.engine.fds.remove(&self.data.fd.raw());
            let _ = self.data.el.remove(self.data.id);
        }
    }
}

impl AsyncFd {
    pub fn raw(&self) -> i32 {
        self.data.fd.raw()
    }

    pub fn readable(&self) -> AsyncFdReadable {
        AsyncFdReadable {
            fd: self,
            woken: Rc::new(Cell::new(None)),
        }
    }

    pub fn writable(&self) -> AsyncFdWritable {
        AsyncFdWritable {
            fd: self,
            woken: Rc::new(Cell::new(None)),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FdStatus {
    Ok,
    Err,
}

pub struct AsyncFdReadable<'a> {
    fd: &'a AsyncFd,
    woken: Rc<Cell<Option<FdStatus>>>,
}

impl<'a> Future for AsyncFdReadable<'a> {
    type Output = Result<FdStatus, AsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let data = &self.fd.data;
        data.poll(&self.woken, cx, |d| &d.read_registered, |d| &d.readers)
    }
}

pub struct AsyncFdWritable<'a> {
    fd: &'a AsyncFd,
    woken: Rc<Cell<Option<FdStatus>>>,
}

impl<'a> Future for AsyncFdWritable<'a> {
    type Output = Result<FdStatus, AsyncError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let data = &self.fd.data;
        data.poll(&self.woken, cx, |d| &d.write_registered, |d| &d.writers)
    }
}
