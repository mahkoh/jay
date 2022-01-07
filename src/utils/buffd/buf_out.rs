use crate::async_engine::AsyncFd;
use crate::utils::buffd::{BufFdError, BUF_SIZE, CMSG_BUF_SIZE};
use futures::{select, FutureExt};
use std::collections::VecDeque;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::slice;
use uapi::{c, Errno, OwnedFd};

pub(super) const OUT_BUF_SIZE: usize = 2 * BUF_SIZE;

pub(super) struct MsgFds {
    pub(super) pos: usize,
    pub(super) fds: Vec<Rc<OwnedFd>>,
}

pub struct BufFdOut {
    fd: AsyncFd,

    pub(super) out_pos: usize,
    pub(super) out_buf: *mut [MaybeUninit<u8>; OUT_BUF_SIZE],

    pub(super) fds: VecDeque<MsgFds>,
    fd_ids: Vec<i32>,
    cmsg_buf: Box<[MaybeUninit<u8>; CMSG_BUF_SIZE]>,
}

impl BufFdOut {
    pub fn new(fd: AsyncFd) -> Self {
        Self {
            fd,
            out_pos: 0,
            out_buf: Box::into_raw(Box::new([MaybeUninit::<u32>::uninit(); OUT_BUF_SIZE / 4])) as _,
            fds: Default::default(),
            fd_ids: vec![],
            cmsg_buf: Box::new([MaybeUninit::uninit(); CMSG_BUF_SIZE]),
        }
    }

    pub fn write(&mut self, bytes: &[MaybeUninit<u8>]) {
        if bytes.len() > OUT_BUF_SIZE - self.out_pos {
            panic!("Out buffer overflow");
        }
        unsafe {
            (*self.out_buf)[self.out_pos..self.out_pos + bytes.len()].copy_from_slice(bytes);
        }
        self.out_pos += bytes.len();
    }

    pub fn needs_flush(&self) -> bool {
        self.out_pos > BUF_SIZE
    }

    pub async fn flush(&mut self) -> Result<(), BufFdError> {
        let mut timeout = None;
        let mut pos = 0;
        while pos < self.out_pos {
            if self.flush_sync(&mut pos)? {
                if timeout.is_none() {
                    timeout = Some(self.fd.eng().timeout(5000)?.fuse());
                }
                select! {
                    _ = timeout.as_mut().unwrap() => return Err(BufFdError::Timeout),
                    res = self.fd.writable().fuse() => res?,
                }
            }
        }
        self.out_pos = 0;
        Ok(())
    }

    fn flush_sync(&mut self, pos: &mut usize) -> Result<bool, BufFdError> {
        while *pos < self.out_pos {
            let mut buf = unsafe { &(*self.out_buf)[*pos..self.out_pos] };
            let mut cmsg_len = 0;
            let mut fds_opt = None;
            {
                let mut f = self.fds.front().map(|f| f.pos);
                if f == Some(*pos) {
                    let fds = self.fds.pop_front().unwrap();
                    self.fd_ids.clear();
                    self.fd_ids.extend(fds.fds.iter().map(|f| f.raw()));
                    let hdr = c::cmsghdr {
                        cmsg_len: 0,
                        cmsg_level: c::SOL_SOCKET,
                        cmsg_type: c::SCM_RIGHTS,
                    };
                    let mut cmsg_buf = &mut self.cmsg_buf[..];
                    cmsg_len = uapi::cmsg_write(&mut cmsg_buf, hdr, &self.fd_ids[..]).unwrap();
                    fds_opt = Some(fds);
                    f = self.fds.front().map(|f| f.pos)
                }
                if let Some(next_pos) = f {
                    buf = &buf[..next_pos - *pos];
                }
            }
            let hdr = uapi::Msghdr {
                iov: slice::from_ref(&buf),
                control: Some(&self.cmsg_buf[..cmsg_len]),
                name: uapi::sockaddr_none_ref(),
            };
            let bytes_sent =
                match uapi::sendmsg(self.fd.raw(), &hdr, c::MSG_DONTWAIT | c::MSG_NOSIGNAL) {
                    Ok(b) => b,
                    Err(Errno(c::EAGAIN)) => {
                        if let Some(fds) = fds_opt {
                            self.fds.push_front(fds);
                        }
                        return Ok(true);
                    }
                    Err(Errno(c::ECONNRESET)) => return Err(BufFdError::Closed),
                    Err(e) => return Err(BufFdError::Io(e.into())),
                };
            *pos += bytes_sent;
        }
        Ok(false)
    }
}

impl Drop for BufFdOut {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.out_buf as *mut [MaybeUninit<u32>; OUT_BUF_SIZE / 4]);
        }
    }
}
