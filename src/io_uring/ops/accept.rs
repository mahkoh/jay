use crate::io_uring::IoUring;
use crate::io_uring::IoUringData;
use crate::io_uring::IoUringError;
use crate::io_uring::IoUringTaskId;
use crate::io_uring::Task;
use crate::io_uring::TaskResultExt;
use crate::io_uring::pending_result::PendingResult;
use crate::io_uring::sys::IORING_OP_ACCEPT;
use crate::io_uring::sys::io_uring_sqe;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

impl IoUring {
    pub async fn accept(
        &self,
        fd: &Rc<OwnedFd>,
        flags: c::c_int,
    ) -> Result<Rc<OwnedFd>, IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_accepts.pop().unwrap_or_default();
            pw.id = id.id;
            pw.fd = fd.raw() as _;
            pw.flags = flags as _;
            pw.data = Some(Data {
                pr: pr.clone(),
                _fd: fd.clone(),
            });
            self.ring.schedule(pw);
        }
        Ok(pr.await.map(OwnedFd::new).map(Rc::new)).merge()
    }
}

struct Data {
    pr: PendingResult,
    _fd: Rc<OwnedFd>,
}

#[derive(Default)]
pub struct AcceptTask {
    id: IoUringTaskId,
    fd: i32,
    flags: u32,
    data: Option<Data>,
}

unsafe impl Task for AcceptTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.pr.complete(res);
        }
        ring.cached_accepts.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_ACCEPT;
        sqe.fd = self.fd;
        sqe.u2.addr = 0;
        sqe.u1.addr2 = 0;
        sqe.u3.accept_flags = self.flags;
    }
}
