use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::numcell::NumCell;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use thiserror::Error;
use uapi::{c, Errno, OwnedFd};

#[derive(Debug, Error)]
pub enum EventLoopError {
    #[error("Could not create an epoll fd: {0}")]
    CreateFailed(std::io::Error),
    #[error("epoll_wait failed: {0}")]
    WaitFailed(std::io::Error),
    #[error("A dispatcher returned a fatal error: {0}")]
    DispatcherError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Could not insert an fd to wait on: {0}")]
    InsertFailed(std::io::Error),
    #[error("Could not modify an fd to wait on: {0}")]
    ModifyFailed(std::io::Error),
    #[error("Could not remove an fd to wait on: {0}")]
    RemoveFailed(std::io::Error),
    #[error("Entry is not registered")]
    NoEntry,
    #[error("Event loop is already destroyed")]
    Destroyed,
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct EventLoopId(u64);

pub trait EventLoopDispatcher {
    fn dispatch(&self, events: i32) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone)]
struct Entry {
    fd: Option<i32>,
    dispatcher: Rc<dyn EventLoopDispatcher>,
}

struct EventLoopData {
    epoll: OwnedFd,
    run: Cell<bool>,
    next_id: NumCell<u64>,
    entries: CopyHashMap<u64, Entry>,
    scheduled: RefCell<VecDeque<u64>>,
}

pub struct EventLoop {
    data: Rc<EventLoopData>,
}

#[derive(Clone)]
pub struct EventLoopRef {
    data: Weak<EventLoopData>,
}

impl EventLoopData {
    fn new() -> Result<Self, EventLoopError> {
        let epoll = match uapi::epoll_create1(c::EPOLL_CLOEXEC) {
            Ok(e) => e,
            Err(e) => return Err(EventLoopError::CreateFailed(e.into())),
        };
        Ok(Self {
            epoll,
            run: Cell::new(true),
            next_id: NumCell::new(1),
            entries: CopyHashMap::new(),
            scheduled: RefCell::new(Default::default()),
        })
    }

    fn id(&self) -> EventLoopId {
        EventLoopId(self.next_id.fetch_add(1))
    }

    fn stop(&self) {
        self.run.set(false);
    }

    fn insert(
        &self,
        id: EventLoopId,
        fd: Option<i32>,
        events: i32,
        dispatcher: Rc<dyn EventLoopDispatcher>,
    ) -> Result<(), EventLoopError> {
        let id = id.0;
        if let Some(fd) = fd {
            let event = c::epoll_event {
                events: events as _,
                u64: id,
            };
            if let Err(e) = uapi::epoll_ctl(self.epoll.raw(), c::EPOLL_CTL_ADD, fd, Some(&event)) {
                return Err(EventLoopError::InsertFailed(e.into()));
            }
        }
        self.entries.set(id, Entry { fd, dispatcher });
        Ok(())
    }

    fn modify(&self, id: EventLoopId, events: i32) -> Result<(), EventLoopError> {
        let id = id.0;
        let entry = match self.entries.get(&id) {
            Some(e) => e,
            None => return Err(EventLoopError::NoEntry),
        };
        if let Some(fd) = entry.fd {
            let event = c::epoll_event {
                events: events as _,
                u64: id,
            };
            if let Err(e) = uapi::epoll_ctl(self.epoll.raw(), c::EPOLL_CTL_MOD, fd, Some(&event)) {
                return Err(EventLoopError::ModifyFailed(e.into()));
            }
        }
        Ok(())
    }

    fn remove(&self, id: EventLoopId) -> Result<(), EventLoopError> {
        let id = id.0;
        let entry = match self.entries.remove(&id) {
            Some(e) => e,
            None => return Err(EventLoopError::NoEntry),
        };
        if let Some(fd) = entry.fd {
            if let Err(e) = uapi::epoll_ctl(self.epoll.raw(), c::EPOLL_CTL_DEL, fd, None) {
                return Err(EventLoopError::RemoveFailed(e.into()));
            }
        }
        Ok(())
    }

    fn schedule(&self, id: EventLoopId) {
        self.scheduled.borrow_mut().push_back(id.0);
    }

    fn run(&self) -> Result<(), EventLoopError> {
        let mut buf = [c::epoll_event { events: 0, u64: 0 }; 16];
        while self.run.get() {
            while let Some(id) = self.scheduled.borrow_mut().pop_front() {
                if !self.run.get() {
                    break;
                }
                if let Some(entry) = self.entries.get(&id) {
                    if let Err(e) = entry.dispatcher.dispatch(0) {
                        return Err(EventLoopError::DispatcherError(e));
                    }
                }
            }
            let num = match uapi::epoll_wait(self.epoll.raw(), &mut buf, -1) {
                Ok(n) => n,
                Err(Errno(c::EINTR)) => continue,
                Err(e) => return Err(EventLoopError::WaitFailed(e.into())),
            };
            for event in &buf[..num] {
                if !self.run.get() {
                    break;
                }
                let id = event.u64;
                let entry = match self.entries.get(&id) {
                    Some(d) => d,
                    None => {
                        log::warn!(
                            "Client {} created an event but has already been removed",
                            id,
                        );
                        continue;
                    }
                };
                if let Err(e) = entry.dispatcher.dispatch(event.events as i32) {
                    return Err(EventLoopError::DispatcherError(e));
                }
            }
        }
        Ok(())
    }
}

impl EventLoop {
    pub fn new() -> Result<Self, EventLoopError> {
        Ok(Self {
            data: Rc::new(EventLoopData::new()?),
        })
    }

    pub fn to_ref(&self) -> EventLoopRef {
        EventLoopRef {
            data: Rc::downgrade(&self.data),
        }
    }

    pub fn run(&self) -> Result<(), EventLoopError> {
        self.data.run()
    }
}

impl EventLoopRef {
    pub fn id(&self) -> Result<EventLoopId, EventLoopError> {
        match self.data.upgrade() {
            Some(d) => Ok(d.id()),
            None => Err(EventLoopError::Destroyed),
        }
    }

    pub fn stop(&self) {
        if let Some(d) = self.data.upgrade() {
            d.stop();
        }
    }

    pub fn insert(
        &self,
        id: EventLoopId,
        fd: Option<i32>,
        events: i32,
        dispatcher: Rc<dyn EventLoopDispatcher>,
    ) -> Result<(), EventLoopError> {
        match self.data.upgrade() {
            Some(d) => d.insert(id, fd, events, dispatcher),
            None => Err(EventLoopError::Destroyed),
        }
    }

    pub fn modify(&self, id: EventLoopId, events: i32) -> Result<(), EventLoopError> {
        match self.data.upgrade() {
            Some(d) => d.modify(id, events),
            None => Err(EventLoopError::Destroyed),
        }
    }

    pub fn remove(&self, id: EventLoopId) -> Result<(), EventLoopError> {
        match self.data.upgrade() {
            Some(d) => d.remove(id),
            None => Err(EventLoopError::Destroyed),
        }
    }

    pub fn schedule(&self, id: EventLoopId) -> Result<(), EventLoopError> {
        match self.data.upgrade() {
            Some(d) => Ok(d.schedule(id)),
            None => Err(EventLoopError::Destroyed),
        }
    }
}
