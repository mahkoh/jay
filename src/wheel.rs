use {
    crate::{
        async_engine::{AsyncEngine, AsyncError, AsyncFd, SpawnedFuture},
        time::{Time, TimeError},
        utils::{
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell, oserror::OsError,
            stack::Stack,
        },
    },
    std::{
        cell::{Cell, RefCell},
        cmp::Reverse,
        collections::BinaryHeap,
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
        time::Duration,
    },
    thiserror::Error,
    uapi::c,
};

#[derive(Debug, Error)]
pub enum WheelError {
    #[error("Could not create the timerfd")]
    CreateFailed(#[source] OsError),
    #[error("Could not set the timerfd")]
    SetFailed(#[source] OsError),
    #[error("An async error occurred")]
    AsyncError(#[from] AsyncError),
    #[error("Cannot determine the time")]
    TimeError(#[from] TimeError),
    #[error("The timer wheel is already destroyed")]
    Destroyed,
    #[error("Could not read from the timerfd")]
    Read(#[source] OsError),
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct WheelEntry {
    expiration: Time,
    id: u64,
}

pub struct Wheel {
    data: Rc<WheelData>,
}

impl Drop for Wheel {
    fn drop(&mut self) {
        self.data.kill();
    }
}

struct WheelTimeoutData {
    id: u64,
    expired: Cell<Option<Result<(), WheelError>>>,
    wheel: Rc<WheelData>,
    waker: Cell<Option<Waker>>,
}

impl WheelTimeoutData {
    fn complete(&self, res: Result<(), WheelError>) {
        self.expired.set(Some(res));
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

pub struct WheelTimeoutFuture {
    data: Rc<WheelTimeoutData>,
}

impl Drop for WheelTimeoutFuture {
    fn drop(&mut self) {
        self.data.wheel.dispatchers.remove(&self.data.id);
        self.data.waker.set(None);
        if !self.data.wheel.destroyed.get() {
            self.data.expired.take();
            self.data.wheel.cached_futures.push(self.data.clone());
        }
    }
}

impl Future for WheelTimeoutFuture {
    type Output = Result<(), WheelError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(res) = self.data.expired.take() {
            Poll::Ready(res)
        } else {
            self.data.waker.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}

pub struct WheelData {
    destroyed: Cell<bool>,
    fd: AsyncFd,
    next_id: NumCell<u64>,
    start: Time,
    current_expiration: Cell<Option<Time>>,
    dispatchers: CopyHashMap<u64, Rc<WheelTimeoutData>>,
    expirations: RefCell<BinaryHeap<Reverse<WheelEntry>>>,
    dispatcher: Cell<Option<SpawnedFuture<()>>>,
    cached_futures: Stack<Rc<WheelTimeoutData>>,
}

impl Wheel {
    pub fn new(eng: &Rc<AsyncEngine>) -> Result<Rc<Self>, WheelError> {
        let fd = match uapi::timerfd_create(c::CLOCK_MONOTONIC, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
            Ok(fd) => Rc::new(fd),
            Err(e) => return Err(WheelError::CreateFailed(e.into())),
        };
        let fd = eng.fd(&fd)?;
        let data = Rc::new(WheelData {
            destroyed: Cell::new(false),
            fd,
            next_id: NumCell::new(1),
            start: Time::now()?,
            current_expiration: Default::default(),
            dispatchers: Default::default(),
            expirations: Default::default(),
            dispatcher: Default::default(),
            cached_futures: Default::default(),
        });
        data.dispatcher
            .set(Some(eng.spawn(data.clone().dispatch())));
        Ok(Rc::new(Wheel { data }))
    }

    fn future(&self) -> WheelTimeoutFuture {
        let data = self.data.cached_futures.pop().unwrap_or_else(|| {
            Rc::new(WheelTimeoutData {
                id: self.data.next_id.fetch_add(1),
                expired: Cell::new(None),
                wheel: self.data.clone(),
                waker: Cell::new(None),
            })
        });
        WheelTimeoutFuture { data }
    }

    pub fn timeout(&self, ms: u64) -> WheelTimeoutFuture {
        if self.data.destroyed.get() {
            return WheelTimeoutFuture {
                data: Rc::new(WheelTimeoutData {
                    id: 0,
                    expired: Cell::new(Some(Err(WheelError::Destroyed))),
                    wheel: self.data.clone(),
                    waker: Default::default(),
                }),
            };
        }
        let future = self.future();
        let now = match Time::now() {
            Ok(n) => n,
            Err(e) => {
                future.data.expired.set(Some(Err(WheelError::TimeError(e))));
                return future;
            }
        };
        let expiration = (now + Duration::from_millis(ms)).round_to_ms();
        let current = self.data.current_expiration.get();
        if current.is_none() || expiration - self.data.start < current.unwrap() - self.data.start {
            log::info!("programming timer {}", self.data.fd.raw());
            let res = uapi::timerfd_settime(
                self.data.fd.raw(),
                c::TFD_TIMER_ABSTIME,
                &c::itimerspec {
                    it_interval: uapi::pod_zeroed(),
                    it_value: expiration.0,
                },
            );
            if let Err(e) = res {
                future
                    .data
                    .expired
                    .set(Some(Err(WheelError::SetFailed(e.into()))));
                return future;
            }
            self.data.current_expiration.set(Some(expiration));
        }
        self.data.expirations.borrow_mut().push(Reverse(WheelEntry {
            expiration,
            id: future.data.id,
        }));
        self.data
            .dispatchers
            .set(future.data.id, future.data.clone());
        future
    }
}

impl WheelData {
    fn kill(&self) {
        self.destroyed.set(true);
        self.dispatcher.set(None);
        self.cached_futures.take();
        for (_, dispatcher) in self.dispatchers.lock().drain() {
            dispatcher.complete(Err(WheelError::Destroyed));
        }
    }

    async fn dispatch(self: Rc<Self>) {
        loop {
            if let Err(e) = self.fd.readable().await {
                log::error!(
                    "Could not wait for the timerfd to become readable: {}",
                    ErrorFmt(e)
                );
                self.kill();
                return;
            }
            if let Err(e) = self.dispatch_once() {
                log::error!("Could not dispatch wheel expirations: {}", ErrorFmt(e));
                self.kill();
                return;
            }
        }
    }

    fn dispatch_once(&self) -> Result<(), WheelError> {
        let mut n = 0u64;
        loop {
            if let Err(e) = uapi::read(self.fd.raw(), &mut n) {
                if e.0 == c::EAGAIN {
                    break;
                }
                return Err(WheelError::Read(e.into()));
            }
        }
        let now = Time::now()?;
        let dist = now - self.start;
        let mut to_dispatch = vec![];
        {
            let mut expirations = self.expirations.borrow_mut();
            while let Some(Reverse(entry)) = expirations.peek() {
                if entry.expiration - self.start > dist {
                    break;
                }
                if let Some(dispatcher) = self.dispatchers.remove(&entry.id) {
                    to_dispatch.push(dispatcher);
                }
                expirations.pop();
            }
            self.current_expiration.set(None);
            while let Some(Reverse(entry)) = expirations.peek() {
                if self.dispatchers.get(&entry.id).is_some() {
                    let res = uapi::timerfd_settime(
                        self.fd.raw(),
                        c::TFD_TIMER_ABSTIME,
                        &c::itimerspec {
                            it_interval: uapi::pod_zeroed(),
                            it_value: entry.expiration.0,
                        },
                    );
                    if let Err(e) = res {
                        return Err(WheelError::SetFailed(e.into()));
                    }
                    self.current_expiration.set(Some(entry.expiration));
                    break;
                }
                expirations.pop();
            }
        }
        for dispatcher in to_dispatch {
            dispatcher.complete(Ok(()));
        }
        Ok(())
    }
}
