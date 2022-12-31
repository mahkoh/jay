use {
    crate::{
        io_uring::{
            pending_result::PendingResult,
            sys::{io_uring_sqe, IORING_OP_SENDMSG},
            IoUring, IoUringData, IoUringError, Task,
        },
        time::Time,
        utils::{buf::Buf, vec_ext::UninitVecExt},
    },
    std::{
        mem::{self, MaybeUninit},
        ptr,
        rc::Rc,
    },
    uapi::{c, OwnedFd},
};

impl IoUring {
    pub async fn sendmsg_one(
        &self,
        fd: &Rc<OwnedFd>,
        buf: Buf,
        fds: Vec<Rc<OwnedFd>>,
        timeout: Option<Time>,
    ) -> Result<usize, IoUringError> {
        self.sendmsg(fd, &mut [buf], fds, timeout).await
    }

    pub async fn sendmsg(
        &self,
        fd: &Rc<OwnedFd>,
        bufs: &mut [Buf],
        fds: Vec<Rc<OwnedFd>>,
        timeout: Option<Time>,
    ) -> Result<usize, IoUringError> {
        self.ring.check_destroyed()?;
        let id = self.ring.id();
        let pr = self.ring.pending_results.acquire();
        {
            let mut st = self.ring.cached_sendmsg.pop().unwrap_or_default();
            st.fds = fds;
            if st.fds.len() > 0 {
                let mut fd_ids = self.ring.fd_ids_scratch.borrow_mut();
                fd_ids.clear();
                fd_ids.extend(st.fds.iter().map(|f| f.raw()));
                let space = uapi::cmsg_space(mem::size_of_val(&fd_ids[..]));
                st.cmsg.clear();
                st.cmsg.reserve(space);
                st.cmsg.set_len_safe(space);
                let hdr = c::cmsghdr {
                    cmsg_len: 0,
                    cmsg_level: c::SOL_SOCKET,
                    cmsg_type: c::SCM_RIGHTS,
                };
                uapi::cmsg_write(&mut &mut st.cmsg[..], hdr, &fd_ids[..]).unwrap();
                st.msghdr.msg_control = st.cmsg.as_ptr() as _;
                st.msghdr.msg_controllen = st.cmsg.len() as _;
            } else {
                st.msghdr.msg_control = ptr::null_mut();
                st.msghdr.msg_controllen = 0;
            }
            st.id = id.id;
            st.fd = fd.raw();
            st.bufs.clear();
            st.bufs.extend(bufs.iter_mut().map(|b| b.clone()));
            st.iovecs.clear();
            st.iovecs.extend(bufs.iter().map(|b| c::iovec {
                iov_base: b.as_ptr() as _,
                iov_len: b.len(),
            }));
            st.msghdr.msg_iov = st.iovecs.as_ptr() as _;
            st.msghdr.msg_iovlen = st.iovecs.len();
            st.data = Some(SendmsgTaskData {
                _fd: fd.clone(),
                res: pr.clone(),
            });
            st.has_timeout = timeout.is_some();
            self.ring.schedule(st);
            if let Some(timeout) = timeout {
                self.schedule_timeout(timeout);
            }
        }
        Ok(pr.await? as _)
    }
}

struct SendmsgTaskData {
    _fd: Rc<OwnedFd>,
    res: PendingResult,
}

pub struct SendmsgTask {
    id: u64,
    iovecs: Vec<c::iovec>,
    msghdr: c::msghdr,
    bufs: Vec<Buf>,
    fd: i32,
    has_timeout: bool,
    fds: Vec<Rc<OwnedFd>>,
    cmsg: Vec<MaybeUninit<u8>>,
    data: Option<SendmsgTaskData>,
}

impl Default for SendmsgTask {
    fn default() -> Self {
        unsafe {
            SendmsgTask {
                id: 0,
                iovecs: vec![],
                msghdr: MaybeUninit::zeroed().assume_init(),
                bufs: vec![],
                fd: 0,
                has_timeout: false,
                fds: vec![],
                cmsg: vec![],
                data: None,
            }
        }
    }
}

unsafe impl Task for SendmsgTask {
    fn id(&self) -> u64 {
        self.id
    }

    fn complete(mut self: Box<Self>, ring: &IoUringData, res: i32) {
        self.fds.clear();
        self.bufs.clear();
        if let Some(data) = self.data.take() {
            data.res.complete(res);
        }
        ring.cached_sendmsg.push(self);
    }

    fn encode(&self, sqe: &mut io_uring_sqe) {
        sqe.opcode = IORING_OP_SENDMSG;
        sqe.fd = self.fd;
        sqe.u2.addr = &self.msghdr as *const _ as _;
        sqe.u3.msg_flags = c::MSG_NOSIGNAL as _;
    }

    fn has_timeout(&self) -> bool {
        self.has_timeout
    }
}
