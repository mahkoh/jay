use {
    crate::{
        io_uring::IoUring,
        object::ObjectId,
        utils::{
            buf::Buf,
            buffd::{BufFdError, MAX_IN_FD},
        },
    },
    std::{collections::VecDeque, ptr, rc::Rc, slice},
    uapi::OwnedFd,
};

const WORD_SIZE: usize = 4;
const WORD_ALIGN: usize = 4;
const HEADER_WORDS: usize = 2;
const HEADER_SIZE: usize = HEADER_WORDS * WORD_SIZE;
const MAX_MESSAGE_SIZE: usize = 4096;
const BUF_SIZE: usize = 2 * MAX_MESSAGE_SIZE;

pub struct WlBufFdIn {
    fd: Rc<OwnedFd>,
    ring: Rc<IoUring>,
    fds: VecDeque<Rc<OwnedFd>>,
    buf: Buf,
    lo: usize,
    len: usize,
}

pub struct WlMessage<'a> {
    pub obj_id: ObjectId,
    pub message: u32,
    pub body: &'a [u32],
    pub fds: &'a mut VecDeque<Rc<OwnedFd>>,
}

impl WlBufFdIn {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        let buf = Buf::new(BUF_SIZE);
        assert_eq!(buf.as_ptr() as usize % WORD_ALIGN, 0);
        Self {
            fd: fd.clone(),
            ring: ring.clone(),
            fds: Default::default(),
            buf,
            lo: Default::default(),
            len: Default::default(),
        }
    }

    pub async fn read_message(&mut self) -> Result<WlMessage<'_>, BufFdError> {
        if self.len == 0 {
            self.lo = 0;
        }
        if self.len < HEADER_SIZE {
            if self.lo > 0 {
                self.compact();
            }
            while self.len < HEADER_SIZE {
                self.recvmsg().await?;
            }
        }
        let hdr: &[u32] =
            unsafe { slice::from_raw_parts(self.buf[self.lo..].as_ptr().cast(), HEADER_WORDS) };
        let obj_id = ObjectId::from_raw(hdr[0]);
        let len = (hdr[1] >> 16) as usize;
        let message = hdr[1] & 0xffff;
        if len & 3 != 0 {
            return Err(BufFdError::UnalignedMessageSize);
        }
        if len > MAX_MESSAGE_SIZE {
            return Err(BufFdError::MessageTooLarge);
        }
        if len < HEADER_SIZE {
            return Err(BufFdError::MessageTooSmall);
        }
        if len > self.len {
            if self.lo + self.len >= MAX_MESSAGE_SIZE {
                self.compact();
            }
            while len > self.len {
                self.recvmsg().await?;
            }
        }
        let body: &[u32] = unsafe {
            let words = (len - HEADER_SIZE) >> 2;
            slice::from_raw_parts(self.buf[self.lo + HEADER_SIZE..].as_ptr().cast(), words)
        };
        self.lo += len;
        self.len -= len;
        Ok(WlMessage {
            obj_id,
            message,
            body,
            fds: &mut self.fds,
        })
    }

    #[inline(always)]
    fn compact(&mut self) {
        unsafe {
            let dst = self.buf.as_mut_ptr();
            let src = dst.add(self.lo);
            ptr::copy(src, dst, self.len);
            self.lo = 0;
        }
    }

    async fn recvmsg(&mut self) -> Result<(), BufFdError> {
        let mut buf = self.buf.slice(self.lo + self.len..);
        match self
            .ring
            .recvmsg(&self.fd, slice::from_mut(&mut buf), &mut self.fds)
            .await
        {
            Ok(0) => return Err(BufFdError::Closed),
            Ok(n) => self.len += n,
            Err(e) => return Err(BufFdError::Ring(e)),
        }
        if self.fds.len() > MAX_IN_FD {
            return Err(BufFdError::TooManyFds);
        }
        Ok(())
    }
}
