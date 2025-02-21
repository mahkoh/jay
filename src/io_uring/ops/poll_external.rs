use {
    crate::{
        io_uring::{
            IoUring, IoUringData, IoUringError, IoUringTaskId, Task,
            sys::{IORING_OP_POLL_ADD, io_uring_sqe},
        },
        utils::oserror::OsError,
    },
    std::{cell::Cell, rc::Rc},
    uapi::{OwnedFd, c},
};

pub trait PollCallback {
    fn completed(self: Rc<Self>, res: Result<c::c_short, OsError>);
}

pub struct PendingPoll {
    data: Rc<IoUringData>,
    shared: Rc<PollExternalTaskShared>,
    id: IoUringTaskId,
}

impl Drop for PendingPoll {
    fn drop(&mut self) {
        if self.shared.id.get() != self.id {
            return;
        }
        self.shared.callback.take();
        self.data.cancel_task(self.id);
    }
}

impl IoUring {
    pub fn poll_external(
        &self,
        fd: &Rc<OwnedFd>,
        events: c::c_short,
        callback: Rc<dyn PollCallback>,
    ) -> Result<PendingPoll, IoUringError> {
        self.ring.check_destroyed()?;
        let mut pw = self.ring.cached_polls_external.pop().unwrap_or_default();
        pw.shared.id.set(self.ring.id_raw());
        pw.shared.callback.set(Some(callback));
        pw.fd = fd.raw() as _;
        pw.events = events as _;
        pw.data = Some(Data { _fd: fd.clone() });
        let pending = PendingPoll {
            data: self.ring.clone(),
            shared: pw.shared.clone(),
            id: pw.shared.id.get(),
        };
        self.ring.schedule(pw);
        Ok(pending)
    }

    pub fn readable_external(
        &self,
        fd: &Rc<OwnedFd>,
        callback: Rc<dyn PollCallback>,
    ) -> Result<PendingPoll, IoUringError> {
        self.poll_external(fd, c::POLLIN, callback)
    }

    #[expect(dead_code)]
    pub fn writable_external(
        &self,
        fd: &Rc<OwnedFd>,
        callback: Rc<dyn PollCallback>,
    ) -> Result<PendingPoll, IoUringError> {
        self.poll_external(fd, c::POLLOUT, callback)
    }
}

struct Data {
    _fd: Rc<OwnedFd>,
}

#[derive(Default)]
struct PollExternalTaskShared {
    id: Cell<IoUringTaskId>,
    callback: Cell<Option<Rc<dyn PollCallback>>>,
}

#[derive(Default)]
pub struct PollExternalTask {
    shared: Rc<PollExternalTaskShared>,
    events: u16,
    fd: i32,
    data: Option<Data>,
}

unsafe impl Task for PollExternalTask {
    fn id(&self) -> IoUringTaskId {
        self.shared.id.get()
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        self.data.take();
        self.shared.id.set(Default::default());
        if let Some(cb) = self.shared.callback.take() {
            let res = if res < 0 {
                Err(OsError::from(-res as c::c_int))
            } else {
                Ok(res as _)
            };
            cb.completed(res)
        }
        ring.cached_polls_external.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_POLL_ADD;
        sqe.fd = self.fd;
        sqe.u3.poll_events = self.events;
    }
}
