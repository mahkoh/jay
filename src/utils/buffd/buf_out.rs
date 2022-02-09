use crate::async_engine::{AsyncFd, Timeout};
use crate::utils::buffd::{BufFdError, BUF_SIZE, CMSG_BUF_SIZE};
use futures::future::Fuse;
use futures::{select, FutureExt};
use std::collections::VecDeque;
use std::mem::MaybeUninit;
use std::rc::Rc;
use std::{mem, slice};
use uapi::{c, Errno, OwnedFd};

pub(super) const OUT_BUF_SIZE: usize = 2 * BUF_SIZE;

pub(super) struct MsgFds {
    pub(super) pos: usize,
    pub(super) fds: Vec<Rc<OwnedFd>>,
}

pub struct OutBuffer {
    pub(super) read_pos: usize,
    pub(super) write_pos: usize,
    pub(super) buf: *mut [MaybeUninit<u8>; OUT_BUF_SIZE],
    pub(super) fds: VecDeque<MsgFds>,
}

impl Default for OutBuffer {
    fn default() -> Self {
        Self {
            read_pos: 0,
            write_pos: 0,
            buf: Box::into_raw(Box::new([MaybeUninit::<u32>::uninit(); OUT_BUF_SIZE / 4])) as _,
            fds: Default::default(),
        }
    }
}

impl OutBuffer {
    pub fn write(&mut self, bytes: &[MaybeUninit<u8>]) {
        if bytes.len() > OUT_BUF_SIZE - self.write_pos {
            panic!("Out buffer overflow");
        }
        unsafe {
            (*self.buf)[self.write_pos..self.write_pos + bytes.len()].copy_from_slice(bytes);
        }
        self.write_pos += bytes.len();
    }

    pub fn is_full(&self) -> bool {
        self.write_pos > BUF_SIZE
    }
}

const LIMIT_PENDING: usize = 10;

#[derive(Default)]
pub struct OutBufferSwapchain {
    pub cur: OutBuffer,
    pub pending: VecDeque<OutBuffer>,
    pub free: Vec<OutBuffer>,
}

impl OutBufferSwapchain {
    pub fn exceeds_limit(&self) -> bool {
        self.pending.len() > LIMIT_PENDING
    }

    pub fn commit(&mut self) {
        if self.cur.write_pos > 0 {
            let new = self.free.pop().unwrap_or_else(|| {
                Default::default()
            });
            let old = mem::replace(&mut self.cur, new);
            self.pending.push_back(old);
        }
    }
}

pub struct BufFdOut {
    fd: AsyncFd,
    cmsg_buf: Box<[MaybeUninit<u8>; CMSG_BUF_SIZE]>,
    fd_ids: Vec<i32>,
}

impl BufFdOut {
    pub fn new(fd: AsyncFd) -> Self {
        Self {
            fd,
            cmsg_buf: Box::new([MaybeUninit::uninit(); CMSG_BUF_SIZE]),
            fd_ids: vec![],
        }
    }

    pub async fn flush(
        &mut self,
        buf: &mut OutBuffer,
        timeout: &mut Option<Fuse<Timeout>>,
    ) -> Result<(), BufFdError> {
        while buf.read_pos < buf.write_pos {
            if self.flush_sync(buf)? {
                self.fd.writable().await?;
                if timeout.is_none() {
                    *timeout = Some(self.fd.eng().timeout(5000)?.fuse());
                }
                select! {
                    _ = timeout.as_mut().unwrap() => {
                        return Err(BufFdError::Timeout);
                    },
                    res = self.fd.writable().fuse() => res?,
                }
            }
        }
        buf.read_pos = 0;
        buf.write_pos = 0;
        Ok(())
    }

    fn flush_sync(&mut self, buffer: &mut OutBuffer) -> Result<bool, BufFdError> {
        while buffer.read_pos < buffer.write_pos {
            let mut buf = unsafe { &(*buffer.buf)[buffer.read_pos..buffer.write_pos] };
            let mut cmsg_len = 0;
            let mut fds_opt = None;
            {
                let mut f = buffer.fds.front().map(|f| f.pos);
                if f == Some(buffer.read_pos) {
                    let fds = buffer.fds.pop_front().unwrap();
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
                    f = buffer.fds.front().map(|f| f.pos)
                }
                if let Some(next_pos) = f {
                    buf = &buf[..next_pos - buffer.read_pos];
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
                            buffer.fds.push_front(fds);
                        }
                        return Ok(true);
                    }
                    Err(Errno(c::ECONNRESET)) => return Err(BufFdError::Closed),
                    Err(e) => return Err(BufFdError::Io(e.into())),
                };
            buffer.read_pos += bytes_sent;
        }
        Ok(false)
    }
}

impl Drop for OutBuffer {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.buf as *mut [MaybeUninit<u32>; OUT_BUF_SIZE / 4]);
        }
    }
}
