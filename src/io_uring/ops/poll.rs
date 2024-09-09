use {
    crate::io_uring::{
        ops::TaskResult,
        pending_result::PendingResult,
        sys::{io_uring_sqe, IORING_OP_POLL_ADD},
        IoUring, IoUringData, IoUringError, IoUringTaskId, Task, TaskResultExt,
    },
    std::rc::Rc,
    uapi::{c, OwnedFd},
};

impl IoUring {
    pub async fn poll(&self, fd: &Rc<OwnedFd>, events: c::c_short) -> TaskResult<c::c_short> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_polls.pop().unwrap_or_default();
            pw.id = id.id;
            pw.fd = fd.raw() as _;
            pw.events = events as _;
            pw.data = Some(Data {
                pr: pr.clone(),
                _fd: fd.clone(),
            });
            self.ring.schedule(pw);
        }
        Ok(pr.await.map(|v| v as c::c_short))
    }

    pub async fn readable(&self, fd: &Rc<OwnedFd>) -> Result<c::c_short, IoUringError> {
        self.poll(fd, c::POLLIN).await.merge()
    }

    #[expect(dead_code)]
    pub async fn writable(&self, fd: &Rc<OwnedFd>) -> Result<c::c_short, IoUringError> {
        self.poll(fd, c::POLLOUT).await.merge()
    }
}

struct Data {
    pr: PendingResult,
    _fd: Rc<OwnedFd>,
}

#[derive(Default)]
pub struct PollTask {
    id: IoUringTaskId,
    events: u16,
    fd: i32,
    data: Option<Data>,
}

unsafe impl Task for PollTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.pr.complete(res);
        }
        ring.cached_polls.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_POLL_ADD;
        sqe.fd = self.fd;
        sqe.u3.poll_events = self.events;
    }
}
