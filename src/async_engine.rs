mod ae_fd;
mod ae_queue;
mod ae_task;
mod ae_yield;

pub use {
    crate::async_engine::ae_yield::Yield,
    ae_fd::{AsyncFd, FdStatus},
    ae_task::SpawnedFuture,
};
use {
    crate::{
        event_loop::{EventLoop, EventLoopError},
        utils::{copyhashmap::CopyHashMap, numcell::NumCell},
    },
    ae_fd::AsyncFdData,
    ae_queue::{DispatchQueue, Dispatcher},
    std::{
        cell::{Cell, RefCell},
        future::Future,
        rc::Rc,
    },
    thiserror::Error,
    uapi::OwnedFd,
};

#[derive(Debug, Error)]
pub enum AsyncError {
    #[error("The event loop caused an error")]
    EventLoopError(#[from] EventLoopError),
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Phase {
    EventHandling,
    Layout,
    PostLayout,
    Present,
}
const NUM_PHASES: usize = 4;

pub struct AsyncEngine {
    el: Rc<EventLoop>,
    queue: Rc<DispatchQueue>,
    fds: CopyHashMap<i32, Rc<AsyncFdData>>,
}

impl AsyncEngine {
    pub fn install(el: &Rc<EventLoop>) -> Result<Rc<Self>, AsyncError> {
        let queue = Dispatcher::install(el)?;
        Ok(Rc::new(Self {
            el: el.clone(),
            queue,
            fds: CopyHashMap::new(),
        }))
    }

    pub fn clear(&self) {
        for (_, fd) in self.fds.lock().drain() {
            fd.readers.take();
            fd.writers.take();
        }
        self.queue.clear();
    }

    pub fn spawn<T, F: Future<Output = T> + 'static>(&self, f: F) -> SpawnedFuture<T> {
        self.queue.spawn(Phase::EventHandling, f)
    }

    pub fn spawn2<T, F: Future<Output = T> + 'static>(
        &self,
        phase: Phase,
        f: F,
    ) -> SpawnedFuture<T> {
        self.queue.spawn(phase, f)
    }

    pub fn fd(self: &Rc<Self>, fd: &Rc<OwnedFd>) -> Result<AsyncFd, AsyncError> {
        let data = if let Some(afd) = self.fds.get(&fd.raw()) {
            afd.ref_count.fetch_add(1);
            afd
        } else {
            let id = self.el.id();
            let afd = Rc::new(AsyncFdData {
                ref_count: NumCell::new(1),
                fd: fd.clone(),
                id,
                el: self.el.clone(),
                write_registered: Cell::new(false),
                read_registered: Cell::new(false),
                readers: RefCell::new(vec![]),
                writers: RefCell::new(vec![]),
            });
            self.el.insert(id, Some(fd.raw()), 0, afd.clone())?;
            afd
        };
        Ok(AsyncFd {
            engine: self.clone(),
            data,
        })
    }

    pub fn yield_now(&self) -> Yield {
        Yield {
            iteration: self.queue.iteration(),
            queue: self.queue.clone(),
        }
    }
}
