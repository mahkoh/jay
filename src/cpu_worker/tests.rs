use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        cpu_worker::{AsyncCpuWork, CompletedWork, CpuJob, CpuWork, CpuWorker, WorkCompletion},
        io_uring::IoUring,
        utils::asyncevent::AsyncEvent,
        wheel::Wheel,
    },
    std::{any::Any, future::pending, rc::Rc, sync::Arc},
    uapi::{OwnedFd, c::EFD_CLOEXEC},
};

struct Job {
    ae: Rc<AsyncEvent>,
    work: Work,
    cancel: bool,
}
struct Work(Arc<OwnedFd>);
struct AsyncWork(Arc<OwnedFd>);

impl CpuJob for Job {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        if self.cancel {
            unreachable!();
        } else {
            self.ae.trigger();
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if self.cancel {
            self.ae.trigger();
        }
    }
}

impl CpuWork for Work {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>> {
        Some(Box::new(AsyncWork(self.0.clone())))
    }

    fn cancel_async(&mut self, _ring: &Rc<IoUring>) {
        uapi::eventfd_write(self.0.raw(), 1).unwrap();
    }

    fn async_work_done(&mut self, work: Box<dyn AsyncCpuWork>) {
        let _ = work;
    }
}

impl AsyncCpuWork for AsyncWork {
    fn run(
        self: Box<Self>,
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
        completion: WorkCompletion,
    ) -> SpawnedFuture<CompletedWork> {
        let ring = ring.clone();
        eng.spawn("", async move {
            let mut buf = [0; 8];
            let res = ring
                .read_no_cancel(self.0.borrow(), 0, &mut buf, |_| ())
                .await;
            res.unwrap();
            completion.complete(self)
        })
    }
}

fn run(cancel: bool) {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let ring2 = ring.clone();
    let wheel = Wheel::new(&eng, &ring).unwrap();
    let cpu = Rc::new(CpuWorker::new(&ring, &eng).unwrap());
    let ae = Rc::new(AsyncEvent::default());
    let eventfd = Arc::new(uapi::eventfd(0, EFD_CLOEXEC).unwrap());
    let pending_job = cpu.submit(Box::new(Job {
        ae: ae.clone(),
        work: Work(eventfd.clone()),
        cancel,
    }));
    let _fut1 = eng.spawn("", async move {
        wheel.timeout(1).await.unwrap();
        if cancel {
            drop(pending_job);
        } else {
            uapi::eventfd_write(eventfd.raw(), 1).unwrap();
            pending::<()>().await;
        }
    });
    let _fut2 = eng.spawn("", async move {
        ae.triggered().await;
        ring2.stop();
    });
    ring.run().unwrap();
}

#[test]
fn cancel() {
    run(true);
}

#[test]
fn complete() {
    run(false);
}
