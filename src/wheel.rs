use crate::event_loop::{EventLoopDispatcher, EventLoopError, EventLoopId, EventLoopRef};
use crate::time::{Time, TimeError};
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::numcell::NumCell;
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::rc::{Rc, Weak};
use std::time::Duration;
use thiserror::Error;
use uapi::{c, OwnedFd};

#[derive(Debug, Error)]
pub enum WheelError {
    #[error("Could not create the timerfd: {0}")]
    CreateFailed(std::io::Error),
    #[error("Could not set the timerfd: {0}")]
    SetFailed(std::io::Error),
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
    fn dispatch(self: Rc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone)]
pub struct WheelRef {
    data: Weak<WheelData>,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct WheelEntry {
    expiration: Time,
    id: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct WheelId(u64);

struct WheelData {
    id: EventLoopId,
    fd: OwnedFd,
    el: EventLoopRef,
    next_id: NumCell<u64>,
    start: Time,
    current_expiration: Cell<Option<Time>>,
    dispatchers: CopyHashMap<u64, Rc<dyn WheelDispatcher>>,
    expirations: RefCell<BinaryHeap<Reverse<WheelEntry>>>,
}

impl WheelData {
    fn new(el: &EventLoopRef) -> Result<Rc<Self>, WheelError> {
        let fd = match uapi::timerfd_create(c::CLOCK_MONOTONIC, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
            Ok(fd) => fd,
            Err(e) => return Err(WheelError::CreateFailed(e.into())),
        };
        let id = el.id()?;
        let wheel = Rc::new(Self {
            id,
            fd,
            el: el.clone(),
            next_id: Default::default(),
            start: Time::now()?,
            current_expiration: Cell::new(None),
            dispatchers: CopyHashMap::new(),
            expirations: RefCell::new(Default::default()),
        });
        el.insert(id, Some(wheel.fd.raw()), c::EPOLLIN, wheel.clone())?;
        Ok(wheel)
    }

    fn id(&self) -> WheelId {
        WheelId(self.next_id.fetch_add(1))
    }

    fn timeout(
        &self,
        id: WheelId,
        ms: u64,
        dispatcher: Rc<dyn WheelDispatcher>,
    ) -> Result<(), WheelError> {
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
        self.expirations.borrow_mut().push(Reverse(WheelEntry {
            expiration,
            id: id.0,
        }));
        self.dispatchers.set(id.0, dispatcher);
        Ok(())
    }

    fn remove(&self, id: WheelId) {
        log::trace!("removing {:?} from wheel", id);
        self.dispatchers.remove(&id.0);
    }
}

impl EventLoopDispatcher for WheelData {
    fn dispatch(&self, events: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
            return Err(Box::new(WheelError::ErrorEvent));
        }
        let mut n = 0u64;
        while uapi::read(self.fd.raw(), &mut n).is_ok() {}
        let now = match Time::now() {
            Ok(n) => n,
            Err(e) => return Err(Box::new(e)),
        };
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
                        return Err(Box::new(WheelError::SetFailed(e.into())));
                    }
                    self.current_expiration.set(Some(entry.expiration));
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

impl Drop for WheelData {
    fn drop(&mut self) {
        let _ = self.el.remove(self.id);
    }
}

impl WheelRef {
    pub fn new(el: &EventLoopRef) -> Result<Self, WheelError> {
        Ok(Self {
            data: Rc::downgrade(&WheelData::new(el)?),
        })
    }

    pub fn id(&self) -> Result<WheelId, WheelError> {
        match self.data.upgrade() {
            Some(d) => Ok(d.id()),
            _ => Err(WheelError::Destroyed),
        }
    }

    pub fn timeout(
        &self,
        id: WheelId,
        ms: u64,
        dispatcher: Rc<dyn WheelDispatcher>,
    ) -> Result<(), WheelError> {
        match self.data.upgrade() {
            Some(d) => d.timeout(id, ms, dispatcher),
            _ => Err(WheelError::Destroyed),
        }
    }

    pub fn remove(&self, id: WheelId) {
        if let Some(wheel) = self.data.upgrade() {
            wheel.remove(id);
        }
    }
}
