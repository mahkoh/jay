use {
    crate::io_uring::{
        pending_result::PendingResult,
        sys::{io_uring_sqe, IORING_OP_CONNECT},
        IoUring, IoUringData, IoUringError, Task, TaskResultExt,
    },
    std::{mem, ptr, rc::Rc},
    uapi::{c, OwnedFd, SockAddr},
};

impl IoUring {
    pub async fn connect<T: SockAddr>(&self, fd: &Rc<OwnedFd>, t: &T) -> Result<(), IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut pw = self.ring.cached_connects.pop().unwrap_or_default();
            pw.id = id.id;
            pw.fd = fd.raw() as _;
            unsafe {
                ptr::copy_nonoverlapping(t, &mut pw.sockaddr as *mut _ as *mut _, 1);
            }
            pw.addrlen = mem::size_of::<T>() as _;
            pw.data = Some(Data {
                pr: pr.clone(),
                _fd: fd.clone(),
            });
            self.ring.schedule(pw);
        }
        Ok(pr.await.map(drop)).merge()
    }
}

struct Data {
    pr: PendingResult,
    _fd: Rc<OwnedFd>,
}

pub struct ConnectTask {
    id: u64,
    fd: i32,
    sockaddr: c::sockaddr_storage,
    addrlen: u64,
    data: Option<Data>,
}

impl Default for ConnectTask {
    fn default() -> Self {
        Self {
            id: 0,
            fd: 0,
            sockaddr: uapi::pod_zeroed(),
            addrlen: 0,
            data: None,
        }
    }
}

unsafe impl Task for ConnectTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        if let Some(data) = self.data.take() {
            data.pr.complete(res);
        }
        ring.cached_connects.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_CONNECT;
        sqe.fd = self.fd;
        sqe.u2.addr = &self.sockaddr as *const _ as _;
        sqe.u1.off = self.addrlen;
    }
}
