use {
    crate::{
        io_uring::{IoUring, IoUringError},
        time::Time,
        utils::{
            buf::Buf,
            buffd::{BufFdError, BUF_SIZE, CMSG_BUF_SIZE},
            oserror::OsError,
        },
    },
    std::{
        collections::VecDeque,
        mem::{self, MaybeUninit},
        rc::Rc,
        slice,
    },
    uapi::{c, Errno, OwnedFd},
};

pub(super) const OUT_BUF_SIZE: usize = 2 * BUF_SIZE;

pub(super) struct MsgFds {
    pub(super) pos: usize,
    pub(super) fds: Vec<Rc<OwnedFd>>,
}

pub(super) struct OutBufferMeta {
    pub(super) read_pos: usize,
    pub(super) write_pos: usize,
    pub(super) fds: VecDeque<MsgFds>,
}

pub struct OutBuffer {
    pub(super) meta: OutBufferMeta,
    pub(super) buf: Buf,
}

impl Default for OutBuffer {
    fn default() -> Self {
        Self {
            meta: OutBufferMeta {
                read_pos: 0,
                write_pos: 0,
                fds: Default::default(),
            },
            buf: Buf::new(OUT_BUF_SIZE),
        }
    }
}

impl OutBuffer {
    pub fn is_full(&self) -> bool {
        self.meta.write_pos > BUF_SIZE
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
        if self.cur.meta.write_pos > 0 {
            let new = self.free.pop().unwrap_or_default();
            let old = mem::replace(&mut self.cur, new);
            self.pending.push_back(old);
        }
    }
}

pub struct BufFdOut {
    fd: Rc<OwnedFd>,
    ring: Rc<IoUring>,
    cmsg_buf: Box<[MaybeUninit<u8>; CMSG_BUF_SIZE]>,
    fd_ids: Vec<i32>,
}

impl BufFdOut {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        Self {
            fd: fd.clone(),
            ring: ring.clone(),
            cmsg_buf: Box::new([MaybeUninit::uninit(); CMSG_BUF_SIZE]),
            fd_ids: vec![],
        }
    }

    pub async fn flush(&mut self, buf: &mut OutBuffer, timeout: Time) -> Result<(), BufFdError> {
        while buf.meta.read_pos < buf.meta.write_pos {
            self.flush_buffer(buf, Some(timeout)).await?;
        }
        buf.meta.read_pos = 0;
        buf.meta.write_pos = 0;
        Ok(())
    }

    pub async fn flush_no_timeout(&mut self, buf: &mut OutBuffer) -> Result<(), BufFdError> {
        while buf.meta.read_pos < buf.meta.write_pos {
            self.flush_buffer(buf, None).await?;
        }
        buf.meta.read_pos = 0;
        buf.meta.write_pos = 0;
        Ok(())
    }

    async fn flush_buffer(
        &mut self,
        buffer: &mut OutBuffer,
        timeout: Option<Time>,
    ) -> Result<(), BufFdError> {
        let mut buf = buffer
            .buf
            .slice(buffer.meta.read_pos..buffer.meta.write_pos);
        let mut fds = vec![];
        {
            let mut f = buffer.meta.fds.front().map(|f| f.pos);
            if f == Some(buffer.meta.read_pos) {
                fds = buffer.meta.fds.pop_front().unwrap().fds;
                f = buffer.meta.fds.front().map(|f| f.pos)
            }
            if let Some(next_pos) = f {
                buf = buffer.buf.slice(buffer.meta.read_pos..next_pos);
            }
        }
        match self.ring.sendmsg(&self.fd, buf, fds, timeout).await {
            Ok(n) => {
                buffer.meta.read_pos += n;
                Ok(())
            }
            Err(IoUringError::OsError(OsError(c::ECONNRESET))) => return Err(BufFdError::Closed),
            Err(IoUringError::OsError(OsError(c::ECANCELED))) => return Err(BufFdError::Timeout),
            Err(e) => return Err(BufFdError::Ring(e)),
        }
    }

    pub async fn flush2(
        &mut self,
        buf: &[u8],
        fds: &mut Vec<Rc<OwnedFd>>,
    ) -> Result<(), BufFdError> {
        let mut read_pos = 0;
        while read_pos < buf.len() {
            if self.flush_sync2(&mut read_pos, buf, fds)? {
                self.ring.writable(&self.fd).await?;
            }
        }
        Ok(())
    }

    fn flush_sync2(
        &mut self,
        read_pos: &mut usize,
        buf: &[u8],
        fds: &mut Vec<Rc<OwnedFd>>,
    ) -> Result<bool, BufFdError> {
        let mut cmsg_len = 0;
        let mut fds_opt = None;
        if fds.len() > 0 {
            self.fd_ids.clear();
            self.fd_ids.extend(fds.iter().map(|f| f.raw()));
            let hdr = c::cmsghdr {
                cmsg_len: 0,
                cmsg_level: c::SOL_SOCKET,
                cmsg_type: c::SCM_RIGHTS,
            };
            let mut cmsg_buf = &mut self.cmsg_buf[..];
            cmsg_len = uapi::cmsg_write(&mut cmsg_buf, hdr, &self.fd_ids[..]).unwrap();
            fds_opt = Some(fds);
        }
        while *read_pos < buf.len() {
            let buf = &buf[*read_pos..];
            let hdr = uapi::Msghdr {
                iov: slice::from_ref(&buf),
                control: Some(&self.cmsg_buf[..cmsg_len]),
                name: uapi::sockaddr_none_ref(),
            };
            let bytes_sent =
                match uapi::sendmsg(self.fd.raw(), &hdr, c::MSG_DONTWAIT | c::MSG_NOSIGNAL) {
                    Ok(b) => {
                        if let Some(fds) = fds_opt.take() {
                            fds.clear();
                        }
                        b
                    }
                    Err(Errno(c::EAGAIN)) => return Ok(true),
                    Err(Errno(c::ECONNRESET)) => return Err(BufFdError::Closed),
                    Err(e) => return Err(BufFdError::Io(e.into())),
                };
            *read_pos += bytes_sent;
        }
        Ok(false)
    }
}
