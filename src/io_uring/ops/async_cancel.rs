use {
    crate::{
        io_uring::{
            sys::{io_uring_sqe, IORING_OP_ASYNC_CANCEL},
            IoUringData, Task,
        },
        utils::errorfmt::ErrorFmt,
    },
    std::cell::Cell,
    uapi::c,
};

pub struct AsyncCancelTask {
    id: Cell<u64>,
    target: Cell<u64>,
}

impl IoUringData {
    pub fn cancel_task_in_kernel(&self, target: u64) {
        let task = self.cached_cancels.pop().unwrap_or_else(|| {
            Box::new(AsyncCancelTask {
                id: Cell::new(0),
                target: Cell::new(0),
            })
        });
        let id = self.id_raw();
        task.id.set(id);
        task.target.set(target);
        self.schedule(task);
    }
}

unsafe impl Task for AsyncCancelTask {
    fn id(&self) -> u64 {
        self.id.get()
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
        sqe.u2.addr = self.target.get();
    }

    fn is_cancel(&self) -> bool {
        true
    }
}
