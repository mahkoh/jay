use {
    crate::{
        io_uring::{
            pending_result::PendingResult,
            sys::{io_uring_sqe, IORING_OP_READ, IORING_OP_WRITE},
            IoUring, IoUringData, IoUringError, Task, TaskResultExt,
        },
        time::Time,
        utils::buf::Buf,
    },
    std::rc::Rc,
    uapi::{c, OwnedFd},
};

impl IoUring {
    pub async fn read(&self, fd: &Rc<OwnedFd>, buf: Buf) -> Result<usize, IoUringError> {
        self.perform(fd, buf, None, IORING_OP_READ).await
    }

    pub async fn write(
        &self,
        fd: &Rc<OwnedFd>,
        buf: Buf,
        timeout: Option<Time>,
    ) -> Result<usize, IoUringError> {
        self.perform(fd, buf, timeout, IORING_OP_WRITE).await
    }

    async fn perform(
        &self,
        fd: &Rc<OwnedFd>,
        buf: Buf,
        timeout: Option<Time>,
        opcode: u8,
    ) -> Result<usize, IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_read_writes.pop().unwrap_or_default();
            pw.opcode = opcode;
            pw.id = id.id;
            pw.has_timeout = timeout.is_some();
            pw.fd = fd.raw();
            pw.buf = buf.as_ptr() as _;
            pw.len = buf.len();
            pw.data = Some(ReadWriteTaskData {
                _fd: fd.clone(),
                _buf: buf,
                res: pr.clone(),
            });
            self.ring.schedule(pw);
            if let Some(time) = timeout {
                self.schedule_timeout(time);
            }
        }
        Ok(pr.await.map(|v| v as usize)).merge()
    }
}

struct ReadWriteTaskData {
    _fd: Rc<OwnedFd>,
    _buf: Buf,
    res: PendingResult,
}

#[derive(Default)]
pub struct ReadWriteTask {
    id: u64,
    has_timeout: bool,
    fd: c::c_int,
    buf: usize,
    len: usize,
    data: Option<ReadWriteTaskData>,
    opcode: u8,
}

unsafe impl Task for ReadWriteTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.res.complete(res);
        }
        ring.cached_read_writes.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = self.opcode;
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
