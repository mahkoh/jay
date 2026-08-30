use crate::io_uring::IoUring;
use crate::io_uring::IoUringData;
use crate::io_uring::IoUringError;
use crate::io_uring::IoUringTaskId;
use crate::io_uring::MultishotTask;
use crate::io_uring::Task;
use crate::io_uring::buffer_ring::BufferRing;
use crate::io_uring::buffer_ring::BufferRingBuffer;
use crate::io_uring::sys::IORING_OP_READ_MULTISHOT;
use crate::io_uring::sys::IOSQE_BUFFER_SELECT;
use crate::io_uring::sys::io_uring_cqe;
use crate::io_uring::sys::io_uring_sqe;
use crate::utils::clonecell::CloneCell;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use derivative::Derivative;
use jay_algorithms::oserror::OsError;
use std::cell::Cell;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

#[cfg(test)]
mod tests;

pub trait ReadMultishotCallback {
    fn read(self: Rc<Self>, res: BufferRingBuffer);
    fn done(self: Rc<Self>, res: Result<Option<BufferRingBuffer>, OsError>);
}

pub struct PendingReadMultishot {
    ring: Rc<IoUringData>,
    shared: Rc<ReadMultishotTaskShared>,
    id: IoUringTaskId,
}

impl Drop for PendingReadMultishot {
    fn drop(&mut self) {
        if self.shared.id.get() != self.id {
            return;
        }
        self.shared.cb.take();
        self.ring.cancel_task(self.id);
    }
}

impl IoUring {
    pub fn read_multishot(
        &self,
        fd: &Rc<OwnedFd>,
        buffer_ring: &Rc<BufferRing>,
        callback: Rc<dyn ReadMultishotCallback>,
    ) -> Result<PendingReadMultishot, IoUringError> {
        self.ring.check_destroyed()?;
        buffer_ring.check_ring(self)?;
        let id = self.ring.id_raw();
        let mut pw = self.ring.cached_read_multishot.pop().unwrap_or_default();
        pw.bgid = buffer_ring.bgid;
        pw.fd = fd.raw() as _;
        pw.data = Some(Data { _fd: fd.clone() });
        pw.shared.id.set(id);
        unsafe {
            pw.shared.ring.get().deref_mut().write(buffer_ring.clone());
        }
        pw.shared.cb.set(Some(callback));
        let pending = PendingReadMultishot {
            ring: self.ring.clone(),
            shared: pw.shared.clone(),
            id,
        };
        self.ring.schedule(pw);
        Ok(pending)
    }
}

struct Data {
    _fd: Rc<OwnedFd>,
}

#[derive(Derivative)]
#[derivative(Default)]
struct ReadMultishotTaskShared {
    id: Cell<IoUringTaskId>,
    #[derivative(Default(value = "UnsafeCell::new(MaybeUninit::uninit())"))]
    ring: UnsafeCell<MaybeUninit<Rc<BufferRing>>>,
    cb: CloneCell<Option<Rc<dyn ReadMultishotCallback>>>,
}

#[derive(Default)]
pub struct ReadMultishotTask {
    shared: Rc<ReadMultishotTaskShared>,
    bgid: u16,
    fd: i32,
    data: Option<Data>,
}

impl ReadMultishotTaskShared {
    unsafe fn get_buffer(&self, cqe: &io_uring_cqe) -> BufferRingBuffer {
        unsafe { self.ring.get().deref().assume_init_ref().get_buffer(cqe) }
    }
}

unsafe impl MultishotTask for ReadMultishotTaskShared {
    fn complete_partial(self: Rc<Self>, cqe: &io_uring_cqe) {
        let buffer = unsafe { self.get_buffer(cqe) };
        if let Some(cb) = self.cb.get() {
            cb.read(buffer);
        }
    }
}

unsafe impl Task for ReadMultishotTask {
    fn id(&self) -> IoUringTaskId {
        self.shared.id.get()
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, cqe: &io_uring_cqe) {
        self.data.take();
        self.shared.id.set(Default::default());
        let buffer = unsafe { self.shared.get_buffer(cqe) };
        let res = if cqe.res < 0 {
            if cqe.res == -c::ENOBUFS {
                Ok(None)
            } else {
                Err(OsError::from(-cqe.res as c::c_int))
            }
        } else {
            Ok(Some(buffer))
        };
        unsafe {
            self.shared.ring.get().deref_mut().assume_init_drop();
        }
        if let Some(cb) = self.shared.cb.take() {
            cb.done(res);
        }
        ring.cached_read_multishot.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_READ_MULTISHOT;
        sqe.flags = IOSQE_BUFFER_SELECT;
        sqe.fd = self.fd as _;
        sqe.u1.off = !0;
        sqe.u4.buf_group = self.bgid;
    }

    fn multishot(&self) -> Option<Rc<dyn MultishotTask>> {
        Some(self.shared.clone())
    }
}
