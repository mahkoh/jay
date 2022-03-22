use crate::async_engine::AsyncFd;
use crate::utils::oserror::OsError;
use crate::utils::stack::Stack;
use crate::utils::vec_ext::{UninitVecExt, VecExt};
use crate::utils::vecstorage::VecStorage;
use crate::{AsyncError, AsyncQueue};
use std::collections::VecDeque;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, Errno, MaybeUninitSliceExt, Msghdr, MsghdrMut, OwnedFd};

#[derive(Debug, Error)]
pub enum BufIoError {
    #[error("Could not write to the socket")]
    FlushError(#[source] OsError),
    #[error("Could not read from the socket")]
    ReadError(#[source] OsError),
    #[error("Cannot wait for fd to become writable")]
    Writable(#[source] AsyncError),
    #[error("Cannot wait for fd to become readable")]
    Readable(#[source] AsyncError),
    #[error("The socket is closed")]
    Closed,
}

pub struct BufIoMessage {
    pub fds: Vec<Rc<OwnedFd>>,
    pub buf: Vec<u8>,
}

struct MessageOffset {
    msg: BufIoMessage,
    offset: usize,
}

pub struct BufIo {
    fd: AsyncFd,
    bufs: Stack<Vec<u8>>,
    outgoing: AsyncQueue<BufIoMessage>,
}

pub struct BufIoIncoming {
    bufio: Rc<BufIo>,

    buf: Box<[MaybeUninit<u8>; 4096]>,
    buf_start: usize,
    buf_end: usize,
    pub fds: VecDeque<Rc<OwnedFd>>,
    cmsg: Box<[MaybeUninit<u8>; 256]>,
}

struct Outgoing {
    bufio: Rc<BufIo>,

    msgs: VecDeque<MessageOffset>,
    cmsg: Vec<MaybeUninit<u8>>,
    fds: Vec<c::c_int>,
    iovecs: VecStorage<NonNull<[u8]>>,
}

impl BufIo {
    pub fn new(fd: AsyncFd) -> Self {
        Self {
            fd,
            bufs: Default::default(),
            outgoing: Default::default(),
        }
    }

    pub fn shutdown(&self) {
        let _ = uapi::shutdown(self.fd.raw(), c::SHUT_RDWR);
    }

    pub fn buf(&self) -> Vec<u8> {
        let mut buf = self.bufs.pop().unwrap_or_default();
        buf.clear();
        buf
    }

    pub fn add_buf(&self, buf: Vec<u8>) {
        self.bufs.push(buf);
    }

    pub fn send(&self, msg: BufIoMessage) {
        self.outgoing.push(msg);
    }

    pub async fn outgoing(self: Rc<Self>) -> Result<(), BufIoError> {
        let mut outgoing = Outgoing {
            bufio: self,
            msgs: Default::default(),
            cmsg: vec![],
            fds: vec![],
            iovecs: Default::default(),
        };
        outgoing.run().await
    }

    pub fn incoming(self: &Rc<Self>) -> BufIoIncoming {
        BufIoIncoming {
            bufio: self.clone(),
            buf: Box::new([MaybeUninit::uninit(); 4096]),
            buf_start: 0,
            buf_end: 0,
            fds: Default::default(),
            cmsg: Box::new([MaybeUninit::uninit(); 256]),
        }
    }
}

impl BufIoIncoming {
    pub async fn fill_msg_buf(
        &mut self,
        mut n: usize,
        buf: &mut Vec<u8>,
    ) -> Result<(), BufIoError> {
        while n > 0 {
            if self.buf_start == self.buf_end {
                while let Err(e) = self.recvmsg() {
                    if e.0 != c::EAGAIN {
                        return Err(BufIoError::ReadError(e.into()));
                    }
                    if let Err(e) = self.bufio.fd.readable().await {
                        return Err(BufIoError::Readable(e));
                    }
                }
                if self.buf_start == self.buf_end {
                    return Err(BufIoError::Closed);
                }
            }
            let read = n.min(self.buf_end - self.buf_start);
            let buf_start = self.buf_start % self.buf.len();
            unsafe {
                buf.extend_from_slice(
                    self.buf[buf_start..buf_start + read].slice_assume_init_ref(),
                );
            }
            n -= read;
            self.buf_start += read;
        }
        Ok(())
    }

    fn recvmsg(&mut self) -> Result<(), Errno> {
        self.buf_start = 0;
        self.buf_end = 0;
        let mut iov = [&mut self.buf[..]];
        let mut hdr = MsghdrMut {
            iov: &mut iov[..],
            control: Some(&mut self.cmsg[..]),
            name: uapi::sockaddr_none_mut(),
            flags: 0,
        };
        let (ivec, _, mut cmsg) =
            uapi::recvmsg(self.bufio.fd.raw(), &mut hdr, c::MSG_CMSG_CLOEXEC)?;
        self.buf_end += ivec.len();
        while cmsg.len() > 0 {
            let (_, hdr, body) = uapi::cmsg_read(&mut cmsg)?;
            if hdr.cmsg_level == c::SOL_SOCKET && hdr.cmsg_type == c::SCM_RIGHTS {
                for fd in uapi::pod_iter(body)? {
                    self.fds.push_back(Rc::new(OwnedFd::new(fd)));
                }
            }
        }
        Ok(())
    }
}

impl Outgoing {
    async fn run(&mut self) -> Result<(), BufIoError> {
        loop {
            self.bufio.outgoing.non_empty().await;
            while let Err(e) = self.try_flush() {
                if e != Errno(c::EAGAIN) {
                    return Err(BufIoError::FlushError(e.into()));
                }
                if let Err(e) = self.bufio.fd.writable().await {
                    return Err(BufIoError::Writable(e));
                }
            }
        }
    }

    fn try_flush(&mut self) -> Result<(), Errno> {
        loop {
            while let Some(msg) = self.bufio.outgoing.try_pop() {
                self.msgs.push_back(MessageOffset { msg, offset: 0 });
            }
            if self.msgs.is_empty() {
                return Ok(());
            }
            let mut iovecs = self.iovecs.take_as();
            let mut fds = &[][..];
            for msg in &mut self.msgs {
                if msg.msg.fds.len() > 0 {
                    if fds.len() > 0 || iovecs.len() > 0 {
                        break;
                    }
                    fds = &msg.msg.fds;
                }
                iovecs.push(&msg.msg.buf[msg.offset..]);
            }
            self.cmsg.clear();
            if fds.len() > 0 {
                self.fds.clear();
                self.fds.extend(fds.iter().map(|f| f.raw()));
                let cmsg_space = uapi::cmsg_space(fds.len() * mem::size_of::<c::c_int>());
                self.cmsg.reserve(cmsg_space);
                let (_, mut spare) = self.cmsg.split_at_spare_mut_bytes_ext();
                let hdr = c::cmsghdr {
                    cmsg_len: 0,
                    cmsg_level: c::SOL_SOCKET,
                    cmsg_type: c::SCM_RIGHTS,
                };
                let len = uapi::cmsg_write(&mut spare, hdr, &self.fds[..]).unwrap();
                self.cmsg.set_len_safe(len);
            }
            let msg = Msghdr {
                iov: &iovecs[..],
                control: Some(&self.cmsg[..]),
                name: uapi::sockaddr_none_ref(),
            };
            let mut n = uapi::sendmsg(self.bufio.fd.raw(), &msg, c::MSG_DONTWAIT)?;
            drop(iovecs);
            self.msgs[0].msg.fds.clear();
            while n > 0 {
                let len = self.msgs[0].msg.buf.len() - self.msgs[0].offset;
                if n < len {
                    self.msgs[0].offset += n;
                    break;
                }
                n -= len;
                let msg = self.msgs.pop_front().unwrap();
                self.bufio.bufs.push(msg.msg.buf);
            }
        }
    }
}
