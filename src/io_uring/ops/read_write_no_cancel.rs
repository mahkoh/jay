#[cfg(test)]
mod tests;

use {
    crate::{
        io_uring::{
            pending_result::PendingResult,
            sys::{io_uring_sqe, IORING_OP_READ, IORING_OP_WRITE},
            IoUring, IoUringData, IoUringError, IoUringTaskId, Task, TaskResultExt,
        },
        time::Time,
        utils::on_drop::OnDrop,
    },
    uapi::{c, Fd},
};

impl IoUring {
    #[expect(dead_code)]
    pub async fn read_no_cancel(
        &self,
        fd: Fd,
        offset: usize,
        buf: &mut [u8],
        cancel: impl FnOnce(IoUringTaskId),
    ) -> Result<usize, IoUringError> {
        self.perform_no_cancel(
            fd,
            offset,
            buf.as_mut_ptr(),
            buf.len(),
            None,
            IORING_OP_READ,
            cancel,
        )
        .await
    }

    #[expect(dead_code)]
    pub async fn write_no_cancel(
        &self,
        fd: Fd,
        offset: usize,
        buf: &[u8],
        timeout: Option<Time>,
        cancel: impl FnOnce(IoUringTaskId),
    ) -> Result<usize, IoUringError> {
        self.perform_no_cancel(
            fd,
            offset,
            buf.as_ptr() as _,
            buf.len(),
            timeout,
            IORING_OP_WRITE,
            cancel,
        )
        .await
    }

    async fn perform_no_cancel(
        &self,
        fd: Fd,
        offset: usize,
        buf: *mut u8,
        len: usize,
        timeout: Option<Time>,
        opcode: u8,
        cancel: impl FnOnce(IoUringTaskId),
    ) -> Result<usize, IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self
                .ring
                .cached_read_writes_no_cancel
                .pop()
                .unwrap_or_default();
            pw.opcode = opcode;
            pw.id = id.id;
            pw.has_timeout = timeout.is_some();
            pw.fd = fd.raw();
            pw.offset = offset;
            pw.buf = buf as _;
            pw.len = len;
            pw.data = Some(ReadWriteTaskData { res: pr.clone() });
            self.ring.schedule(pw);
            if let Some(time) = timeout {
                self.schedule_timeout_link(time);
            }
        }
        let panic = OnDrop(|| panic!("Operation cannot be cancelled from userspace"));
        cancel(id.id);
        let res = Ok(pr.await.map(|v| v as usize)).merge();
        panic.forget();
        res
    }
}

struct ReadWriteTaskData {
    res: PendingResult,
}

#[derive(Default)]
pub struct ReadWriteNoCancelTask {
    id: IoUringTaskId,
    has_timeout: bool,
    fd: c::c_int,
    offset: usize,
    buf: usize,
    len: usize,
    data: Option<ReadWriteTaskData>,
    opcode: u8,
}

unsafe impl Task for ReadWriteNoCancelTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.res.complete(res);
        }
        ring.cached_read_writes_no_cancel.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = self.opcode;
        sqe.fd = self.fd as _;
        sqe.u1.off = self.offset as _;
        sqe.u2.addr = self.buf as _;
        sqe.u3.rw_flags = 0;
        sqe.len = self.len as _;
    }

    fn has_timeout(&self) -> bool {
        self.has_timeout
    }
}
