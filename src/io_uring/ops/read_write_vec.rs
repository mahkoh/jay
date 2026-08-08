use crate::io_uring::IoUring;
use crate::io_uring::IoUringData;
use crate::io_uring::IoUringError;
use crate::io_uring::IoUringTaskId;
use crate::io_uring::Task;
use crate::io_uring::TaskResultExt;
use crate::io_uring::pending_result::PendingResult;
use crate::io_uring::sys::IORING_OP_WRITE;
use crate::io_uring::sys::io_uring_sqe;
use crate::utils::keep_alive::KeepAlive;
use crate::utils::ptr_ext::PtrExt;
use crate::utils::stack::Stack;
use derivative::Derivative;
use std::cell::Cell;
use std::mem;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct WriteVecCache<T> {
    cache: Stack<Rc<Cell<T>>>,
}

impl IoUring {
    #[expect(unused)]
    pub async fn write_vec_all<T>(
        &self,
        fd: &Rc<OwnedFd>,
        cache: &WriteVecCache<T>,
        vec: &mut T,
    ) -> Result<(), IoUringError>
    where
        T: AsRef<[u8]> + Default + 'static,
    {
        let mut off = 0;
        let len = vec.as_ref().len();
        while off < len {
            let n = self.write_vec(fd, cache, vec, off).await?;
            off += n;
        }
        Ok(())
    }

    pub async fn write_vec<T>(
        &self,
        fd: &Rc<OwnedFd>,
        cache: &WriteVecCache<T>,
        vec_mut: &mut T,
        mut off: usize,
    ) -> Result<usize, IoUringError>
    where
        T: AsRef<[u8]> + Default + 'static,
    {
        self.ring.check_destroyed()?;
        let storage = cache.cache.pop().unwrap_or_default();
        storage.set(mem::take(vec_mut));
        let slice = unsafe { storage.as_ptr().deref().as_ref() };
        let len = slice.len();
        off = off.min(len);
        let ptr = unsafe { slice.as_ptr().add(off) as _ };
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_write_vecs.pop().unwrap_or_default();
            pw.id = id.id;
            pw.fd = fd.raw();
            pw.ptr = ptr;
            pw.len = len - off;
            pw.data = Some(TaskData {
                _fd: fd.clone(),
                _storage: storage.clone(),
                res: pr.clone(),
            });
            self.ring.schedule(pw);
        }
        let res = pr.await.map(|v| v as usize);
        *vec_mut = storage.take();
        cache.cache.push(storage);
        Ok(res).merge()
    }
}

struct TaskData {
    _fd: Rc<OwnedFd>,
    _storage: Rc<Cell<dyn KeepAlive>>,
    res: PendingResult,
}

#[derive(Default)]
pub struct WriteVecTask {
    id: IoUringTaskId,
    fd: c::c_int,
    ptr: usize,
    len: usize,
    data: Option<TaskData>,
}

unsafe impl Task for WriteVecTask {
    fn id(&self) -> IoUringTaskId {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.res.complete(res);
        }
        ring.cached_write_vecs.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_WRITE;
        sqe.fd = self.fd as _;
        sqe.u1.off = !0;
        sqe.u2.addr = self.ptr as _;
        sqe.u3.rw_flags = 0;
        sqe.len = self.len as _;
    }
}
