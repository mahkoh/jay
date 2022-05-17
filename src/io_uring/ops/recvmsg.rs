use {
    crate::{
        io_uring::{
            pending_result::PendingResult,
            sys::{io_uring_sqe, IORING_OP_RECVMSG},
            IoUring, IoUringData, IoUringError, Task,
        },
        utils::buf::Buf,
    },
    std::{cell::Cell, collections::VecDeque, mem::MaybeUninit, rc::Rc},
    uapi::{c, OwnedFd},
};

impl IoUring {
    pub async fn recvmsg(
        &self,
        fd: &Rc<OwnedFd>,
        bufs: &mut [Buf],
        fds: &mut VecDeque<OwnedFd>,
    ) -> Result<usize, IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        let mut cmsg = self.ring.cmsg_buf();
        let cmsg_len;
        {
            let mut rm = self.ring.cached_recvmsg.pop().unwrap_or_default();
            rm.iovecs.clear();
            for buf in bufs {
                rm.bufs.push(buf.clone());
                rm.iovecs.push(c::iovec {
                    iov_base: buf.as_ptr() as _,
                    iov_len: buf.len() as _,
                });
            }
            rm.id = id.id;
            rm.fd = fd.raw();
            rm.msghdr.msg_control = cmsg.as_ptr() as _;
            rm.msghdr.msg_controllen = cmsg.len() as _;
            rm.msghdr.msg_iov = rm.iovecs.as_mut_ptr();
            rm.msghdr.msg_iovlen = rm.iovecs.len() as _;
            rm.data = Some(Data {
                _cmsg: cmsg.clone(),
                _fd: fd.clone(),
                pr: pr.clone(),
            });
            cmsg_len = rm.cmsg_len.clone();
            self.ring.schedule(rm);
        }
        macro_rules! return_cmsg {
            () => {
                self.ring.cached_cmsg_bufs.push(cmsg);
            };
        }
        match pr.await {
            Ok(n) => {
                let mut cmsg_data = &cmsg[..cmsg_len.get()];
                while cmsg_data.len() > 0 {
                    let (_, hdr, data) = match uapi::cmsg_read(&mut cmsg_data) {
                        Ok(m) => m,
                        Err(_) => {
                            return_cmsg!();
                            return Err(IoUringError::InvalidCmsgData);
                        }
                    };
                    if (hdr.cmsg_level, hdr.cmsg_type) == (c::SOL_SOCKET, c::SCM_RIGHTS) {
                        fds.extend(uapi::pod_iter(data).unwrap());
                    }
                }
                return_cmsg!();
                Ok(n as _)
            }
            Err(e) => {
                return_cmsg!();
                Err(IoUringError::OsError(e))
            }
        }
    }
}

struct Data {
    _cmsg: Buf,
    _fd: Rc<OwnedFd>,
    pr: PendingResult,
}

pub struct RecvmsgTask {
    id: u64,
    fd: c::c_int,
    bufs: Vec<Buf>,
    iovecs: Vec<c::iovec>,
    msghdr: c::msghdr,
    cmsg_len: Rc<Cell<usize>>,
    data: Option<Data>,
}

impl Default for RecvmsgTask {
    fn default() -> Self {
        RecvmsgTask {
            id: 0,
            fd: 0,
            bufs: vec![],
            iovecs: vec![],
            msghdr: unsafe { MaybeUninit::zeroed().assume_init() },
            cmsg_len: Rc::new(Cell::new(0)),
            data: None,
        }
    }
}

unsafe impl Task for RecvmsgTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        self.cmsg_len.set(self.msghdr.msg_controllen as _);
        self.bufs.clear();
        if let Some(data) = self.data.take() {
            data.pr.complete(res);
        }
        ring.cached_recvmsg.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_RECVMSG;
        sqe.fd = self.fd as _;
        sqe.u2.addr = &self.msghdr as *const _ as _;
        sqe.u3.msg_flags = c::MSG_CMSG_CLOEXEC as _;
    }
}
