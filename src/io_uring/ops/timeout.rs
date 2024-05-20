use {
    crate::io_uring::{
        pending_result::PendingResult,
        sys::{io_uring_sqe, IORING_OP_TIMEOUT, IORING_TIMEOUT_ABS},
        IoUring, IoUringData, IoUringError, Task,
    },
    uapi::c,
};

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Default)]
pub(super) struct timespec64 {
    pub tv_sec: i64,
    pub tv_nsec: c::c_long,
}

#[derive(Default)]
pub struct TimeoutTask {
    id: u64,
    timespec: timespec64,
    pr: Option<PendingResult>,
}

impl IoUring {
    pub async fn timeout(&self, timeout_nsec: u64) -> Result<(), IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_timeouts.pop().unwrap_or_default();
            pw.id = id.id;
            pw.timespec = timespec64 {
                tv_sec: (timeout_nsec / 1_000_000_000) as _,
                tv_nsec: (timeout_nsec % 1_000_000_000) as _,
            };
            pw.pr = Some(pr.clone());
            self.ring.schedule(pw);
        }
        let _ = pr.await;
        Ok(())
    }
}

unsafe impl Task for TimeoutTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(pr) = self.pr.take() {
            pr.complete(res);
        }
        ring.cached_timeouts.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_TIMEOUT;
        sqe.u2.addr = &self.timespec as *const _ as _;
        sqe.len = 1;
        sqe.u3.timeout_flags = IORING_TIMEOUT_ABS;
        sqe.u1.off = 0;
    }
}
