use crate::utils::ptr_ext::PtrExt;
use crate::xcon::wire_type::Message;
use crate::xcon::XconError;
use bstr::{BStr, ByteSlice};
use std::borrow::Cow;
use std::mem;
use std::rc::Rc;
use uapi::{OwnedFd, Pod};

pub struct Parser<'a> {
    pos: usize,
    buf: &'a [u8],
    fds_pos: usize,
    fds: Vec<Rc<OwnedFd>>,
}

impl<'a> Parser<'a> {
    pub fn new(buf: &'a [u8], fds: Vec<Rc<OwnedFd>>) -> Self {
        Self {
            buf,
            pos: 0,
            fds,
            fds_pos: 0,
        }
    }

    pub fn eof(&self) -> bool {
        self.pos == self.buf.len()
    }

    fn rem(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn unmarshal<T: Message<'a>>(&mut self) -> Result<T, XconError> {
        T::deserialize(self)
    }

    pub fn pad(&mut self, new: usize) -> Result<(), XconError> {
        if new > self.buf.len() - self.pos {
            return Err(XconError::UnexpectedEof);
        }
        self.pos += new;
        Ok(())
    }

    pub fn align(&mut self, n: usize) -> Result<(), XconError> {
        let new = self.pos + (self.pos.wrapping_neg() & (n - 1));
        if new > self.buf.len() {
            return Err(XconError::UnexpectedEof);
        }
        self.pos = new;
        Ok(())
    }

    pub fn read_fd(&mut self) -> Result<Rc<OwnedFd>, XconError> {
        if self.fds_pos >= self.fds.len() {
            return Err(XconError::NotEnoughFds);
        }
        self.fds_pos += 1;
        Ok(self.fds[self.fds_pos - 1].clone())
    }

    pub fn read_pod<T: Pod>(&mut self) -> Result<T, XconError> {
        match uapi::pod_read_init(&self.buf[self.pos..]) {
            Ok(v) => {
                self.pos += mem::size_of::<T>();
                Ok(v)
            }
            _ => Err(XconError::UnexpectedEof),
        }
    }

    pub fn read_list_slice<T: Message<'a> + Pod>(
        &mut self,
        n: Option<usize>,
    ) -> Result<&'a [T], XconError> {
        let n = match n {
            Some(n) => n,
            _ => self.rem() / mem::size_of::<T>(),
        };
        let len = mem::size_of::<T>() * n;
        if len > self.rem() {
            return Err(XconError::UnexpectedEof);
        }
        if self.buf[self.pos..].as_ptr() as usize & (mem::align_of::<T>() - 1) != 0 {
            return Err(XconError::UnalignedSlice);
        }
        let res =
            unsafe { std::slice::from_raw_parts(self.buf.as_ptr().add(self.pos) as *const T, n) };
        self.pos += len;
        Ok(res)
    }

    pub fn read_list<T: Message<'a> + Clone>(
        &mut self,
        n: Option<usize>,
    ) -> Result<Cow<'a, [T]>, XconError> {
        let mut res = vec![];
        if let Some(n) = n {
            for _ in 0..n {
                res.push(T::deserialize(self)?);
            }
        } else {
            while !self.eof() {
                res.push(T::deserialize(self)?);
            }
        }
        Ok(res.into())
    }

    pub fn read_bytes<const N: usize>(&mut self) -> Result<&'a [u8; N], XconError> {
        if N > self.rem() {
            return Err(XconError::UnexpectedEof);
        }
        let res = unsafe { self.buf.as_ptr().add(self.pos).cast::<[u8; N]>().deref() };
        self.pos += N;
        Ok(res)
    }

    pub fn read_slice(&mut self, n: usize) -> Result<&'a [u8], XconError> {
        if n > self.rem() {
            return Err(XconError::UnexpectedEof);
        }
        let res = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(res)
    }

    pub fn read_string(&mut self, n: usize) -> Result<&'a BStr, XconError> {
        if n > self.rem() {
            return Err(XconError::UnexpectedEof);
        }
        let res = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(res.as_bstr())
    }
}
