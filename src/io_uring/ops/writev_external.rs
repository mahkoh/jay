use crate::io_uring::IoUring;
use crate::io_uring::IoUringData;
use crate::io_uring::IoUringError;
use crate::io_uring::IoUringTaskId;
use crate::io_uring::Task;
use crate::io_uring::sys::IORING_OP_WRITEV;
use crate::io_uring::sys::io_uring_cqe;
use crate::io_uring::sys::io_uring_sqe;
use crate::utils::aliasable_box::AliasableBox;
use crate::utils::aliasable_box::AliasableBoxExt;
use crate::utils::oserror::OsError;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;
use uapi::c::iovec;

#[cfg(test)]
mod tests;

pub unsafe trait WritevData {
    fn iovecs(&self) -> &[iovec];
    fn done(self: Box<Self>, res: Result<usize, OsError>);
}

impl IoUring {
    #[cfg_attr(not(test), expect(unused))]
    pub fn writev_external(
        &self,
        fd: &Rc<OwnedFd>,
        data: Box<dyn WritevData>,
    ) -> Result<(), IoUringError> {
        self.ring.check_destroyed()?;
        let data = data.into_aliasable();
        let iovecs = data.iovecs();
        let mut pw = self.ring.cached_writev_external.pop().unwrap_or_default();
        pw.id = self.ring.id_raw();
        pw.fd = fd.raw();
        pw.iovecs = iovecs.as_ptr();
        pw.iovecs_len = iovecs.len();
        pw.data = Some(WritevExternalTaskData {
            _fd: fd.clone(),
            data,
        });
        self.ring.schedule(pw);
        Ok(())
    }
}

struct WritevExternalTaskData {
    _fd: Rc<OwnedFd>,
    data: AliasableBox<dyn WritevData>,
}

#[derive(Default)]
pub struct WritevExternalTask {
    id: IoUringTaskId,
    fd: c::c_int,
    iovecs: *const iovec,
    iovecs_len: usize,
    data: Option<WritevExternalTaskData>,
}

unsafe impl Task for WritevExternalTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, cqe: &io_uring_cqe) {
        if let Some(data) = self.data.take() {
            let res = map_err!(cqe.res).map(|v| v as usize);
            data.data.into_box().done(res);
        }
        ring.cached_writev_external.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_WRITEV;
        sqe.fd = self.fd as _;
        sqe.u1.off = !0;
        sqe.u2.addr = self.iovecs as _;
        sqe.u3.rw_flags = 0;
        sqe.len = self.iovecs_len as _;
    }
}
