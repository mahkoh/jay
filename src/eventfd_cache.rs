use crate::async_engine::AsyncEngine;
use crate::async_engine::SpawnedFuture;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
use crate::io_uring::PendingPoll;
use crate::io_uring::PollCallback;
use crate::utils::buf::Buf;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::oserror::OsError;
use crate::utils::oserror::OsErrorExt;
use crate::utils::oserror::OsErrorExt2;
use crate::utils::queue::AsyncQueue;
use crate::utils::stack::Stack;
use std::cell::Cell;
use std::ffi::c_short;
use std::future::poll_fn;
use std::pin::Pin;
use std::rc::Rc;
use std::slice;
use std::task::Poll;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum EventfdError {
    #[error("Could not create an eventfd")]
    CreateEventfd(#[source] OsError),
}

pub struct EventfdCache {
    inner: Rc<Inner>,
    _task: SpawnedFuture<()>,
}

struct Inner {
    ring: Rc<IoUring>,
    fds: Stack<Rc<OwnedFd>>,
    signaled: Stack<Rc<Cell<bool>>>,
    readable: Stack<Rc<Readable>>,
    recycle: AsyncQueue<Rc<OwnedFd>>,
}

pub struct Eventfd {
    cache: Rc<Inner>,
    pub fd: Rc<OwnedFd>,
    signaled: Rc<Cell<bool>>,
}

#[derive(Default)]
struct Readable {
    data: Cell<Option<ReadableData>>,
}

struct ReadableData {
    cache: Rc<Inner>,
    signaled: Rc<Cell<bool>>,
    cb: Rc<dyn PollCallback>,
}

impl EventfdCache {
    pub fn new(ring: &Rc<IoUring>, eng: &Rc<AsyncEngine>) -> Rc<Self> {
        let inner = Rc::new(Inner {
            ring: ring.clone(),
            fds: Default::default(),
            signaled: Default::default(),
            readable: Default::default(),
            recycle: Default::default(),
        });
        let task = eng.spawn("eventfd-cache", inner.clone().recycle());
        Rc::new(Self { inner, _task: task })
    }

    pub fn acquire(&self) -> Result<Eventfd, EventfdError> {
        let fd = match self.inner.fds.pop() {
            Some(fd) => fd,
            _ => uapi::eventfd(0, c::EFD_CLOEXEC)
                .map(Rc::new)
                .map_os_err(EventfdError::CreateEventfd)?,
        };
        Ok(Eventfd {
            cache: self.inner.clone(),
            fd,
            signaled: self.inner.signaled.pop().unwrap_or_default(),
        })
    }
}

impl Eventfd {
    pub fn is_signaled(&self) -> bool {
        self.signaled.get()
    }

    pub async fn signaled(&self) -> Result<(), IoUringError> {
        if self.signaled.get() {
            return Ok(());
        }
        self.cache.ring.readable(&self.fd).await?;
        self.signaled.set(true);
        Ok(())
    }

    pub fn signaled_blocking(&self) -> Result<(), OsError> {
        if self.signaled.get() {
            return Ok(());
        }
        let mut pollfd = c::pollfd {
            fd: self.fd.raw(),
            events: c::POLLIN,
            revents: 0,
        };
        uapi::poll(slice::from_mut(&mut pollfd), -1).to_os_error()?;
        self.signaled.set(true);
        Ok(())
    }

    pub fn signaled_external(
        &self,
        cb: Rc<dyn PollCallback>,
    ) -> Result<Option<PendingPoll>, IoUringError> {
        if self.signaled.get() {
            return Ok(None);
        }
        let readable = self.cache.readable.pop().unwrap_or_default();
        readable.data.set(Some(ReadableData {
            cache: self.cache.clone(),
            signaled: self.signaled.clone(),
            cb,
        }));
        self.cache
            .ring
            .readable_external(&self.fd, readable)
            .map(Some)
    }
}

impl PollCallback for Readable {
    fn completed(self: Rc<Self>, res: Result<c_short, OsError>) {
        let Some(data) = self.data.take() else {
            return;
        };
        data.cache.readable.push(self);
        if data.cache.return_signaled(&data.signaled) {
            // nothing
        } else if res.is_ok() {
            data.signaled.set(true);
        }
        data.cb.completed(res);
    }
}

impl Inner {
    async fn recycle(self: Rc<Self>) {
        let slf = &*self;
        let mut fds = vec![];
        let mut bufs = vec![];
        let mut tasks = vec![];
        let mut todo = vec![];
        loop {
            fds.clear();
            tasks.clear();
            todo.clear();
            slf.recycle.non_empty().await;
            while let Some(fd) = slf.recycle.try_pop() {
                fds.push(fd);
            }
            for (idx, fd) in fds.iter().enumerate() {
                if idx >= bufs.len() {
                    bufs.push(Buf::new(size_of::<u64>()));
                }
                let fd = fd.clone();
                let buf = bufs[idx].clone();
                tasks.push(async move { slf.ring.read(&fd, buf).await });
                todo.push(idx);
            }
            poll_fn(|ctx| {
                let mut i = 0;
                while i < todo.len() {
                    let idx = todo[i];
                    let task = unsafe { Pin::new_unchecked(&mut tasks[idx]) };
                    if let Poll::Ready(res) = task.poll(ctx) {
                        todo.swap_remove(i);
                        match res {
                            Ok(_) => {
                                self.fds.push(fds[idx].clone());
                            }
                            Err(e) => {
                                log::error!("Could not read from eventfd: {}", ErrorFmt(e));
                            }
                        }
                    } else {
                        i += 1;
                    }
                }
                if todo.is_empty() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
        }
    }

    fn return_signaled(&self, signaled: &Rc<Cell<bool>>) -> bool {
        let returned = Rc::strong_count(signaled) == 1;
        if returned {
            signaled.set(false);
            self.signaled.push(signaled.clone());
        }
        returned
    }
}

impl Drop for Eventfd {
    fn drop(&mut self) {
        if self.signaled.get() {
            self.cache.recycle.push(self.fd.clone());
        }
        self.cache.return_signaled(&self.signaled);
    }
}
