use crate::event_loop::{EventLoop, EventLoopDispatcher, EventLoopError, EventLoopId};
use crate::time::{Time, TimeError};
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::numcell::NumCell;
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::rc::Rc;
use std::time::Duration;
use thiserror::Error;
use uapi::{c, OwnedFd};

#[derive(Debug, Error)]
pub enum WheelError {
    #[error("Could not create the timerfd: {0}")]
    CreateFailed(crate::utils::oserror::OsError),
    #[error("Could not set the timerfd: {0}")]
    SetFailed(crate::utils::oserror::OsError),
    #[error("The timerfd is in an error state")]
    ErrorEvent,
    #[error("An event loop error occurred: {0}")]
    EventLoopError(#[from] EventLoopError),
    #[error("Cannot determine the time: {0}")]
    TimeError(#[from] TimeError),
    #[error("The timer wheel is already destroyed")]
    Destroyed,
}

pub trait WheelDispatcher {
    fn dispatch(self: Rc<Self>) -> Result<(), Box<dyn std::error::Error>>;
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct WheelEntry {
    expiration: Time,
    id: WheelId,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WheelId(u64);

pub struct Wheel {
    destroyed: Cell<bool>,
    fd: OwnedFd,
    next_id: NumCell<u64>,
    start: Time,
    current_expiration: Cell<Option<Time>>,
    dispatchers: CopyHashMap<WheelId, Rc<dyn WheelDispatcher>>,
    periodic_dispatchers: CopyHashMap<WheelId, Rc<PeriodicDispatcher>>,
    expirations: RefCell<BinaryHeap<Reverse<WheelEntry>>>,
    id: EventLoopId,
    el: Rc<EventLoop>,
}

impl Wheel {
    pub fn install(el: &Rc<EventLoop>) -> Result<Rc<Self>, WheelError> {
        let fd = match uapi::timerfd_create(c::CLOCK_MONOTONIC, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
            Ok(fd) => fd,
            Err(e) => return Err(WheelError::CreateFailed(e.into())),
        };
        let id = el.id();
        let wheel = Rc::new(Self {
            destroyed: Cell::new(false),
            fd,
            next_id: Default::default(),
            start: Time::now()?,
            current_expiration: Cell::new(None),
            dispatchers: CopyHashMap::new(),
            periodic_dispatchers: Default::default(),
            expirations: RefCell::new(Default::default()),
            id,
            el: el.clone(),
        });
        let wrapper = Rc::new(WheelWrapper {
            wheel: wheel.clone(),
        });
        el.insert(id, Some(wheel.fd.raw()), c::EPOLLIN, wrapper)?;
        Ok(wheel)
    }

    pub fn id(&self) -> WheelId {
        WheelId(self.next_id.fetch_add(1))
    }

    fn check_destroyed(&self) -> Result<(), WheelError> {
        if self.destroyed.get() {
            return Err(WheelError::Destroyed);
        }
        Ok(())
    }

    pub fn timeout(
        &self,
        id: WheelId,
        ms: u64,
        dispatcher: Rc<dyn WheelDispatcher>,
    ) -> Result<(), WheelError> {
        self.check_destroyed()?;
        let expiration = (Time::now()? + Duration::from_millis(ms)).round_to_ms();
        let current = self.current_expiration.get();
        if current.is_none() || expiration - self.start < current.unwrap() - self.start {
            let res = uapi::timerfd_settime(
                self.fd.raw(),
                c::TFD_TIMER_ABSTIME,
                &c::itimerspec {
                    it_interval: uapi::pod_zeroed(),
                    it_value: expiration.0,
                },
            );
            if let Err(e) = res {
                return Err(WheelError::SetFailed(e.into()));
            }
            self.current_expiration.set(Some(expiration));
        }
        self.expirations
            .borrow_mut()
            .push(Reverse(WheelEntry { expiration, id }));
        self.dispatchers.set(id, dispatcher);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn periodic(
        &self,
        id: WheelId,
        us: u64,
        dispatcher: Rc<dyn WheelDispatcher>,
    ) -> Result<(), WheelError> {
        self.check_destroyed()?;
        let fd = match uapi::timerfd_create(c::CLOCK_MONOTONIC, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
            Ok(fd) => fd,
            Err(e) => return Err(WheelError::CreateFailed(e.into())),
        };
        let tv_sec = (us / 1_000_000) as _;
        let tv_nsec = (us % 1_000_000 * 1_000) as _;
        let res = uapi::timerfd_settime(
            fd.raw(),
            0,
            &c::itimerspec {
                it_interval: c::timespec { tv_sec, tv_nsec },
                it_value: c::timespec { tv_sec, tv_nsec },
            },
        );
        if let Err(e) = res {
            return Err(WheelError::SetFailed(e.into()));
        }
        let el_id = self.el.id();
        let pd = Rc::new(PeriodicDispatcher {
            fd,
            id: el_id,
            el: self.el.clone(),
            dispatcher,
        });
        self.el
            .insert(el_id, Some(pd.fd.raw()), c::EPOLLIN, pd.clone())?;
        self.periodic_dispatchers.set(id, pd);
        Ok(())
    }

    pub fn remove(&self, id: WheelId) {
        // log::trace!("removing {:?} from wheel", id);
        self.dispatchers.remove(&id);
        if let Some(d) = self.periodic_dispatchers.remove(&id) {
            let _ = self.el.remove(d.id);
        }
    }
}

struct WheelWrapper {
    wheel: Rc<Wheel>,
}

impl EventLoopDispatcher for WheelWrapper {
    fn dispatch(
        self: Rc<Self>,
        _fd: Option<i32>,
        events: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
            return Err(Box::new(WheelError::ErrorEvent));
        }
        let mut n = 0u64;
        while uapi::read(self.wheel.fd.raw(), &mut n).is_ok() {}
        let now = match Time::now() {
            Ok(n) => n,
            Err(e) => return Err(Box::new(e)),
        };
        let dist = now - self.wheel.start;
        let mut to_dispatch = vec![];
        {
            let mut expirations = self.wheel.expirations.borrow_mut();
            while let Some(Reverse(entry)) = expirations.peek() {
                if entry.expiration - self.wheel.start > dist {
                    break;
                }
                if let Some(dispatcher) = self.wheel.dispatchers.remove(&entry.id) {
                    to_dispatch.push(dispatcher);
                }
                expirations.pop();
            }
            self.wheel.current_expiration.set(None);
            while let Some(Reverse(entry)) = expirations.peek() {
                if self.wheel.dispatchers.get(&entry.id).is_some() {
                    let res = uapi::timerfd_settime(
                        self.wheel.fd.raw(),
                        c::TFD_TIMER_ABSTIME,
                        &c::itimerspec {
                            it_interval: uapi::pod_zeroed(),
                            it_value: entry.expiration.0,
                        },
                    );
                    if let Err(e) = res {
                        return Err(Box::new(WheelError::SetFailed(e.into())));
                    }
                    self.wheel.current_expiration.set(Some(entry.expiration));
                    break;
                }
                expirations.pop();
            }
        }
        for dispatcher in to_dispatch {
            dispatcher.dispatch()?;
        }
        Ok(())
    }
}

impl Drop for WheelWrapper {
    fn drop(&mut self) {
        self.wheel.destroyed.set(true);
        self.wheel.dispatchers.clear();
        let _ = self.wheel.el.remove(self.wheel.id);
    }
}

struct PeriodicDispatcher {
    fd: OwnedFd,
    id: EventLoopId,
    el: Rc<EventLoop>,
    dispatcher: Rc<dyn WheelDispatcher>,
}

impl EventLoopDispatcher for PeriodicDispatcher {
    fn dispatch(self: Rc<Self>, _fd: Option<i32>, events: i32) -> Result<(), Box<dyn Error>> {
        if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
            return Err(Box::new(WheelError::ErrorEvent));
        }
        let mut n = 0u64;
        while uapi::read(self.fd.raw(), &mut n).is_ok() {}
        self.dispatcher.clone().dispatch()
    }
}

impl Drop for PeriodicDispatcher {
    fn drop(&mut self) {
        let _ = self.el.remove(self.id);
    }
}
