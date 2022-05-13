use {
    crate::{
        io_uring::{
            ops::TaskResult,
            pending_result::PendingResult,
            sys::{io_uring_sqe, IORING_OP_WRITE},
            IoUring, IoUringData, Task,
        },
        utils::buf::Buf,
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    uapi::OwnedFd,
};

impl IoUring {
    pub async fn write(&self, fd: &Rc<OwnedFd>, buf: Buf) -> TaskResult<usize> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let pw = self.ring.cached_writes.pop().unwrap_or_else(|| {
                Box::new(WriteTask {
                    id: Cell::new(0),
                    data: Default::default(),
                })
            });
            pw.id.set(id.id);
            *pw.data.borrow_mut() = Some(WriteTaskData {
                fd: fd.clone(),
                buf,
                res: pr.clone(),
            });
            self.ring.schedule(pw);
        }
        Ok(pr.await.map(|v| v as usize))
    }
}

struct WriteTaskData {
    fd: Rc<OwnedFd>,
    buf: Buf,
    res: PendingResult,
}

pub struct WriteTask {
    id: Cell<u64>,
    data: RefCell<Option<WriteTaskData>>,
}

unsafe impl Task for WriteTask {
    fn id(&self) -> u64 {
        self.id.get()
    }

    fn complete(self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.borrow_mut().take() {
            data.res.complete(res);
        }
        ring.clone().cached_writes.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        let data = self.data.borrow_mut();
        let data = data.as_ref().unwrap();
        sqe.opcode = IORING_OP_WRITE;
        sqe.fd = data.fd.raw();
        sqe.u1.off = !0;
        sqe.u2.addr = data.buf.as_ptr() as _;
        sqe.u3.rw_flags = 0;
        sqe.len = data.buf.len() as _;
    }
}
