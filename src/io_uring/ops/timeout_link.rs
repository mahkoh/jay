use crate::{
    io_uring::{
        ops::timeout::timespec64,
        sys::{io_uring_sqe, IORING_OP_LINK_TIMEOUT, IORING_TIMEOUT_ABS},
        IoUring, IoUringData, IoUringTaskId, Task,
    },
    time::Time,
};

#[derive(Default)]
pub struct TimeoutLinkTask {
    id: IoUringTaskId,
    timespec: timespec64,
}

impl IoUring {
    pub(super) fn schedule_timeout_link(&self, timeout: Time) {
        let id = self.ring.id_raw();
        {
            let mut to = self.ring.cached_timeout_links.pop().unwrap_or_default();
            to.id = id;
            to.timespec.tv_sec = timeout.0.tv_sec as _;
            to.timespec.tv_nsec = timeout.0.tv_nsec as _;
            self.ring.schedule(to);
        }
    }
}

unsafe impl Task for TimeoutLinkTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(self: Box<Self>, ring: &IoUringData, _res: i32) {
        ring.cached_timeout_links.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_LINK_TIMEOUT;
        sqe.u2.addr = &self.timespec as *const _ as _;
        sqe.len = 1;
        sqe.u3.timeout_flags = IORING_TIMEOUT_ABS;
    }
}
