#![allow(dead_code)]

use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        io_uring::IoUring,
        utils::{
            buf::Buf, clonecell::CloneCell, copyhashmap::CopyHashMap, numcell::NumCell,
            oserror::OsError, queue::AsyncQueue, stack::Stack,
        },
        video::drm::{
            syncobj::{SyncObj, SyncObjCtx, SyncObjPoint},
            DrmError,
        },
    },
    futures_util::{select, FutureExt},
    std::rc::Rc,
    uapi::{c, OwnedFd},
};

pub struct WaitForSyncObj {
    inner: Rc<Inner>,
    eng: Rc<AsyncEngine>,
}

pub trait SyncObjWaiter {
    fn done(self: Rc<Self>, result: Result<(), DrmError>);
}

pub struct WaitForSyncObjHandle {
    inner: Rc<Inner>,
    id: u64,
}

struct Inner {
    ctx: CloneCell<Option<Rc<SyncObjCtx>>>,
    next_id: NumCell<u64>,
    ring: Rc<IoUring>,
    busy: CopyHashMap<u64, BusyWaiter>,
    idle: Stack<Waiter>,
}

struct BusyWaiter {
    waiter: Waiter,
    job: Job,
    sow: Rc<dyn SyncObjWaiter>,
}

struct Waiter {
    task: SpawnedFuture<()>,
    inner: Rc<WaiterInner>,
}

#[derive(Clone)]
struct Job {
    id: u64,
    syncobj: Rc<SyncObj>,
    point: SyncObjPoint,
    signaled: bool,
}

struct WaiterInner {
    inner: Rc<Inner>,
    eventfd: Rc<OwnedFd>,
    queue: AsyncQueue<Option<Job>>,
}

impl Drop for WaitForSyncObjHandle {
    fn drop(&mut self) {
        if let Some(waiter) = self.inner.busy.remove(&self.id) {
            waiter.waiter.inner.queue.push(None);
            self.inner.idle.push(waiter.waiter);
        }
    }
}

impl WaitForSyncObj {
    pub fn new(ring: &Rc<IoUring>, eng: &Rc<AsyncEngine>) -> Self {
        Self {
            inner: Rc::new(Inner {
                ctx: Default::default(),
                next_id: Default::default(),
                ring: ring.clone(),
                busy: Default::default(),
                idle: Default::default(),
            }),
            eng: eng.clone(),
        }
    }

    pub fn set_ctx(&self, ctx: Option<Rc<SyncObjCtx>>) {
        self.inner.ctx.set(ctx);
        let busy_waiters: Vec<_> = self.inner.busy.lock().drain().map(|(_, w)| w).collect();
        for mut waiter in busy_waiters {
            waiter.job.id = self.inner.next_id.fetch_add(1);
            waiter.waiter.inner.queue.push(Some(waiter.job.clone()));
            self.inner.busy.set(waiter.job.id, waiter);
        }
    }

    pub fn wait(
        &self,
        syncobj: &Rc<SyncObj>,
        point: SyncObjPoint,
        signaled: bool,
        sow: Rc<dyn SyncObjWaiter>,
    ) -> Result<WaitForSyncObjHandle, DrmError> {
        let waiter = match self.inner.idle.pop() {
            Some(w) => w,
            None => {
                let eventfd = uapi::eventfd(0, c::EFD_CLOEXEC)
                    .map_err(OsError::from)
                    .map_err(DrmError::EventFd)?;
                let waiter = Rc::new(WaiterInner {
                    inner: self.inner.clone(),
                    eventfd: Rc::new(eventfd),
                    queue: AsyncQueue::with_capacity(1),
                });
                Waiter {
                    task: self.eng.spawn(waiter.clone().run()),
                    inner: waiter,
                }
            }
        };
        let job_id = self.inner.next_id.fetch_add(1);
        let job = Job {
            id: job_id,
            syncobj: syncobj.clone(),
            point,
            signaled,
        };
        let waiter = BusyWaiter {
            waiter,
            job: job.clone(),
            sow: sow.clone(),
        };
        waiter.waiter.inner.queue.push(Some(job));
        self.inner.busy.set(job_id, waiter);
        Ok(WaitForSyncObjHandle {
            inner: self.inner.clone(),
            id: job_id,
        })
    }
}

impl Drop for WaitForSyncObj {
    fn drop(&mut self) {
        self.inner.busy.clear();
        self.inner.idle.take();
    }
}

impl WaiterInner {
    async fn run(self: Rc<Self>) {
        let mut buf = Buf::new(8);
        loop {
            let job = self.queue.pop().await;
            let job = match job {
                None => continue,
                Some(j) => j,
            };
            let res = self.wait(&mut buf, &job).await;
            if let Some(waiter) = self.inner.busy.remove(&job.id) {
                waiter.sow.done(res);
                if self.queue.is_empty() {
                    self.inner.idle.push(waiter.waiter);
                }
            }
        }
    }

    async fn wait(&self, buf: &mut Buf, job: &Job) -> Result<(), DrmError> {
        let ctx = match self.inner.ctx.get() {
            None => return Err(DrmError::NoSyncObjContextAvailable),
            Some(c) => c,
        };
        ctx.wait_for_point(&self.eventfd, &job.syncobj, job.point, job.signaled)?;
        loop {
            select! {
                res = self.inner.ring.read(&self.eventfd, buf.clone()).fuse() => {
                    res.map_err(DrmError::ReadEventFd)?;
                    if ctx.is_ready(&job.syncobj, job.point, job.signaled)? {
                        return Ok(());
                    }
                    log::debug!("Spurious wakeup");
                }
                _ = self.queue.non_empty().fuse() => return Ok(()),
            }
        }
    }
}
