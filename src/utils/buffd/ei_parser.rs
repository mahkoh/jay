use {
    crate::{ei::ei_object::EiObjectId, utils::buffd::BufFdIn},
    std::{ptr, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

#[derive(Debug, Error)]
pub enum EiMsgParserError {
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

pub struct EiMsgParser<'a, 'b> {
    buf: &'a mut BufFdIn,
    pos: usize,
    data: &'b [u8],
}

impl<'a, 'b> EiMsgParser<'a, 'b> {
    pub fn new(buf: &'a mut BufFdIn, data: &'b [u32]) -> Self {
        Self {
            buf,
            pos: 0,
            data: uapi::as_bytes(data),
        }
    }

    pub fn int(&mut self) -> Result<i32, EiMsgParserError> {
        if self.data.len() - self.pos < 4 {
            return Err(EiMsgParserError::UnexpectedEof);
        }
        let res = unsafe { *(self.data.as_ptr().add(self.pos) as *const i32) };
        self.pos += 4;
        Ok(res)
    }

    pub fn uint(&mut self) -> Result<u32, EiMsgParserError> {
        self.int().map(|i| i as u32)
    }

    pub fn long(&mut self) -> Result<i64, EiMsgParserError> {
        if self.data.len() - self.pos < 8 {
            return Err(EiMsgParserError::UnexpectedEof);
        }
        let res = unsafe { ptr::read_unaligned(self.data.as_ptr().add(self.pos) as *const i64) };
        self.pos += 8;
        Ok(res)
    }

    pub fn ulong(&mut self) -> Result<u64, EiMsgParserError> {
        self.long().map(|i| i as u64)
    }

    pub fn object<T>(&mut self) -> Result<T, EiMsgParserError>
    where
        EiObjectId: Into<T>,
    {
        self.ulong().map(|i| EiObjectId::from_raw(i).into())
    }

    pub fn float(&mut self) -> Result<f32, EiMsgParserError> {
        Ok(f32::from_bits(self.uint()?))
    }

    pub fn optstr(&mut self) -> Result<Option<&'b str>, EiMsgParserError> {
        let len = self.uint()? as usize;
        if len == 0 {
            return Ok(None);
        }
        let cap = (len + 3) & !3;
        if cap > self.data.len() - self.pos {
            return Err(EiMsgParserError::UnexpectedEof);
        }
        let pos = self.pos;
        self.pos += cap;
        match std::str::from_utf8(&self.data[pos..pos + len - 1]) {
            Ok(s) => Ok(Some(s)),
            Err(_) => Err(EiMsgParserError::NonUtf8),
        }
    }

    pub fn str(&mut self) -> Result<&'b str, EiMsgParserError> {
        match self.optstr()? {
            Some(s) => Ok(s),
            _ => Err(EiMsgParserError::EmptyString),
        }
    }

    pub fn fd(&mut self) -> Result<Rc<OwnedFd>, EiMsgParserError> {
        match self.buf.get_fd() {
            Ok(fd) => Ok(fd),
            _ => Err(EiMsgParserError::MissingFd),
        }
    }

    pub fn eof(&self) -> Result<(), EiMsgParserError> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(EiMsgParserError::TrailingData)
        }
    }
}
