use crate::dbus::types::{Bool, ObjectPath, Signature, Variant, FALSE, TRUE};
use crate::dbus::{DbusError, DbusType, DynamicType, Parser};
use bstr::ByteSlice;
use std::borrow::Cow;
use std::mem;
use std::rc::Rc;
use uapi::{OwnedFd, Pod};

impl<'a> Parser<'a> {
    pub fn new(buf: &'a [u8], fds: &'a [Rc<OwnedFd>]) -> Self {
        Self { buf, pos: 0, fds }
    }

    pub fn eof(&self) -> bool {
        self.pos == self.buf.len()
    }

    pub fn unmarshal<T: DbusType<'a>>(&mut self) -> Result<T, DbusError> {
        T::unmarshal(self)
    }

    pub fn align_to(&mut self, n: usize) -> Result<(), DbusError> {
        let new = self.pos + (self.pos.wrapping_neg() & (n - 1));
        if new > self.buf.len() {
            return Err(DbusError::UnexpectedEof);
        }
        self.pos = new;
        Ok(())
    }

    pub fn read_fd(&mut self) -> Result<Rc<OwnedFd>, DbusError> {
        let idx: u32 = self.read_pod()?;
        let idx = idx as usize;
        if idx >= self.fds.len() {
            return Err(DbusError::OobFds);
        }
        Ok(self.fds[idx].clone())
    }

    pub fn read_pod<'b, T: DbusType<'b> + Pod>(&mut self) -> Result<T, DbusError> {
        self.align_to(T::ALIGNMENT);
        match uapi::pod_read_init(&self.buf[self.pos..]) {
            Ok(v) => {
                self.pos += mem::size_of::<T>();
                Ok(v)
            }
            _ => Err(DbusError::UnexpectedEof),
        }
    }

    pub fn read_bool(&mut self) -> Result<Bool, DbusError> {
        let v: u32 = self.read_pod()?;
        match v {
            0 => Ok(FALSE),
            1 => Ok(TRUE),
            _ => Err(DbusError::InvalidBoolValue),
        }
    }

    pub fn read_object_path(&mut self) -> Result<ObjectPath<'a>, DbusError> {
        self.read_string().map(ObjectPath)
    }

    pub fn read_string(&mut self) -> Result<Cow<'a, str>, DbusError> {
        let len: u32 = self.read_pod()?;
        let s = self.read_string_(len as usize)?;
        Ok(Cow::Borrowed(s))
    }

    pub fn read_signature(&mut self) -> Result<Signature<'a>, DbusError> {
        let len: u8 = self.read_pod()?;
        let s = self.read_string_(len as usize)?;
        Ok(Signature(Cow::Borrowed(s)))
    }

    fn read_string_(&mut self, len: usize) -> Result<&'a str, DbusError> {
        if self.buf.len() - self.pos < len + 1 {
            return Err(DbusError::UnexpectedEof);
        }
        let s = &self.buf[self.pos..self.pos + len];
        self.pos += len + 1;
        match s.to_str() {
            Ok(s) => Ok(s),
            _ => Err(DbusError::InvalidUtf8),
        }
    }

    pub fn read_array<T: DbusType<'a>>(&mut self) -> Result<Cow<'a, [T]>, DbusError> {
        let len: u32 = self.read_pod()?;
        let len = len as usize;
        self.align_to(T::ALIGNMENT)?;
        if self.buf.len() - self.pos < len {
            return Err(DbusError::UnexpectedEof);
        }
        if T::IS_POD {
            if len % mem::size_of::<T>() != 0 {
                return Err(DbusError::PodArrayLength);
            }
            let slice = unsafe {
                std::slice::from_raw_parts(
                    self.buf[self.pos..].as_ptr() as *const T,
                    len / mem::size_of::<T>(),
                )
            };
            self.pos += len;
            Ok(Cow::Borrowed(slice))
        } else {
            let mut parser = Parser {
                buf: &self.buf[..self.pos + len],
                pos: self.pos,
                fds: self.fds,
            };
            self.pos += len;
            let mut res = vec![];
            while !parser.eof() {
                res.push(T::unmarshal(&mut parser)?);
            }
            Ok(Cow::Owned(res))
        }
    }

    pub fn read_variant(&mut self) -> Result<Variant<'a>, DbusError> {
        let sig = self.read_signature()?;
        let sig = sig.0.as_bytes();
        let (parser, rem) = DynamicType::from_signature(sig)?;
        if rem.len() > 0 {
            return Err(DbusError::TrailingVariantSignature);
        }
        parser.parse(self)
    }
}
