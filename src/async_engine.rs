pub use crate::async_engine::yield_::Yield;
use crate::event_loop::{EventLoop, EventLoopError};
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::numcell::NumCell;
use crate::wheel::{Wheel, WheelError};
pub use fd::AsyncFd;
use fd::AsyncFdData;
use queue::{DispatchQueue, Dispatcher};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
pub use task::SpawnedFuture;
use thiserror::Error;
pub use timeout::Timeout;
use timeout::TimeoutData;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum AsyncError {
    #[error("The timer wheel returned an error: {0}")]
    WheelError(#[from] WheelError),
    #[error("The event loop caused an error: {0}")]
    EventLoopError(#[from] EventLoopError),
}

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

    pub fn spawn<T, F: Future<Output = T> + 'static>(&self, f: F) -> SpawnedFuture<T> {
        self.queue.spawn(f)
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
                erroneous: Cell::new(false),
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
    use crate::async_engine::queue::DispatchQueue;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll};

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
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

mod timeout {
    use crate::wheel::{Wheel, WheelDispatcher, WheelId};
    use std::cell::{Cell, RefCell};
    use std::error::Error;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    pub(super) struct TimeoutData {
        pub expired: Cell<bool>,
        pub waker: RefCell<Option<Waker>>,
    }

    impl WheelDispatcher for TimeoutData {
        fn dispatch(self: Rc<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
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
    use crate::async_engine::queue::DispatchQueue;
    use std::future::Future;
    use std::mem::ManuallyDrop;
    use std::pin::Pin;
    use std::ptr;
    use std::rc::Rc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

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
            let task = &mut *(data as *mut Task<T, F>);
            if task.state & COMPLETED == 0 {
                task.waker = Some(ctx.waker().clone());
                Poll::Pending
            } else if task.state & EMPTIED == 0 {
                task.state |= EMPTIED;
                Poll::Ready(ptr::read(&mut *task.data.result))
            } else {
                panic!("Future polled after it has already been emptied");
            }
        }

        unsafe fn drop(data: *mut u8) {
            let task = &mut *(data as *mut Task<T, F>);
            task.state |= CANCELLED;
            if task.state & RUNNING == 0 {
                task.drop_data();
            }
            task.dec_ref_count();
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

    const RUNNING: usize = 1;
    const RUN_AGAIN: usize = 2;
    const COMPLETED: usize = 4;
    const EMPTIED: usize = 8;
    const CANCELLED: usize = 16;

    struct Task<T, F: Future<Output = T>> {
        ref_count: u64,
        state: usize,
        data: TaskData<T, F>,
        waker: Option<Waker>,
        queue: Rc<DispatchQueue>,
    }

    pub(super) struct Runnable {
        data: *mut u8,
        run: unsafe fn(data: *mut u8, run: bool),
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
        pub(super) fn spawn<T, F: Future<Output = T>>(self: &Rc<Self>, f: F) -> SpawnedFuture<T> {
            let mut f = Box::new(Task {
                ref_count: 1,
                state: 0,
                data: TaskData {
                    future: ManuallyDrop::new(f),
                },
                waker: None,
                queue: self.clone(),
            });
            unsafe {
                f.schedule_run();
            }
            let f = Box::into_raw(f);
            SpawnedFuture {
                vtable: &SpawnedFutureVTableProxy::<T, F>::VTABLE,
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

        unsafe fn run_proxy(data: *mut u8, run: bool) {
            let task = &mut *(data as *mut Self);
            if run {
                task.run();
            }
            task.dec_ref_count();
        }

        unsafe fn dec_ref_count(&mut self) {
            self.ref_count -= 1;
            if self.ref_count == 0 {
                Box::from_raw(self);
            }
        }

        unsafe fn inc_ref_count(&mut self) {
            self.ref_count += 1;
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
            let task = &mut *(data as *mut Self);
            task.schedule_run();
        }

        unsafe fn waker_drop(data: *const ()) {
            let task = &mut *(data as *mut Self);
            task.dec_ref_count();
        }

        unsafe fn schedule_run(&mut self) {
            if self.state & (COMPLETED | CANCELLED) == 0 {
                if self.state & RUNNING == 0 {
                    self.state |= RUNNING;
                    self.inc_ref_count();
                    let data = self as *mut _ as _;
                    self.queue.push(Runnable {
                        data,
                        run: Self::run_proxy,
                    });
                } else {
                    self.state |= RUN_AGAIN;
                }
            }
        }

        unsafe fn run(&mut self) {
            if self.state & CANCELLED == 0 {
                self.inc_ref_count();
                let raw_waker = RawWaker::new(self as *const _ as _, &Self::VTABLE);
                let waker = Waker::from_raw(raw_waker);

                let mut ctx = Context::from_waker(&waker);
                if let Poll::Ready(d) = Pin::new_unchecked(&mut *self.data.future).poll(&mut ctx) {
                    ManuallyDrop::drop(&mut self.data.future);
                    ptr::write(&mut self.data.result, ManuallyDrop::new(d));
                    self.state |= COMPLETED;
                    if let Some(waker) = self.waker.take() {
                        waker.wake();
                    }
                }
            }

            self.state &= !RUNNING;

            if self.state & CANCELLED != 0 {
                self.drop_data();
            } else if self.state & RUN_AGAIN != 0 {
                self.state &= !RUN_AGAIN;
                self.schedule_run()
            }
        }

        unsafe fn drop_data(&mut self) {
            if self.state & COMPLETED == 0 {
                ManuallyDrop::drop(&mut self.data.future);
            } else if self.state & EMPTIED == 0 {
                ManuallyDrop::drop(&mut self.data.result);
            }
        }
    }
}

mod queue {
    use crate::async_engine::task::Runnable;
    use crate::async_engine::AsyncError;
    use crate::event_loop::{EventLoop, EventLoopDispatcher, EventLoopId};
    use crate::utils::numcell::NumCell;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::error::Error;
    use std::mem;
    use std::rc::Rc;

    pub(super) struct Dispatcher {
        queue: Rc<DispatchQueue>,
        stash: RefCell<VecDeque<Runnable>>,
    }

    impl Dispatcher {
        pub fn install(el: &Rc<EventLoop>) -> Result<Rc<DispatchQueue>, AsyncError> {
            let id = el.id();
            let queue = Rc::new(DispatchQueue {
                id,
                el: el.clone(),
                dispatch_scheduled: Cell::new(false),
                queue: RefCell::new(Default::default()),
                iteration: Default::default(),
            });
            let slf = Rc::new(Dispatcher {
                queue: queue.clone(),
                stash: RefCell::new(Default::default()),
            });
            el.insert(id, None, 0, slf)?;
            Ok(queue)
        }
    }

    impl EventLoopDispatcher for Dispatcher {
        fn dispatch(&self, _events: i32) -> Result<(), Box<dyn Error + Send + Sync>> {
            loop {
                self.queue.iteration.fetch_add(1);
                let mut stash = self.stash.borrow_mut();
                mem::swap(&mut *stash, &mut *self.queue.queue.borrow_mut());
                if stash.is_empty() {
                    break;
                }
                for runnable in stash.drain(..) {
                    runnable.run();
                }
            }
            self.queue.dispatch_scheduled.set(false);
            Ok(())
        }
    }

    impl Drop for Dispatcher {
        fn drop(&mut self) {
            let _ = self.queue.el.remove(self.queue.id);
            mem::take(&mut *self.queue.queue.borrow_mut());
        }
    }

    pub(super) struct DispatchQueue {
        dispatch_scheduled: Cell<bool>,
        id: EventLoopId,
        el: Rc<EventLoop>,
        queue: RefCell<VecDeque<Runnable>>,
        iteration: NumCell<u64>,
    }

    impl DispatchQueue {
        pub fn push(&self, runnable: Runnable) {
            self.queue.borrow_mut().push_back(runnable);
            if !self.dispatch_scheduled.get() {
                let _ = self.el.schedule(self.id);
                self.dispatch_scheduled.set(true);
            }
        }

        pub fn iteration(&self) -> u64 {
            self.iteration.load()
        }
    }
}

mod fd {
    use crate::async_engine::{AsyncEngine, AsyncError};
    use crate::event_loop::{EventLoop, EventLoopDispatcher, EventLoopError, EventLoopId};
    use crate::utils::numcell::NumCell;
    use std::cell::{Cell, RefCell};
    use std::error::Error;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};
    use uapi::{c, OwnedFd};

    type Queue = RefCell<Vec<(Waker, Rc<Cell<bool>>)>>;

    pub(super) struct AsyncFdData {
        pub(super) ref_count: NumCell<u64>,
        pub(super) fd: Rc<OwnedFd>,
        pub(super) id: EventLoopId,
        pub(super) el: Rc<EventLoop>,
        pub(super) write_registered: Cell<bool>,
        pub(super) read_registered: Cell<bool>,
        pub(super) readers: Queue,
        pub(super) writers: Queue,
        pub(super) erroneous: Cell<bool>,
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
                self.erroneous.set(true);
                let _ = self.el.remove(self.id);
            }
            res
        }

        fn poll(
            &self,
            woken: &Rc<Cell<bool>>,
            cx: &mut Context<'_>,
            registered: impl Fn(&AsyncFdData) -> &Cell<bool>,
            queue: impl Fn(&AsyncFdData) -> &Queue,
        ) -> Poll<Result<(), AsyncError>> {
            if woken.get() || self.erroneous.get() {
                return Poll::Ready(Ok(()));
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
        fn dispatch(&self, events: i32) -> Result<(), Box<dyn Error + Send + Sync>> {
            if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
                self.erroneous.set(true);
                if let Err(e) = self.el.remove(self.id) {
                    return Err(Box::new(e));
                }
            }
            let mut woke_any = false;
            if events & c::EPOLLIN != 0 || self.erroneous.get() {
                let mut readers = self.readers.borrow_mut();
                woke_any |= !readers.is_empty();
                for (waker, woken) in readers.drain(..) {
                    woken.set(true);
                    waker.wake();
                }
            }
            if events & c::EPOLLOUT != 0 || self.erroneous.get() {
                let mut writers = self.writers.borrow_mut();
                woke_any |= !writers.is_empty();
                for (waker, woken) in writers.drain(..) {
                    woken.set(true);
                    waker.wake();
                }
            }
            if !woke_any && !self.erroneous.get() {
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
                woken: Rc::new(Cell::new(false)),
            }
        }

        pub fn writable(&self) -> AsyncFdWritable {
            AsyncFdWritable {
                fd: self,
                woken: Rc::new(Cell::new(false)),
            }
        }
    }

    pub struct AsyncFdReadable<'a> {
        fd: &'a AsyncFd,
        woken: Rc<Cell<bool>>,
    }

    impl<'a> Future for AsyncFdReadable<'a> {
        type Output = Result<(), AsyncError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let data = &self.fd.data;
            data.poll(&self.woken, cx, |d| &d.read_registered, |d| &d.readers)
        }
    }

    pub struct AsyncFdWritable<'a> {
        fd: &'a AsyncFd,
        woken: Rc<Cell<bool>>,
    }

    impl<'a> Future for AsyncFdWritable<'a> {
        type Output = Result<(), AsyncError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let data = &self.fd.data;
            data.poll(&self.woken, cx, |d| &d.write_registered, |d| &d.writers)
        }
    }
}
