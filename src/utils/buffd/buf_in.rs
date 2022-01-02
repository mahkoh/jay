use crate::async_engine::AsyncFd;
use crate::utils::buffd::{BufFdError, BUF_SIZE, CMSG_BUF_SIZE, MAX_IN_FD};
use std::collections::VecDeque;
use std::mem::MaybeUninit;
use uapi::{c, Errno, OwnedFd, Pod};

pub struct BufFdIn {
    fd: AsyncFd,

    in_fd: VecDeque<OwnedFd>,

    in_buf: Box<[MaybeUninit<u8>; BUF_SIZE]>,
    in_cmsg_buf: Box<[MaybeUninit<u8>; CMSG_BUF_SIZE]>,
    in_left: usize,
    in_right: usize,
}

impl BufFdIn {
    pub fn new(fd: AsyncFd) -> Self {
        Self {
            fd,
            in_fd: Default::default(),
            in_buf: Box::new([MaybeUninit::uninit(); BUF_SIZE]),
            in_cmsg_buf: Box::new([MaybeUninit::uninit(); CMSG_BUF_SIZE]),
            in_left: 0,
            in_right: 0,
        }
    }

    pub async fn read_full<T: Pod + ?Sized>(&mut self, buf: &mut T) -> Result<(), BufFdError> {
        let bytes = unsafe { uapi::as_maybe_uninit_bytes_mut2(buf) };
        let mut offset = 0;
        while offset < bytes.len() {
            if self.read_full_(bytes, &mut offset)? {
                self.fd.readable().await?;
            }
        }
        Ok(())
    }

    fn read_full_(
        &mut self,
        bytes: &mut [MaybeUninit<u8>],
        offset: &mut usize,
    ) -> Result<bool, BufFdError> {
        let num_bytes = (bytes.len() - *offset).min(self.in_right - self.in_left);
        if num_bytes > 0 {
            let left = self.in_left % BUF_SIZE;
            let right = (self.in_left + num_bytes) % BUF_SIZE;
            if left < right {
                bytes[*offset..*offset + num_bytes].copy_from_slice(&self.in_buf[left..right]);
            } else {
                bytes[*offset..*offset + (BUF_SIZE - left)].copy_from_slice(&self.in_buf[left..]);
                bytes[*offset + (BUF_SIZE - left)..*offset + num_bytes]
                    .copy_from_slice(&self.in_buf[..right]);
            }
            self.in_left += num_bytes;
            *offset += num_bytes;
        }
        if *offset == bytes.len() {
            return Ok(false);
        }
        let left = self.in_left % BUF_SIZE;
        let right = self.in_right % BUF_SIZE;
        let mut iov = if right < left {
            [&mut self.in_buf[right..left], &mut []]
        } else {
            let (l, r) = self.in_buf.split_at_mut(right);
            [r, &mut l[..left]]
        };
        let mut hdr = uapi::MsghdrMut {
            iov: &mut iov[..],
            control: Some(&mut self.in_cmsg_buf[..]),
            name: uapi::sockaddr_none_mut(),
            flags: 0,
        };
        let (iov, _, mut cmsg) = match uapi::recvmsg(self.fd.raw(), &mut hdr, c::MSG_DONTWAIT) {
            Ok((iov, _, _)) if iov.is_empty() => return Err(BufFdError::Closed),
            Ok(v) => v,
            Err(Errno(c::EAGAIN)) => return Ok(true),
            Err(e) => return Err(BufFdError::Io(e.into())),
        };
        self.in_right += iov.len();
        while cmsg.len() > 0 {
            let (_, hdr, data) = match uapi::cmsg_read(&mut cmsg) {
                Ok(m) => m,
                Err(e) => return Err(BufFdError::Io(e.into())),
            };
            if (hdr.cmsg_level, hdr.cmsg_type) == (c::SOL_SOCKET, c::SCM_RIGHTS) {
                self.in_fd.extend(uapi::pod_iter(data).unwrap());
            }
        }
        if self.in_fd.len() > MAX_IN_FD {
            return Err(BufFdError::TooManyFds);
        }
        Ok(false)
    }

    pub fn get_fd(&mut self) -> Result<OwnedFd, BufFdError> {
        match self.in_fd.pop_front() {
            Some(f) => Ok(f),
            None => Err(BufFdError::NoFd),
        }
    }
}
