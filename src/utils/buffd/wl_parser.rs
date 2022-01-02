use crate::globals::GlobalName;
use crate::objects::ObjectId;
use crate::utils::buffd::BufFdIn;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlParserError {
    #[error("The message ended unexpectedly")]
    UnexpectedEof,
    #[error("The message contained a non-utf8 string")]
    NonUtf8,
    #[error("The message contained a string of size 0")]
    EmptyString,
    #[error("Message is missing a required file descriptor")]
    MissingFd,
    #[error("There is trailing data after the message")]
    TrailingData,
}

pub struct WlParser<'a, 'b> {
    buf: &'a mut BufFdIn,
    pos: usize,
    data: &'b [u8],
}

impl<'a, 'b> WlParser<'a, 'b> {
    pub fn new(buf: &'a mut BufFdIn, data: &'b [u32]) -> Self {
        Self {
            buf,
            pos: 0,
            data: unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) },
        }
    }

    pub fn int(&mut self) -> Result<i32, WlParserError> {
        if self.data.len() - self.pos < 4 {
            return Err(WlParserError::UnexpectedEof);
        }
        let res = unsafe { *(self.data.as_ptr().add(self.pos) as *const i32) };
        self.pos += 4;
        Ok(res)
    }

    pub fn uint(&mut self) -> Result<u32, WlParserError> {
        self.int().map(|i| i as u32)
    }

    pub fn object(&mut self) -> Result<ObjectId, WlParserError> {
        self.int().map(|i| ObjectId::from_raw(i as u32))
    }

    pub fn global(&mut self) -> Result<GlobalName, WlParserError> {
        self.int().map(|i| GlobalName::from_raw(i as u32))
    }

    pub fn fixed(&mut self) -> Result<f64, WlParserError> {
        self.int().map(|i| i as f64 / 256.0)
    }

    pub fn string(&mut self) -> Result<&'b str, WlParserError> {
        let len = self.uint()? as usize;
        if len == 0 {
            return Err(WlParserError::EmptyString);
        }
        let cap = (len + 3) & !3;
        if cap > self.data.len() - self.pos {
            return Err(WlParserError::UnexpectedEof);
        }
        let s = &self.data[self.pos..self.pos + len - 1];
        let s = match std::str::from_utf8(s) {
            Ok(s) => s,
            _ => return Err(WlParserError::NonUtf8),
        };
        self.pos += cap;
        Ok(s)
    }

    pub fn fd(&mut self) -> Result<OwnedFd, WlParserError> {
        match self.buf.get_fd() {
            Ok(fd) => Ok(fd),
            _ => Err(WlParserError::MissingFd),
        }
    }

    pub fn eof(&self) -> Result<(), WlParserError> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(WlParserError::TrailingData)
        }
    }
}
