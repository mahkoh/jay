use {
    crate::{
        io_uring::{IoUring, IoUringError},
        utils::{
            buf::{Buf, DynamicBuf},
            queue::AsyncQueue,
            stack::Stack,
        },
    },
    std::{
        collections::VecDeque,
        mem::{self},
        rc::Rc,
    },
    thiserror::Error,
    uapi::{OwnedFd, c},
};

#[derive(Debug, Error)]
pub enum BufIoError {
    #[error("Could not write to the socket")]
    FlushError(#[source] IoUringError),
    #[error("Could not read from the socket")]
    ReadError(#[source] IoUringError),
    #[error("The socket is closed")]
    Closed,
}

pub struct BufIoMessage {
    pub fds: Vec<Rc<OwnedFd>>,
    pub buf: Buf,
}

struct MessageOffset {
    msg: BufIoMessage,
    offset: usize,
}

pub struct BufIo {
    fd: Rc<OwnedFd>,
    ring: Rc<IoUring>,
    bufs: Stack<Buf>,
    outgoing: AsyncQueue<BufIoMessage>,
}

pub struct BufIoIncoming {
    bufio: Rc<BufIo>,

    buf: Buf,
    buf_start: usize,
    buf_end: usize,
    pub fds: VecDeque<Rc<OwnedFd>>,
}

struct Outgoing {
    bufio: Rc<BufIo>,

    msgs: VecDeque<MessageOffset>,
    bufs: Vec<Buf>,
}

impl BufIo {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        Self {
            fd: fd.clone(),
            ring: ring.clone(),
            bufs: Default::default(),
            outgoing: Default::default(),
        }
    }

    pub fn shutdown(&self) {
        let _ = uapi::shutdown(self.fd.raw(), c::SHUT_RDWR);
    }

    pub fn buf(&self) -> DynamicBuf {
        let buf = self.bufs.pop().unwrap_or_default();
        DynamicBuf::from_buf(buf)
    }

    pub fn send(&self, msg: BufIoMessage) {
        self.outgoing.push(msg);
    }

    pub async fn outgoing(self: Rc<Self>) -> Result<(), BufIoError> {
        let mut outgoing = Outgoing {
            bufio: self,
            msgs: Default::default(),
            bufs: vec![],
        };
        outgoing.run().await
    }

    pub fn incoming(self: &Rc<Self>) -> BufIoIncoming {
        BufIoIncoming {
            bufio: self.clone(),
            buf: Buf::new(4096),
            buf_start: 0,
            buf_end: 0,
            fds: Default::default(),
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
                self.buf_start = 0;
                self.buf_end = 0;
                let res = self
                    .bufio
                    .ring
                    .recvmsg(&self.bufio.fd, &mut [self.buf.clone()], &mut self.fds)
                    .await;
                match res {
                    Ok(n) => self.buf_end = n,
                    Err(e) => return Err(BufIoError::ReadError(e)),
                }
                if self.buf_start == self.buf_end {
                    return Err(BufIoError::Closed);
                }
            }
            let read = n.min(self.buf_end - self.buf_start);
            let buf_start = self.buf_start;
            buf.extend_from_slice(&self.buf[buf_start..buf_start + read]);
            n -= read;
            self.buf_start += read;
        }
        Ok(())
    }
}

impl Outgoing {
    async fn run(&mut self) -> Result<(), BufIoError> {
        loop {
            self.bufio.outgoing.non_empty().await;
            if let Err(e) = self.try_flush().await {
                return Err(BufIoError::FlushError(e));
            }
        }
    }

    async fn try_flush(&mut self) -> Result<(), IoUringError> {
        loop {
            while let Some(msg) = self.bufio.outgoing.try_pop() {
                self.msgs.push_back(MessageOffset { msg, offset: 0 });
            }
            if self.msgs.is_empty() {
                return Ok(());
            }
            let mut fds = Vec::new();
            for msg in &mut self.msgs {
                if msg.msg.fds.len() > 0 {
                    if fds.len() > 0 || self.bufs.len() > 0 {
                        break;
                    }
                    fds = mem::take(&mut msg.msg.fds);
                }
                self.bufs.push(msg.msg.buf.slice(msg.offset..));
            }
            let res = self
                .bufio
                .ring
                .sendmsg(&self.bufio.fd, &mut self.bufs, fds, None)
                .await;
            self.bufs.clear();
            let mut n = res?;
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
