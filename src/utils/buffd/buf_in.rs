use {
    crate::{
        io_uring::IoUring,
        utils::{
            buf::Buf,
            buffd::{BUF_SIZE, BufFdError, MAX_IN_FD},
        },
    },
    smallvec::SmallVec,
    std::{collections::VecDeque, mem::MaybeUninit, rc::Rc},
    uapi::{OwnedFd, Pod},
};

pub struct BufFdIn {
    fd: Rc<OwnedFd>,
    ring: Rc<IoUring>,

    in_fd: VecDeque<Rc<OwnedFd>>,

    in_buf: Buf,
    in_left: usize,
    in_right: usize,
}

impl BufFdIn {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        Self {
            fd: fd.clone(),
            ring: ring.clone(),
            in_fd: Default::default(),
            in_buf: Buf::new(BUF_SIZE),
            in_left: 0,
            in_right: 0,
        }
    }

    pub async fn read_full<T: Pod + ?Sized>(&mut self, buf: &mut T) -> Result<(), BufFdError> {
        let bytes = unsafe { uapi::as_maybe_uninit_bytes_mut2(buf) };
        let mut offset = 0;
        while offset < bytes.len() {
            self.read_full_(bytes, &mut offset).await?;
        }
        Ok(())
    }

    async fn read_full_(
        &mut self,
        bytes: &mut [MaybeUninit<u8>],
        offset: &mut usize,
    ) -> Result<(), BufFdError> {
        let in_buf = uapi::as_maybe_uninit_bytes(&self.in_buf[..]);
        let num_bytes = (bytes.len() - *offset).min(self.in_right - self.in_left);
        if num_bytes > 0 {
            let left = self.in_left % BUF_SIZE;
            let right = (self.in_left + num_bytes) % BUF_SIZE;
            if left < right {
                bytes[*offset..*offset + num_bytes].copy_from_slice(&in_buf[left..right]);
            } else {
                bytes[*offset..*offset + (BUF_SIZE - left)].copy_from_slice(&in_buf[left..]);
                bytes[*offset + (BUF_SIZE - left)..*offset + num_bytes]
                    .copy_from_slice(&in_buf[..right]);
            }
            self.in_left += num_bytes;
            *offset += num_bytes;
        }
        if *offset == bytes.len() {
            return Ok(());
        }
        let left = self.in_left % BUF_SIZE;
        let right = self.in_right % BUF_SIZE;
        let mut iov = SmallVec::<[_; 2]>::new();
        if right < left {
            iov.push(self.in_buf.slice(right..left));
        } else {
            iov.push(self.in_buf.slice(right..));
            iov.push(self.in_buf.slice(..left));
        }
        match self.ring.recvmsg(&self.fd, &mut iov, &mut self.in_fd).await {
            Ok(0) => return Err(BufFdError::Closed),
            Ok(n) => self.in_right += n,
            Err(e) => return Err(BufFdError::Ring(e)),
        }
        if self.in_fd.len() > MAX_IN_FD {
            return Err(BufFdError::TooManyFds);
        }
        Ok(())
    }

    pub fn get_fd(&mut self) -> Result<Rc<OwnedFd>, BufFdError> {
        match self.in_fd.pop_front() {
            Some(f) => Ok(f),
            None => Err(BufFdError::NoFd),
        }
    }
}
