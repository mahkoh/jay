use {
    crate::{
        io_uring::{
            sys::{io_uring_sqe, IORING_OP_ASYNC_CANCEL},
            IoUringData, IoUringTaskId, Task,
        },
        utils::errorfmt::ErrorFmt,
    },
    uapi::c,
};

#[derive(Default)]
pub struct AsyncCancelTask {
    id: IoUringTaskId,
    target: IoUringTaskId,
}

impl IoUringData {
    pub fn cancel_task_in_kernel(&self, target: IoUringTaskId) {
        let id = self.id_raw();
        let mut task = self.cached_cancels.pop().unwrap_or_default();
        task.id = id;
        task.target = target;
        self.schedule(task);
    }
}

unsafe impl Task for AsyncCancelTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Err(e) = map_err!(res) {
            if e.0 != c::ENOENT {
                log::debug!("Could not cancel task: {}", ErrorFmt(e));
            }
        }
        ring.cached_cancels.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_ASYNC_CANCEL;
        sqe.u2.addr = self.target.raw();
    }

    fn is_cancel(&self) -> bool {
        true
    }
}
