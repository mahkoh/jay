use {
    crate::{
        io_uring::{
            ops::TaskResult,
            pending_result::PendingResult,
            sys::{io_uring_sqe, IORING_OP_WRITE},
            IoUring, IoUringData, Task,
        },
        time::Time,
        utils::buf::Buf,
    },
    std::rc::Rc,
    uapi::{c, OwnedFd},
};

impl IoUring {
    pub async fn write(
        &self,
        fd: &Rc<OwnedFd>,
        buf: Buf,
        timeout: Option<Time>,
    ) -> TaskResult<usize> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_writes.pop().unwrap_or_default();
            pw.id = id.id;
            pw.has_timeout = timeout.is_some();
            pw.fd = fd.raw();
            pw.buf = buf.as_ptr() as _;
            pw.len = buf.len();
            pw.data = Some(WriteTaskData {
                _fd: fd.clone(),
                _buf: buf,
                res: pr.clone(),
            });
            self.ring.schedule(pw);
            if let Some(time) = timeout {
                self.schedule_timeout(time);
            }
        }
        Ok(pr.await.map(|v| v as usize))
    }
}

struct WriteTaskData {
    _fd: Rc<OwnedFd>,
    _buf: Buf,
    res: PendingResult,
}

#[derive(Default)]
pub struct WriteTask {
    id: u64,
    has_timeout: bool,
    fd: c::c_int,
    buf: usize,
    len: usize,
    data: Option<WriteTaskData>,
}

unsafe impl Task for WriteTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.res.complete(res);
        }
        ring.cached_writes.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_WRITE;
        sqe.fd = self.fd as _;
        sqe.u1.off = !0;
        sqe.u2.addr = self.buf as _;
        sqe.u3.rw_flags = 0;
        sqe.len = self.len as _;
    }

    fn has_timeout(&self) -> bool {
        self.has_timeout
    }
}
