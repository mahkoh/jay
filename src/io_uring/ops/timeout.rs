use {
    crate::{
        io_uring::{
            sys::{io_uring_sqe, IORING_OP_LINK_TIMEOUT, IORING_TIMEOUT_ABS},
            IoUring, IoUringData, Task,
        },
        time::Time,
    },
    uapi::c,
};

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Default)]
struct timespec64 {
    tv_sec: i64,
    tv_nsec: c::c_long,
}

#[derive(Default)]
pub struct TimeoutTask {
    id: u64,
    timespec: timespec64,
}

impl IoUring {
    pub(super) fn schedule_timeout(&self, timeout: Time) {
        let id = self.ring.id_raw();
        {
            let mut to = self.ring.cached_timeouts.pop().unwrap_or_default();
            to.id = id;
            to.timespec.tv_sec = timeout.0.tv_sec as _;
            to.timespec.tv_nsec = timeout.0.tv_nsec as _;
            self.ring.schedule(to);
        }
    }
}

unsafe impl Task for TimeoutTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(self: Box<Self>, ring: &IoUringData, _res: i32) {
        ring.cached_timeouts.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_LINK_TIMEOUT;
        sqe.u2.addr = &self.timespec as *const _ as _;
        sqe.len = 1;
        sqe.u3.timeout_flags = IORING_TIMEOUT_ABS;
    }
}
