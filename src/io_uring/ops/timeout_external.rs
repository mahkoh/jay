use {
    crate::{
        io_uring::{
            ops::timeout::timespec64,
            sys::{io_uring_sqe, IORING_OP_TIMEOUT, IORING_TIMEOUT_ABS},
            IoUring, IoUringData, IoUringError, IoUringTaskId, Task,
        },
        utils::oserror::OsError,
    },
    std::{cell::Cell, rc::Rc},
    uapi::c,
};

pub trait TimeoutCallback {
    fn completed(self: Rc<Self>, res: Result<(), OsError>, data: u64);
}

pub struct PendingTimeout {
    data: Rc<IoUringData>,
    shared: Rc<TimeoutExternalTaskShared>,
    id: IoUringTaskId,
}

impl Drop for PendingTimeout {
    fn drop(&mut self) {
        if self.shared.id.get() != self.id {
            return;
        }
        self.shared.callback.take();
        self.data.cancel_task(self.id);
    }
}

#[derive(Default)]
struct TimeoutExternalTaskShared {
    id: Cell<IoUringTaskId>,
    callback: Cell<Option<Rc<dyn TimeoutCallback>>>,
}

#[derive(Default)]
pub struct TimeoutExternalTask {
    timespec: timespec64,
    shared: Rc<TimeoutExternalTaskShared>,
    data: u64,
}

impl IoUring {
    pub fn timeout_external(
        &self,
        timeout_nsec: u64,
        callback: Rc<dyn TimeoutCallback>,
        data: u64,
    ) -> Result<PendingTimeout, IoUringError> {
        self.ring.check_destroyed()?;
        let mut pw = self.ring.cached_timeouts_external.pop().unwrap_or_default();
        pw.shared.id.set(self.ring.id_raw());
        pw.shared.callback.set(Some(callback));
        pw.timespec = timespec64 {
            tv_sec: (timeout_nsec / 1_000_000_000) as _,
            tv_nsec: (timeout_nsec % 1_000_000_000) as _,
        };
        pw.data = data;
        let pending = PendingTimeout {
            data: self.ring.clone(),
            shared: pw.shared.clone(),
            id: pw.shared.id.get(),
        };
        self.ring.schedule(pw);
        Ok(pending)
    }
}

unsafe impl Task for TimeoutExternalTask {
    fn id(&self) -> IoUringTaskId {
        self.shared.id.get()
    }

    fn complete(self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(pr) = self.shared.callback.take() {
            let res = if res == -c::ETIME {
                Ok(())
            } else {
                map_err!(res).map(drop)
            };
            pr.completed(res, self.data);
        }
        ring.cached_timeouts_external.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_TIMEOUT;
        sqe.u2.addr = &self.timespec as *const _ as _;
        sqe.len = 1;
        sqe.u3.timeout_flags = IORING_TIMEOUT_ABS;
        sqe.u1.off = 0;
    }
}
