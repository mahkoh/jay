use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        io_uring::IoUring,
        utils::{
            asyncevent::AsyncEvent, buf::Buf, clonecell::CloneCell, copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt, numcell::NumCell, oserror::OsErrorExt2, stack::Stack,
        },
        video::drm::{
            DrmError,
            syncobj::{Syncobj, SyncobjCtx, SyncobjPoint},
        },
    },
    std::{cell::Cell, rc::Rc},
    uapi::{OwnedFd, c},
};

pub struct WaitForSyncobj {
    inner: Rc<Inner>,
    eng: Rc<AsyncEngine>,
}

pub trait SyncobjWaiter {
    fn done(self: Rc<Self>, result: Result<(), DrmError>);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct JobId(u64);

#[must_use]
pub struct WaitForSyncobjHandle {
    inner: Rc<Inner>,
    id: JobId,
}

struct Inner {
    ctx: CloneCell<Option<Rc<SyncobjCtx>>>,
    next_id: NumCell<u64>,
    ring: Rc<IoUring>,
    busy: CopyHashMap<JobId, BusyWaiter>,
    idle: Stack<Waiter>,
}

struct BusyWaiter {
    waiter: Waiter,
    job: Job,
    sow: Rc<dyn SyncobjWaiter>,
}

struct Waiter {
    _task: SpawnedFuture<()>,
    inner: Rc<WaiterInner>,
}

#[derive(Clone)]
struct Job {
    id: JobId,
    syncobj: Rc<Syncobj>,
    point: SyncobjPoint,
    signaled: bool,
}

struct WaiterInner {
    inner: Rc<Inner>,
    eventfd: Rc<OwnedFd>,
    next: Cell<Option<Job>>,
    trigger: AsyncEvent,
}

impl Drop for WaitForSyncobjHandle {
    fn drop(&mut self) {
        let _ = self.inner.busy.remove(&self.id);
    }
}

impl WaitForSyncobj {
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

    pub fn clear(&self) {
        self.inner.ctx.take();
        self.inner.busy.clear();
        self.inner.idle.take();
    }

    pub fn set_ctx(&self, ctx: Option<Rc<SyncobjCtx>>) {
        self.inner.ctx.set(ctx);
        let busy_waiters: Vec<_> = self.inner.busy.lock().drain_values().collect();
        for waiter in busy_waiters {
            let res = self.submit_job(
                waiter.job.id,
                &waiter.job.syncobj,
                waiter.job.point,
                waiter.job.signaled,
                waiter.sow.clone(),
            );
            if res.is_err() {
                waiter.sow.done(res);
            }
        }
    }

    pub fn wait(
        &self,
        syncobj: &Rc<Syncobj>,
        point: SyncobjPoint,
        signaled: bool,
        sow: Rc<dyn SyncobjWaiter>,
    ) -> Result<WaitForSyncobjHandle, DrmError> {
        let job_id = JobId(self.inner.next_id.fetch_add(1));
        self.submit_job(job_id, syncobj, point, signaled, sow)?;
        Ok(WaitForSyncobjHandle {
            inner: self.inner.clone(),
            id: job_id,
        })
    }

    fn submit_job(
        &self,
        job_id: JobId,
        syncobj: &Rc<Syncobj>,
        point: SyncobjPoint,
        signaled: bool,
        sow: Rc<dyn SyncobjWaiter>,
    ) -> Result<(), DrmError> {
        let waiter = match self.inner.idle.pop() {
            Some(w) => w,
            None => {
                let eventfd = uapi::eventfd(0, c::EFD_CLOEXEC).map_os_err(DrmError::EventFd)?;
                let waiter = Rc::new(WaiterInner {
                    inner: self.inner.clone(),
                    eventfd: Rc::new(eventfd),
                    next: Cell::new(None),
                    trigger: Default::default(),
                });
                Waiter {
                    _task: self.eng.spawn("wait for syncobj", waiter.clone().run()),
                    inner: waiter,
                }
            }
        };
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
        waiter.waiter.inner.next.set(Some(job));
        waiter.waiter.inner.trigger.trigger();
        self.inner.busy.set(job_id, waiter);
        Ok(())
    }
}

impl WaiterInner {
    async fn run(self: Rc<Self>) {
        let mut buf = Buf::new(8);
        loop {
            self.trigger.triggered().await;
            let job = self.next.take().unwrap();
            let res = self.wait(&mut buf, &job).await;
            if let Some(waiter) = self.inner.busy.remove(&job.id) {
                waiter.sow.done(res);
                self.inner.idle.push(waiter.waiter);
            }
        }
    }

    async fn wait(&self, buf: &mut Buf, job: &Job) -> Result<(), DrmError> {
        let ctx = match self.inner.ctx.get() {
            None => return Err(DrmError::NoSyncobjContextAvailable),
            Some(c) => c,
        };
        ctx.wait_for_point(&self.eventfd, &job.syncobj, job.point, job.signaled)?;
        self.inner
            .ring
            .read(&self.eventfd, buf.clone())
            .await
            .map(drop)
            .map_err(DrmError::ReadEventFd)
    }
}
