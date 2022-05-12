use {
    crate::io_uring::{
        ops::TaskResult,
        pending_result::PendingResult,
        sys::{io_uring_sqe, IORING_OP_POLL_ADD},
        IoUring, IoUringData, IoUringError, Task, TaskResultExt,
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    uapi::{c, OwnedFd},
};

impl IoUring {
    pub async fn poll(&self, fd: &Rc<OwnedFd>, events: c::c_short) -> TaskResult<c::c_short> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let pw = self.ring.cached_polls.pop().unwrap_or_default();
            pw.id.set(id.id);
            *pw.data.borrow_mut() = Some(Data {
                pr: pr.clone(),
                fd: fd.clone(),
                events: events as _,
            });
            self.ring.schedule(pw);
        }
        Ok(pr.await.map(|v| v as c::c_short))
    }

    pub async fn readable(&self, fd: &Rc<OwnedFd>) -> Result<c::c_short, IoUringError> {
        self.poll(fd, c::POLLIN).await.merge()
    }

    pub async fn writable(&self, fd: &Rc<OwnedFd>) -> Result<c::c_short, IoUringError> {
        self.poll(fd, c::POLLOUT).await.merge()
    }
}

struct Data {
    pr: PendingResult,
    fd: Rc<OwnedFd>,
    events: u16,
}

#[derive(Default)]
pub struct PollTask {
    id: Cell<u64>,
    data: RefCell<Option<Data>>,
}

unsafe impl Task for PollTask {
    fn id(&self) -> u64 {
        self.id.get()
    }

    fn complete(self: Box<Self>, ring: &IoUringData, res: i32) {
        let data = self.data.borrow_mut().take();
        if let Some(data) = data {
            data.pr.complete(res);
        }
        ring.cached_polls.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        let data = self.data.borrow_mut();
        let data = data.as_ref().unwrap();
        sqe.opcode = IORING_OP_POLL_ADD;
        sqe.fd = data.fd.raw();
        sqe.u3.poll_events = data.events;
    }
}
