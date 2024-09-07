use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        cpu_worker::{AsyncCpuWork, CompletedWork, CpuWork, WorkCompletion},
        io_uring::{IoUring, IoUringError, IoUringTaskId},
    },
    std::{
        any::Any,
        ptr,
        rc::Rc,
        slice,
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
            Arc,
        },
    },
    thiserror::Error,
    uapi::{c, Fd},
};

#[derive(Debug, Error)]
pub enum ReadWriteJobError {
    #[error("An io_uring error occurred")]
    IoUring(#[source] IoUringError),
    #[error("The job was cancelled")]
    Cancelled,
    #[error("Tried to operate outside the bounds of the file descriptor")]
    OutOfBounds,
}

pub struct ReadWriteWork {
    cancel: Arc<CancelState>,
    config: Option<Box<ReadWriteWorkConfig>>,
}

unsafe impl Send for ReadWriteWork {}

impl ReadWriteWork {
    #[expect(dead_code)]
    pub unsafe fn new() -> Self {
        let cancel = Arc::new(CancelState::default());
        ReadWriteWork {
            cancel: cancel.clone(),
            config: Some(Box::new(ReadWriteWorkConfig {
                fd: -1,
                offset: 0,
                ptr: ptr::null_mut(),
                len: 0,
                write: false,
                cancel,
                result: None,
            })),
        }
    }

    #[expect(dead_code)]
    pub fn config(&mut self) -> &mut ReadWriteWorkConfig {
        self.config.as_mut().unwrap()
    }
}

pub struct ReadWriteWorkConfig {
    pub fd: c::c_int,
    pub offset: usize,
    pub ptr: *mut u8,
    pub len: usize,
    pub write: bool,
    pub result: Option<Result<(), ReadWriteJobError>>,
    cancel: Arc<CancelState>,
}

#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    cancel_id: AtomicU64,
}

impl CpuWork for ReadWriteWork {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>> {
        self.cancel.cancelled.store(false, Relaxed);
        self.cancel.cancel_id.store(0, Relaxed);
        self.config.take().map(|b| b as _)
    }

    fn cancel_async(&mut self, ring: &Rc<IoUring>) {
        self.cancel.cancelled.store(true, Relaxed);
        let id = self.cancel.cancel_id.load(Relaxed);
        if id != 0 {
            ring.cancel(IoUringTaskId::from_raw(id));
        }
    }

    fn async_work_done(&mut self, work: Box<dyn AsyncCpuWork>) {
        let work = work.into_any().downcast().unwrap();
        self.config = Some(work);
    }
}

impl AsyncCpuWork for ReadWriteWorkConfig {
    fn run(
        mut self: Box<Self>,
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
        completion: WorkCompletion,
    ) -> SpawnedFuture<CompletedWork> {
        let ring = ring.clone();
        eng.spawn(async move {
            let res = loop {
                if self.cancel.cancelled.load(Relaxed) {
                    break Err(ReadWriteJobError::Cancelled);
                }
                if self.len == 0 {
                    break Ok(());
                };
                let res = if self.write {
                    ring.write_no_cancel(
                        Fd::new(self.fd),
                        self.offset,
                        unsafe { slice::from_raw_parts(self.ptr, self.len) },
                        None,
                        |id| self.cancel.cancel_id.store(id.raw(), Relaxed),
                    )
                    .await
                } else {
                    ring.read_no_cancel(
                        Fd::new(self.fd),
                        self.offset,
                        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) },
                        |id| self.cancel.cancel_id.store(id.raw(), Relaxed),
                    )
                    .await
                };
                match res {
                    Ok(0) => break Err(ReadWriteJobError::OutOfBounds),
                    Ok(n) => {
                        self.len -= n;
                        self.offset += n;
                        unsafe {
                            self.ptr = self.ptr.add(n);
                        }
                    }
                    Err(e) => break Err(ReadWriteJobError::IoUring(e)),
                }
            };
            self.result = Some(res);
            completion.complete(self)
        })
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}
