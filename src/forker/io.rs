use bincode::{Decode, Encode};
use std::mem;
use std::rc::Rc;

use crate::async_engine::AsyncFd;
use crate::utils::buffd::{BufFdIn, BufFdOut};
use crate::utils::vec_ext::VecExt;
use crate::ForkerError;
use jay_config::_private::bincode_ops;
use uapi::OwnedFd;

pub struct IoIn {
    incoming: BufFdIn,
    scratch: Vec<u8>,
}

impl IoIn {
    pub fn new(fd: AsyncFd) -> Self {
        Self {
            incoming: BufFdIn::new(fd),
            scratch: vec![],
        }
    }

    pub fn pop_fd(&mut self) -> Option<OwnedFd> {
        self.incoming.get_fd().ok()
    }

    pub async fn read_msg<T: Decode>(&mut self) -> Result<T, ForkerError> {
        let mut len = 0usize;
        if let Err(e) = self.incoming.read_full(&mut len).await {
            return Err(ForkerError::ReadFailed(e));
        }
        self.scratch.clear();
        self.scratch.reserve(len);
        let space = self.scratch.split_at_spare_mut_ext().1;
        if let Err(e) = self.incoming.read_full(&mut space[..len]).await {
            return Err(ForkerError::ReadFailed(e));
        }
        unsafe {
            self.scratch.set_len(len);
        }
        let res = bincode::decode_from_slice::<T, _>(&self.scratch, bincode_ops());
        match res {
            Ok((msg, _)) => Ok(msg),
            Err(e) => Err(ForkerError::DecodeFailed(e)),
        }
    }
}

pub struct IoOut {
    outgoing: BufFdOut,
    scratch: Vec<u8>,
    fds: Vec<Rc<OwnedFd>>,
}

impl IoOut {
    pub fn new(fd: AsyncFd) -> Self {
        Self {
            outgoing: BufFdOut::new(fd),
            scratch: vec![],
            fds: vec![],
        }
    }

    pub fn push_fd(&mut self, fd: Rc<OwnedFd>) {
        self.fds.push(fd);
    }

    pub async fn write_msg<T: Encode>(&mut self, msg: T) -> Result<(), ForkerError> {
        self.scratch.clear();
        self.scratch.extend_from_slice(uapi::as_bytes(&0usize));
        let res = bincode::encode_into_std_write(&msg, &mut self.scratch, bincode_ops());
        let len = match res {
            Ok(l) => l,
            Err(e) => return Err(ForkerError::EncodeFailed(e)),
        };
        self.scratch[..mem::size_of_val(&len)].copy_from_slice(uapi::as_bytes(&len));
        match self.outgoing.flush2(&self.scratch, &mut self.fds).await {
            Ok(()) => Ok(()),
            Err(e) => Err(ForkerError::WriteFailed(e)),
        }
    }
}
