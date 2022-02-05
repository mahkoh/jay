use crate::fixed::Fixed;
use crate::globals::GlobalName;
use crate::object::ObjectId;
use crate::utils::buffd::BufFdIn;
use bstr::{BStr, ByteSlice};
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum MsgParserError {
    #[error("The message ended unexpectedly")]
    UnexpectedEof,
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

    pub fn global(&mut self) -> Result<GlobalName, MsgParserError> {
        self.int().map(|i| GlobalName::from_raw(i as u32))
    }

    #[allow(dead_code)]
    pub fn fixed(&mut self) -> Result<Fixed, MsgParserError> {
        self.int().map(Fixed)
    }

    pub fn bstr(&mut self) -> Result<&'b BStr, MsgParserError> {
        let len = self.uint()? as usize;
        if len == 0 {
            return Err(MsgParserError::EmptyString);
        }
        let cap = (len + 3) & !3;
        if cap > self.data.len() - self.pos {
            return Err(MsgParserError::UnexpectedEof);
        }
        let s = &self.data[self.pos..self.pos + len - 1];
        let s = s.as_bstr();
        self.pos += cap;
        Ok(s)
    }

    pub fn str(&mut self) -> Result<&'b str, MsgParserError> {
        match self.bstr()?.to_str() {
            Ok(s) => Ok(s),
            _ => Err(MsgParserError::NonUtf8),
        }
    }

    pub fn fd(&mut self) -> Result<OwnedFd, MsgParserError> {
        match self.buf.get_fd() {
            Ok(fd) => Ok(fd),
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
}
