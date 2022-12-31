use {
    crate::{
        io_uring::{IoUring, IoUringError},
        time::Time,
        utils::{
            buf::Buf,
            buffd::{BufFdError, BUF_SIZE},
            oserror::OsError,
        },
    },
    std::{
        collections::VecDeque,
        mem::{self},
        rc::Rc,
    },
    uapi::{c, OwnedFd},
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
}

impl BufFdOut {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        Self {
            fd: fd.clone(),
            ring: ring.clone(),
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
        match self.ring.sendmsg_one(&self.fd, buf, fds, timeout).await {
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
        mut buf: Buf,
        mut fds: Vec<Rc<OwnedFd>>,
    ) -> Result<(), BufFdError> {
        let mut read_pos = 0;
        while read_pos < buf.len() {
            let res = self
                .ring
                .sendmsg_one(&self.fd, buf.slice(read_pos..), mem::take(&mut fds), None)
                .await;
            match res {
                Ok(n) => read_pos += n,
                Err(IoUringError::OsError(OsError(c::ECONNRESET))) => {
                    return Err(BufFdError::Closed)
                }
                Err(e) => return Err(BufFdError::Io(e)),
            }
        }
        Ok(())
    }
}
