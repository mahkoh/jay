pub use {
    crate::async_engine::yield_::Yield,
    fd::{AsyncFd, FdStatus},
    task::SpawnedFuture,
    timeout::Timeout,
    timer::Timer,
};
use {
    crate::{
        event_loop::{EventLoop, EventLoopError},
        utils::{copyhashmap::CopyHashMap, numcell::NumCell, oserror::OsError},
        wheel::{Wheel, WheelError},
    },
    fd::AsyncFdData,
    queue::{DispatchQueue, Dispatcher},
    std::{
        cell::{Cell, RefCell},
        future::Future,
        rc::Rc,
    },
    thiserror::Error,
    timeout::TimeoutData,
    uapi::{c, OwnedFd},
};

#[derive(Debug, Error)]
pub enum AsyncError {
    #[error("The timer wheel returned an error")]
    WheelError(#[from] WheelError),
    #[error("The event loop caused an error")]
    EventLoopError(#[from] EventLoopError),
    #[error("Could not read from a timer")]
    TimerReadError(#[source] OsError),
    #[error("Could not set a timer")]
    SetTimer(#[source] OsError),
    #[error("Could not create a timer")]
    CreateTimer(#[source] OsError),
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
    wheel: Rc<Wheel>,
    el: Rc<EventLoop>,
    queue: Rc<DispatchQueue>,
    fds: CopyHashMap<i32, Rc<AsyncFdData>>,
}

impl AsyncEngine {
    pub fn install(el: &Rc<EventLoop>, wheel: &Rc<Wheel>) -> Result<Rc<Self>, AsyncError> {
        let queue = Dispatcher::install(el)?;
        Ok(Rc::new(Self {
            wheel: wheel.clone(),
            el: el.clone(),
            queue,
            fds: CopyHashMap::new(),
        }))
    }

    pub fn timeout(&self, ms: u64) -> Result<Timeout, AsyncError> {
        let data = Rc::new(TimeoutData {
            expired: Cell::new(false),
            waker: RefCell::new(None),
        });
        let id = self.wheel.id();
        self.wheel.timeout(id, ms, data.clone())?;
        Ok(Timeout {
            id,
            wheel: self.wheel.clone(),
            data,
        })
    }

    pub fn timer(self: &Rc<Self>, clock_id: c::c_int) -> Result<Timer, AsyncError> {
        Timer::new(self, clock_id)
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

mod yield_ {
    use {
        crate::async_engine::queue::DispatchQueue,
        std::{
            future::Future,
            pin::Pin,
            rc::Rc,
            task::{Context, Poll},
        },
    };

    pub struct Yield {
        pub(super) iteration: u64,
        pub(super) queue: Rc<DispatchQueue>,
    }

    impl Future for Yield {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.queue.iteration() > self.iteration {
                Poll::Ready(())
            } else {
                self.queue.push_yield(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

mod timer {
    use {
        crate::async_engine::{AsyncEngine, AsyncError, AsyncFd},
        std::{rc::Rc, time::Duration},
        uapi::c,
    };

    #[derive(Clone)]
    pub struct Timer {
        fd: AsyncFd,
    }

    impl Timer {
        pub(super) fn new(eng: &Rc<AsyncEngine>, clock_id: c::c_int) -> Result<Self, AsyncError> {
            let fd = match uapi::timerfd_create(clock_id, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
                Ok(fd) => fd,
                Err(e) => return Err(AsyncError::CreateTimer(e.into())),
            };
            let afd = eng.fd(&Rc::new(fd))?;
            Ok(Self { fd: afd })
        }

        pub async fn expired(&self) -> Result<u64, AsyncError> {
            self.fd.readable().await?;
            let mut buf = 0u64;
            if let Err(e) = uapi::read(self.fd.raw(), &mut buf) {
                return Err(AsyncError::TimerReadError(e.into()));
            }
            Ok(buf)
        }

        pub fn program(
            &self,
            initial: Option<Duration>,
            periodic: Option<Duration>,
        ) -> Result<(), AsyncError> {
            let mut timerspec: c::itimerspec = uapi::pod_zeroed();
            if let Some(init) = initial {
                timerspec.it_value.tv_sec = init.as_secs() as _;
                timerspec.it_value.tv_nsec = init.subsec_nanos() as _;
                if let Some(per) = periodic {
                    timerspec.it_interval.tv_sec = per.as_secs() as _;
                    timerspec.it_interval.tv_nsec = per.subsec_nanos() as _;
                }
            }
            if let Err(e) = uapi::timerfd_settime(self.fd.raw(), 0, &timerspec) {
                return Err(AsyncError::SetTimer(e.into()));
            }
            Ok(())
        }
    }
}

mod timeout {
    use {
        crate::wheel::{Wheel, WheelDispatcher, WheelId},
        std::{
            cell::{Cell, RefCell},
            error::Error,
            future::Future,
            pin::Pin,
            rc::Rc,
            task::{Context, Poll, Waker},
        },
    };

    pub(super) struct TimeoutData {
        pub expired: Cell<bool>,
        pub waker: RefCell<Option<Waker>>,
    }

    impl WheelDispatcher for TimeoutData {
        fn dispatch(self: Rc<Self>) -> Result<(), Box<dyn Error>> {
            self.expired.set(true);
            if let Some(w) = self.waker.borrow_mut().take() {
                w.wake();
            }
            Ok(())
        }
    }

    pub struct Timeout {
        pub(super) id: WheelId,
        pub(super) wheel: Rc<Wheel>,
        pub(super) data: Rc<TimeoutData>,
    }

    impl Future for Timeout {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.data.expired.get() {
                Poll::Ready(())
            } else {
                *self.data.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    impl Drop for Timeout {
        fn drop(&mut self) {
            self.wheel.remove(self.id);
        }
    }
}

mod task {
    use {
        crate::{
            async_engine::{queue::DispatchQueue, Phase},
            utils::{
                numcell::NumCell,
                ptr_ext::{MutPtrExt, PtrExt},
            },
        },
        std::{
            cell::{Cell, UnsafeCell},
            future::Future,
            mem::ManuallyDrop,
            pin::Pin,
            ptr,
            rc::Rc,
            task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
        },
    };

    #[must_use]
    pub struct SpawnedFuture<T: 'static> {
        vtable: &'static SpawnedFutureVtable<T>,
        data: *mut u8,
    }

    impl<T> Future for SpawnedFuture<T> {
        type Output = T;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            unsafe { (self.vtable.poll)(self.data, cx) }
        }
    }

    impl<T> Drop for SpawnedFuture<T> {
        fn drop(&mut self) {
            unsafe {
                (self.vtable.drop)(self.data);
            }
        }
    }

    struct SpawnedFutureVTableProxy<T, F>(T, F);

    impl<T: 'static, F: Future<Output = T>> SpawnedFutureVTableProxy<T, F> {
        const VTABLE: &'static SpawnedFutureVtable<T> = &SpawnedFutureVtable {
            poll: Self::poll,
            drop: Self::drop,
        };

        unsafe fn poll(data: *mut u8, ctx: &mut Context<'_>) -> Poll<T> {
            let task = (data as *const Task<T, F>).deref();
            if &task.state & COMPLETED == 0 {
                task.waker.set(Some(ctx.waker().clone()));
                Poll::Pending
            } else if &task.state & EMPTIED == 0 {
                task.state.or_assign(EMPTIED);
                Poll::Ready(ptr::read(&*task.data.get().deref().result))
            } else {
                panic!("Future polled after it has already been emptied");
            }
        }

        unsafe fn drop(data: *mut u8) {
            {
                let task = (data as *const Task<T, F>).deref();
                task.state.or_assign(CANCELLED);
                if &task.state & RUNNING == 0 {
                    task.drop_data();
                }
            }
            Task::<T, F>::dec_ref_count(data as _);
        }
    }

    struct SpawnedFutureVtable<T> {
        poll: unsafe fn(data: *mut u8, ctx: &mut Context<'_>) -> Poll<T>,
        drop: unsafe fn(data: *mut u8),
    }

    union TaskData<T, F: Future<Output = T>> {
        result: ManuallyDrop<T>,
        future: ManuallyDrop<F>,
    }

    const RUNNING: u32 = 1;
    const RUN_AGAIN: u32 = 2;
    const COMPLETED: u32 = 4;
    const EMPTIED: u32 = 8;
    const CANCELLED: u32 = 16;

    struct Task<T, F: Future<Output = T>> {
        ref_count: NumCell<u64>,
        phase: Phase,
        state: NumCell<u32>,
        data: UnsafeCell<TaskData<T, F>>,
        waker: Cell<Option<Waker>>,
        queue: Rc<DispatchQueue>,
    }

    pub(super) struct Runnable {
        data: *const u8,
        run: unsafe fn(data: *const u8, run: bool),
    }

    impl Runnable {
        pub(super) fn run(self) {
            let slf = ManuallyDrop::new(self);
            unsafe {
                (slf.run)(slf.data, true);
            }
        }
    }

    impl Drop for Runnable {
        fn drop(&mut self) {
            unsafe {
                (self.run)(self.data, false);
            }
        }
    }

    impl DispatchQueue {
        pub(super) fn spawn<T, F: Future<Output = T>>(
            self: &Rc<Self>,
            phase: Phase,
            f: F,
        ) -> SpawnedFuture<T> {
            let f = Box::new(Task {
                ref_count: NumCell::new(1),
                phase,
                state: NumCell::new(0),
                data: UnsafeCell::new(TaskData {
                    future: ManuallyDrop::new(f),
                }),
                waker: Cell::new(None),
                queue: self.clone(),
            });
            unsafe {
                f.schedule_run();
            }
            let f = Box::into_raw(f);
            SpawnedFuture {
                vtable: SpawnedFutureVTableProxy::<T, F>::VTABLE,
                data: f as _,
            }
        }
    }

    impl<T, F: Future<Output = T>> Task<T, F> {
        const VTABLE: &'static RawWakerVTable = &RawWakerVTable::new(
            Self::waker_clone,
            Self::waker_wake,
            Self::waker_wake_by_ref,
            Self::waker_drop,
        );

        unsafe fn run_proxy(data: *const u8, run: bool) {
            let task = data as *const Self;
            if run {
                task.deref().run();
            }
            Self::dec_ref_count(task);
        }

        unsafe fn dec_ref_count(slf: *const Self) {
            if slf.deref().ref_count.fetch_sub(1) == 1 {
                Box::from_raw(slf as *mut Self);
            }
        }

        unsafe fn inc_ref_count(&self) {
            self.ref_count.fetch_add(1);
        }

        unsafe fn waker_clone(data: *const ()) -> RawWaker {
            let task = &mut *(data as *mut Self);
            task.inc_ref_count();
            RawWaker::new(data, Self::VTABLE)
        }

        unsafe fn waker_wake(data: *const ()) {
            Self::waker_wake_by_ref(data);
            Self::waker_drop(data);
        }

        unsafe fn waker_wake_by_ref(data: *const ()) {
            (data as *const Self).deref().schedule_run();
        }

        unsafe fn waker_drop(data: *const ()) {
            Self::dec_ref_count(data as _)
        }

        unsafe fn schedule_run(&self) {
            if &self.state & (COMPLETED | CANCELLED) == 0 {
                if &self.state & RUNNING == 0 {
                    self.state.or_assign(RUNNING);
                    self.inc_ref_count();
                    let data = self as *const _ as _;
                    self.queue.push(
                        Runnable {
                            data,
                            run: Self::run_proxy,
                        },
                        self.phase,
                    );
                } else {
                    self.state.or_assign(RUN_AGAIN);
                }
            }
        }

        unsafe fn run(&self) {
            if &self.state & CANCELLED == 0 {
                let data = self.data.get().deref_mut();
                self.inc_ref_count();
                let raw_waker = RawWaker::new(self as *const _ as _, Self::VTABLE);
                let waker = Waker::from_raw(raw_waker);

                let mut ctx = Context::from_waker(&waker);
                if let Poll::Ready(d) = Pin::new_unchecked(&mut *data.future).poll(&mut ctx) {
                    ManuallyDrop::drop(&mut data.future);
                    ptr::write(&mut data.result, ManuallyDrop::new(d));
                    self.state.or_assign(COMPLETED);
                    if let Some(waker) = self.waker.take() {
                        waker.wake();
                    }
                }
            }

            self.state.and_assign(!RUNNING);

            if &self.state & CANCELLED != 0 {
                self.drop_data();
            } else if &self.state & RUN_AGAIN != 0 {
                self.state.and_assign(!RUN_AGAIN);
                self.schedule_run()
            }
        }

        unsafe fn drop_data(&self) {
            if &self.state & COMPLETED == 0 {
                ManuallyDrop::drop(&mut self.data.get().deref_mut().future);
            } else if &self.state & EMPTIED == 0 {
                ManuallyDrop::drop(&mut self.data.get().deref_mut().result);
            }
        }
    }
}

mod queue {
    use {
        crate::{
            async_engine::{task::Runnable, AsyncError, Phase, NUM_PHASES},
            event_loop::{EventLoop, EventLoopDispatcher, EventLoopId},
            utils::{array, numcell::NumCell, syncqueue::SyncQueue},
        },
        std::{
            cell::{Cell, RefCell},
            collections::VecDeque,
            error::Error,
            rc::Rc,
            task::Waker,
        },
    };

    pub(super) struct Dispatcher {
        queue: Rc<DispatchQueue>,
        stash: RefCell<VecDeque<Runnable>>,
        yield_stash: RefCell<VecDeque<Waker>>,
    }

    impl Dispatcher {
        pub fn install(el: &Rc<EventLoop>) -> Result<Rc<DispatchQueue>, AsyncError> {
            let id = el.id();
            let queue = Rc::new(DispatchQueue {
                id,
                el: el.clone(),
                dispatch_scheduled: Cell::new(false),
                num_queued: Default::default(),
                queues: array::from_fn(|_| Default::default()),
                iteration: Default::default(),
                yields: Default::default(),
            });
            let slf = Rc::new(Dispatcher {
                queue: queue.clone(),
                stash: Default::default(),
                yield_stash: Default::default(),
            });
            el.insert(id, None, 0, slf)?;
            Ok(queue)
        }
    }

    impl EventLoopDispatcher for Dispatcher {
        fn dispatch(self: Rc<Self>, _fd: Option<i32>, _events: i32) -> Result<(), Box<dyn Error>> {
            let mut stash = self.stash.borrow_mut();
            let mut yield_stash = self.yield_stash.borrow_mut();
            while self.queue.num_queued.get() > 0 {
                self.queue.iteration.fetch_add(1);
                let mut phase = 0;
                while phase < NUM_PHASES as usize {
                    self.queue.queues[phase].swap(&mut *stash);
                    if stash.is_empty() {
                        phase += 1;
                        continue;
                    }
                    self.queue.num_queued.fetch_sub(stash.len());
                    for runnable in stash.drain(..) {
                        runnable.run();
                    }
                }
                self.queue.yields.swap(&mut *yield_stash);
                for waker in yield_stash.drain(..) {
                    waker.wake();
                }
            }
            self.queue.dispatch_scheduled.set(false);
            Ok(())
        }
    }

    impl Drop for Dispatcher {
        fn drop(&mut self) {
            let _ = self.queue.el.remove(self.queue.id);
            for queue in &self.queue.queues {
                queue.swap(&mut VecDeque::new());
            }
        }
    }

    pub(super) struct DispatchQueue {
        dispatch_scheduled: Cell<bool>,
        id: EventLoopId,
        el: Rc<EventLoop>,
        num_queued: NumCell<usize>,
        queues: [SyncQueue<Runnable>; NUM_PHASES],
        iteration: NumCell<u64>,
        yields: SyncQueue<Waker>,
    }

    impl DispatchQueue {
        pub fn push(&self, runnable: Runnable, phase: Phase) {
            self.queues[phase as usize].push(runnable);
            self.num_queued.fetch_add(1);
            if !self.dispatch_scheduled.get() {
                let _ = self.el.schedule(self.id);
                self.dispatch_scheduled.set(true);
            }
        }

        pub fn push_yield(&self, waker: Waker) {
            self.yields.push(waker);
        }

        pub fn iteration(&self) -> u64 {
            self.iteration.get()
        }
    }
}

mod fd {
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

    impl Drop for AsyncFdData {
        fn drop(&mut self) {
            let _ = self.el.remove(self.id);
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
            }
        }
    }

    impl AsyncFd {
        pub fn raw(&self) -> i32 {
            self.data.fd.raw()
        }

        pub fn eng(&self) -> &Rc<AsyncEngine> {
            &self.engine
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
}
