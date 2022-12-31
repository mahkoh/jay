use {
    bincode::{Decode, Encode},
    std::{mem, rc::Rc},
};

use {
    crate::{
        forker::ForkerError,
        io_uring::IoUring,
        utils::{
            buf::DynamicBuf,
            buffd::{BufFdIn, BufFdOut},
            vec_ext::VecExt,
        },
    },
    jay_config::_private::bincode_ops,
    uapi::OwnedFd,
};

pub struct IoIn {
    incoming: BufFdIn,
    scratch: Vec<u8>,
}

impl IoIn {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        Self {
            incoming: BufFdIn::new(fd, ring),
            scratch: vec![],
        }
    }

    pub fn pop_fd(&mut self) -> Option<Rc<OwnedFd>> {
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
    scratch: DynamicBuf,
    fds: Vec<Rc<OwnedFd>>,
}

impl IoOut {
    pub fn new(fd: &Rc<OwnedFd>, ring: &Rc<IoUring>) -> Self {
        Self {
            outgoing: BufFdOut::new(fd, ring),
            scratch: DynamicBuf::new(),
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
        let mut buf = self.scratch.borrow();
        match self
            .outgoing
            .flush2(buf.buf.clone(), mem::take(&mut self.fds))
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => Err(ForkerError::WriteFailed(e)),
        }
    }
}
