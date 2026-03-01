use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        io_uring::{IoUring, IoUringError},
        utils::{buf::Buf, errorfmt::ErrorFmt, oserror::OsError, queue::AsyncQueue, stack::Stack},
    },
    std::{cell::Cell, future::poll_fn, pin::Pin, rc::Rc, slice, task::Poll},
    thiserror::Error,
    uapi::{OwnedFd, c},
};

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
    recycle: AsyncQueue<Rc<OwnedFd>>,
}

pub struct Eventfd {
    cache: Rc<Inner>,
    pub fd: Rc<OwnedFd>,
    signaled: Cell<bool>,
}

impl EventfdCache {
    pub fn new(ring: &Rc<IoUring>, eng: &Rc<AsyncEngine>) -> Rc<Self> {
        let inner = Rc::new(Inner {
            ring: ring.clone(),
            fds: Default::default(),
            recycle: Default::default(),
        });
        let task = eng.spawn("eventfd-cache", inner.clone().recycle());
        Rc::new(Self { inner, _task: task })
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn acquire(&self) -> Result<Eventfd, EventfdError> {
        let fd = match self.inner.fds.pop() {
            Some(fd) => fd,
            _ => uapi::eventfd(0, c::EFD_CLOEXEC)
                .map(Rc::new)
                .map_err(Into::into)
                .map_err(EventfdError::CreateEventfd)?,
        };
        Ok(Eventfd {
            cache: self.inner.clone(),
            fd,
            signaled: Default::default(),
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
        uapi::poll(slice::from_mut(&mut pollfd), -1)?;
        self.signaled.set(true);
        Ok(())
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
}

impl Drop for Eventfd {
    fn drop(&mut self) {
        if self.signaled.get() {
            self.cache.recycle.push(self.fd.clone());
        }
    }
}
