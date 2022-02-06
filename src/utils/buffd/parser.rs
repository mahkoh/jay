use std::{mem, ptr};
use std::rc::Rc;
use crate::fixed::Fixed;
use crate::globals::GlobalName;
use crate::object::ObjectId;
use crate::utils::buffd::BufFdIn;
use bstr::{BStr, ByteSlice};
use thiserror::Error;
use uapi::{OwnedFd, Pod};

#[derive(Debug, Error)]
pub enum MsgParserError {
    #[error("The message ended unexpectedly")]
    UnexpectedEof,
    #[error("The binary array contains more than the required number of bytes")]
    BinaryArrayTooLarge,
    #[error("The size of the binary array is not a multiple of the element size")]
    BinaryArraySize,
    #[error("The message contained a string of size 0")]
    EmptyString,
    #[error("Message is missing a required file descriptor")]
    MissingFd,
    #[error("There is trailing data after the message")]
    TrailingData,
    #[error("String is not UTF-8")]
    NonUtf8,
}

pub struct MsgParser<'a, 'b> {
    buf: &'a mut BufFdIn,
    pos: usize,
    data: &'b [u8],
}

impl<'a, 'b> MsgParser<'a, 'b> {
    pub fn new(buf: &'a mut BufFdIn, data: &'b [u32]) -> Self {
        Self {
            buf,
            pos: 0,
            data: unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) },
        }
    }

    pub fn int(&mut self) -> Result<i32, MsgParserError> {
        if self.data.len() - self.pos < 4 {
            return Err(MsgParserError::UnexpectedEof);
        }
        let res = unsafe { *(self.data.as_ptr().add(self.pos) as *const i32) };
        self.pos += 4;
        Ok(res)
    }

    pub fn uint(&mut self) -> Result<u32, MsgParserError> {
        self.int().map(|i| i as u32)
    }

    pub fn object<T>(&mut self) -> Result<T, MsgParserError>
    where
        ObjectId: Into<T>,
    {
        self.int().map(|i| ObjectId::from_raw(i as u32).into())
    }

    #[allow(dead_code)]
    pub fn global(&mut self) -> Result<GlobalName, MsgParserError> {
        self.int().map(|i| GlobalName::from_raw(i as u32))
    }

    #[allow(dead_code)]
    pub fn fixed(&mut self) -> Result<Fixed, MsgParserError> {
        self.int().map(Fixed)
    }

    pub fn bstr(&mut self) -> Result<&'b BStr, MsgParserError> {
        let s = self.array()?;
        if s.len() == 0 {
            return Err(MsgParserError::EmptyString);
        }
        Ok(s[..s.len()-1].as_bstr())
    }

    pub fn str(&mut self) -> Result<&'b str, MsgParserError> {
        match self.bstr()?.to_str() {
            Ok(s) => Ok(s),
            _ => Err(MsgParserError::NonUtf8),
        }
    }

    pub fn fd(&mut self) -> Result<Rc<OwnedFd>, MsgParserError> {
        match self.buf.get_fd() {
            Ok(fd) => Ok(Rc::new(fd)),
            _ => Err(MsgParserError::MissingFd),
        }
    }

    pub fn eof(&self) -> Result<(), MsgParserError> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(MsgParserError::TrailingData)
        }
    }

    pub fn array(&mut self) -> Result<&'b [u8], MsgParserError> {
        let len = self.uint()? as usize;
        let cap = (len + 3) & !3;
        if cap > self.data.len() - self.pos {
            return Err(MsgParserError::UnexpectedEof);
        }
        let pos = self.pos;
        self.pos += cap;
        Ok(&self.data[pos..pos + len])
    }

    pub fn binary<T: Pod>(&mut self) -> Result<T, MsgParserError> {
        let array = self.array()?;
        if array.len() < mem::size_of::<T>() {
            return Err(MsgParserError::UnexpectedEof);
        }
        if array.len() > mem::size_of::<T>() {
            return Err(MsgParserError::BinaryArrayTooLarge);
        }
        unsafe {
            Ok(ptr::read_unaligned(array.as_ptr() as _))
        }
    }

    pub fn binary_array<T: Pod>(&mut self) -> Result<&'b [T], MsgParserError> {
        if std::mem::align_of::<T>() > 4 {
            panic!("Alignment of binary array element is too large");
        };
        if std::mem::size_of::<T>() == 0 {
            panic!("Size of binary array element is 0");
        };
        let array = self.array()?;
        if array.len() % mem::size_of::<T>() != 0 {
            return Err(MsgParserError::BinaryArraySize);
        }
        unsafe {
            Ok(std::slice::from_raw_parts(array.as_ptr() as _, array.len() / mem::size_of::<T>()))
        }
    }
}
