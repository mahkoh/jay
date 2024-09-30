pub mod jobs;
#[cfg(test)]
mod tests;

use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        io_uring::IoUring,
        utils::{
            buf::TypedBuf, copyhashmap::CopyHashMap, errorfmt::ErrorFmt, oserror::OsError,
            ptr_ext::MutPtrExt, queue::AsyncQueue, stack::Stack,
        },
    },
    parking_lot::{Condvar, Mutex},
    std::{
        any::Any,
        cell::{Cell, RefCell},
        collections::VecDeque,
        mem,
        ptr::NonNull,
        rc::Rc,
        sync::Arc,
        thread,
    },
    thiserror::Error,
    uapi::{c, OwnedFd},
};

pub trait CpuJob {
    fn work(&mut self) -> &mut dyn CpuWork;
    fn completed(self: Box<Self>);
}

pub trait CpuWork: Send {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>>;

    fn cancel_async(&mut self, ring: &Rc<IoUring>) {
        let _ = ring;
        unreachable!();
    }

    fn async_work_done(&mut self, work: Box<dyn AsyncCpuWork>) {
        let _ = work;
        unreachable!();
    }
}

pub trait AsyncCpuWork {
    fn run(
        self: Box<Self>,
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
        completion: WorkCompletion,
    ) -> SpawnedFuture<CompletedWork>;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub struct WorkCompletion {
    worker: Rc<Worker>,
    id: CpuJobId,
}

pub struct CompletedWork(());

impl WorkCompletion {
    pub fn complete(self, work: Box<dyn AsyncCpuWork>) -> CompletedWork {
        let job = self.worker.async_jobs.remove(&self.id).unwrap();
        unsafe {
            job.work.deref_mut().async_work_done(work);
        }
        self.worker.send_completion(self.id);
        CompletedWork(())
    }
}

pub struct CpuWorker {
    data: Rc<CpuWorkerData>,
    _completions_listener: SpawnedFuture<()>,
    _job_enqueuer: SpawnedFuture<()>,
}

#[must_use]
pub struct PendingJob {
    id: CpuJobId,
    thread_data: Rc<CpuWorkerData>,
    job_data: Rc<PendingJobData>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
enum PendingJobState {
    #[default]
    Waiting,
    Abandoned,
    Completed,
}

#[derive(Default)]
struct PendingJobData {
    job: Cell<Option<NonNull<dyn CpuJob>>>,
    state: Cell<PendingJobState>,
}

enum Job {
    New {
        id: CpuJobId,
        work: *mut dyn CpuWork,
    },
    Cancel {
        id: CpuJobId,
    },
}

unsafe impl Send for Job {}

#[derive(Default)]
struct CompletedJobsExchange {
    queue: VecDeque<CpuJobId>,
    condvar: Option<Arc<Condvar>>,
}

struct CpuWorkerData {
    next: CpuJobIds,
    jobs_to_enqueue: AsyncQueue<Job>,
    new_jobs: Arc<Mutex<VecDeque<Job>>>,
    have_new_jobs: Rc<OwnedFd>,
    completed_jobs_remote: Arc<Mutex<CompletedJobsExchange>>,
    completed_jobs_local: RefCell<VecDeque<CpuJobId>>,
    have_completed_jobs: Rc<OwnedFd>,
    pending_jobs: CopyHashMap<CpuJobId, Rc<PendingJobData>>,
    ring: Rc<IoUring>,
    _stop: OwnedFd,
    pending_job_data_cache: Stack<Rc<PendingJobData>>,
    sync_wake_condvar: Arc<Condvar>,
}

linear_ids!(CpuJobIds, CpuJobId, u64);

#[derive(Debug, Error)]
pub enum CpuWorkerError {
    #[error("Could not create a pipe")]
    Pipe(#[source] OsError),
    #[error("Could not create an eventfd")]
    EventFd(#[source] OsError),
    #[error("Could not dup an eventfd")]
    Dup(#[source] OsError),
}

impl PendingJob {
    pub fn detach(self) {
        match self.job_data.state.get() {
            PendingJobState::Waiting => {
                self.job_data.state.set(PendingJobState::Abandoned);
            }
            PendingJobState::Abandoned => {
                unreachable!();
            }
            PendingJobState::Completed => {}
        }
    }
}

impl Drop for CpuWorker {
    fn drop(&mut self) {
        self.data.do_equeue_jobs();
        if self.data.pending_jobs.is_not_empty() {
            log::warn!("CpuWorker dropped with pending jobs. Completed jobs will not be triggered.")
        }
    }
}

impl Drop for PendingJob {
    fn drop(&mut self) {
        match self.job_data.state.get() {
            PendingJobState::Waiting => {
                log::warn!("PendingJob dropped before completion. Blocking.");
                let data = &self.thread_data;
                let id = self.id;
                self.job_data.state.set(PendingJobState::Abandoned);
                data.jobs_to_enqueue.push(Job::Cancel { id });
                data.do_equeue_jobs();
                loop {
                    data.dispatch_completions();
                    if !data.pending_jobs.contains(&id) {
                        break;
                    }
                    let mut remote = data.completed_jobs_remote.lock();
                    while remote.queue.is_empty() {
                        remote.condvar = Some(data.sync_wake_condvar.clone());
                        data.sync_wake_condvar.wait(&mut remote);
                    }
                }
            }
            PendingJobState::Abandoned => {}
            PendingJobState::Completed => {
                self.thread_data
                    .pending_job_data_cache
                    .push(self.job_data.clone());
            }
        }
    }
}

impl CpuWorkerData {
    async fn wait_for_completions(self: Rc<Self>) {
        let mut buf = TypedBuf::<u64>::new();
        loop {
            if let Err(e) = self.ring.read(&self.have_completed_jobs, buf.buf()).await {
                log::error!("Could not wait for job completions: {}", ErrorFmt(e));
                return;
            }
            self.dispatch_completions();
        }
    }

    fn dispatch_completions(&self) {
        let completions = &mut *self.completed_jobs_local.borrow_mut();
        mem::swap(completions, &mut self.completed_jobs_remote.lock().queue);
        while let Some(id) = completions.pop_front() {
            let job_data = self.pending_jobs.remove(&id).unwrap();
            let job = job_data.job.take().unwrap();
            let job = unsafe { Box::from_raw(job.as_ptr()) };
            match job_data.state.get() {
                PendingJobState::Waiting => {
                    job_data.state.set(PendingJobState::Completed);
                    job.completed();
                }
                PendingJobState::Abandoned => {
                    self.pending_job_data_cache.push(job_data);
                }
                PendingJobState::Completed => {
                    unreachable!();
                }
            }
        }
    }

    async fn equeue_jobs(self: Rc<Self>) {
        loop {
            self.jobs_to_enqueue.non_empty().await;
            self.do_equeue_jobs();
        }
    }

    fn do_equeue_jobs(&self) {
        self.jobs_to_enqueue.move_to(&mut self.new_jobs.lock());
        if let Err(e) = uapi::eventfd_write(self.have_new_jobs.raw(), 1) {
            panic!("Could not signal eventfd: {}", ErrorFmt(e));
        }
    }
}

impl CpuWorker {
    pub fn new(ring: &Rc<IoUring>, eng: &Rc<AsyncEngine>) -> Result<Self, CpuWorkerError> {
        let new_jobs: Arc<Mutex<VecDeque<Job>>> = Default::default();
        let completed_jobs: Arc<Mutex<CompletedJobsExchange>> = Default::default();
        let (stop_read, stop_write) =
            uapi::pipe2(c::O_CLOEXEC).map_err(|e| CpuWorkerError::Pipe(e.into()))?;
        let have_new_jobs =
            uapi::eventfd(0, c::EFD_CLOEXEC).map_err(|e| CpuWorkerError::EventFd(e.into()))?;
        let have_completed_jobs =
            uapi::eventfd(0, c::EFD_CLOEXEC).map_err(|e| CpuWorkerError::EventFd(e.into()))?;
        thread::Builder::new()
            .name("cpu worker".to_string())
            .spawn({
                let new_jobs = new_jobs.clone();
                let completed_jobs = completed_jobs.clone();
                let have_new_jobs = uapi::fcntl_dupfd_cloexec(have_new_jobs.raw(), 0)
                    .map_err(|e| CpuWorkerError::Dup(e.into()))?;
                let have_completed_jobs = uapi::fcntl_dupfd_cloexec(have_completed_jobs.raw(), 0)
                    .map_err(|e| CpuWorkerError::Dup(e.into()))?;
                move || {
                    work(
                        new_jobs,
                        completed_jobs,
                        stop_write,
                        have_new_jobs,
                        have_completed_jobs,
                    )
                }
            })
            .unwrap();
        let data = Rc::new(CpuWorkerData {
            next: Default::default(),
            jobs_to_enqueue: Default::default(),
            new_jobs,
            have_new_jobs: Rc::new(have_new_jobs),
            completed_jobs_remote: completed_jobs,
            completed_jobs_local: Default::default(),
            have_completed_jobs: Rc::new(have_completed_jobs),
            pending_jobs: Default::default(),
            ring: ring.clone(),
            _stop: stop_read,
            pending_job_data_cache: Default::default(),
            sync_wake_condvar: Arc::new(Condvar::new()),
        });
        Ok(Self {
            _completions_listener: eng.spawn(
                "cpu worker completions",
                data.clone().wait_for_completions(),
            ),
            _job_enqueuer: eng.spawn("cpu worker enqueue", data.clone().equeue_jobs()),
            data,
        })
    }

    pub fn submit(&self, job: Box<dyn CpuJob>) -> PendingJob {
        let mut job = NonNull::from(Box::leak(job));
        let id = self.data.next.next();
        self.data.jobs_to_enqueue.push(Job::New {
            id,
            work: unsafe { job.as_mut().work() },
        });
        let job_data = self.data.pending_job_data_cache.pop().unwrap_or_default();
        job_data.job.set(Some(job));
        job_data.state.set(PendingJobState::Waiting);
        self.data.pending_jobs.set(id, job_data.clone());
        PendingJob {
            id,
            thread_data: self.data.clone(),
            job_data,
        }
    }

    #[cfg(feature = "it")]
    pub fn wait_idle(&self) -> bool {
        let was_idle = self.data.pending_jobs.is_empty();
        loop {
            self.data.dispatch_completions();
            if self.data.pending_jobs.is_empty() {
                break;
            }
            let mut remote = self.data.completed_jobs_remote.lock();
            while remote.queue.is_empty() {
                remote.condvar = Some(self.data.sync_wake_condvar.clone());
                self.data.sync_wake_condvar.wait(&mut remote);
            }
        }
        was_idle
    }
}

fn work(
    new_jobs: Arc<Mutex<VecDeque<Job>>>,
    completed_jobs: Arc<Mutex<CompletedJobsExchange>>,
    stop: OwnedFd,
    have_new_jobs: OwnedFd,
    have_completed_jobs: OwnedFd,
) {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let worker = Rc::new(Worker {
        eng,
        ring,
        completed_jobs,
        have_completed_jobs,
        async_jobs: Default::default(),
        stopped: Cell::new(false),
    });
    let _stop_listener = worker
        .eng
        .spawn("stop listener", worker.clone().handle_stop(stop));
    let _new_job_listener = worker.eng.spawn(
        "new job listener",
        worker.clone().handle_new_jobs(new_jobs, have_new_jobs),
    );
    if let Err(e) = worker.ring.run() {
        panic!("io_uring failed: {}", ErrorFmt(e));
    }
}

struct Worker {
    eng: Rc<AsyncEngine>,
    ring: Rc<IoUring>,
    completed_jobs: Arc<Mutex<CompletedJobsExchange>>,
    have_completed_jobs: OwnedFd,
    async_jobs: CopyHashMap<CpuJobId, AsyncJob>,
    stopped: Cell<bool>,
}

struct AsyncJob {
    _future: SpawnedFuture<CompletedWork>,
    work: *mut dyn CpuWork,
}

impl Worker {
    async fn handle_stop(self: Rc<Self>, stop: OwnedFd) {
        let stop = Rc::new(stop);
        if let Err(e) = self.ring.poll(&stop, 0).await {
            log::error!(
                "Could not wait for stop fd to become readable: {}",
                ErrorFmt(e)
            );
        } else {
            assert!(self.async_jobs.is_empty());
            self.stopped.set(true);
            self.ring.stop();
        }
    }

    async fn handle_new_jobs(
        self: Rc<Self>,
        jobs_remote: Arc<Mutex<VecDeque<Job>>>,
        new_jobs: OwnedFd,
    ) {
        let mut buf = TypedBuf::<u64>::new();
        let new_jobs = Rc::new(new_jobs);
        let mut jobs = VecDeque::new();
        loop {
            if let Err(e) = self.ring.read(&new_jobs, buf.buf()).await {
                if self.stopped.get() {
                    return;
                }
                panic!(
                    "Could not wait for new jobs fd to be signaled: {}",
                    ErrorFmt(e),
                );
            }
            mem::swap(&mut jobs, &mut *jobs_remote.lock());
            while let Some(job) = jobs.pop_front() {
                self.handle_new_job(job);
            }
        }
    }

    fn handle_new_job(self: &Rc<Self>, job: Job) {
        match job {
            Job::Cancel { id } => {
                let mut jobs = self.async_jobs.lock();
                if let Some(job) = jobs.get_mut(&id) {
                    unsafe {
                        job.work.deref_mut().cancel_async(&self.ring);
                    }
                }
            }
            Job::New { id, work } => match unsafe { work.deref_mut() }.run() {
                None => {
                    self.send_completion(id);
                    return;
                }
                Some(w) => {
                    let completion = WorkCompletion {
                        worker: self.clone(),
                        id,
                    };
                    let future = w.run(&self.eng, &self.ring, completion);
                    self.async_jobs.set(
                        id,
                        AsyncJob {
                            _future: future,
                            work,
                        },
                    );
                }
            },
        }
    }

    fn send_completion(&self, id: CpuJobId) {
        let cv = {
            let mut exchange = self.completed_jobs.lock();
            exchange.queue.push_back(id);
            exchange.condvar.take()
        };
        if let Some(cv) = cv {
            cv.notify_all();
        }
        if let Err(e) = uapi::eventfd_write(self.have_completed_jobs.raw(), 1) {
            panic!("Could not signal job completion: {}", ErrorFmt(e));
        }
    }
}
