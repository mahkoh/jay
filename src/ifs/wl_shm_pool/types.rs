use crate::client::{ClientError, RequestParser};
use crate::clientmem::ClientMemError;
use crate::ifs::wl_buffer::{WlBufferError, WlBufferId};
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlShmPoolError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `resize` request")]
    ResizeError(#[from] ResizeError),
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(WlShmPoolError, ClientError);
efrom!(WlShmPoolError, ClientMemError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Format {0} is not supported")]
    InvalidFormat(u32),
    #[error("All parameters in a create_buffer request must be non-negative")]
    NegativeParameters,
    #[error(transparent)]
    WlBufferError(Box<WlBufferError>),
}
efrom!(CreateBufferError, ParseError, MsgParserError);
efrom!(CreateBufferError, ClientError);
efrom!(CreateBufferError, WlBufferError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseError, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum ResizeError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Tried to shrink the pool")]
    CannotShrink,
    #[error("Requested size is negative")]
    NegativeSize,
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(ResizeError, ParseError, MsgParserError);
efrom!(ResizeError, ClientMemError);

pub(super) struct CreateBuffer {
    pub id: WlBufferId,
    pub offset: i32,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub format: u32,
}
impl RequestParser<'_> for CreateBuffer {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            offset: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
            stride: parser.int()?,
            format: parser.uint()?,
        })
    }
}
impl Debug for CreateBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "create_buffer(id: {}, offset: {}, width: {}, height: {}, stride: {}, format: {})",
            self.id, self.offset, self.width, self.height, self.stride, self.format,
        )
    }
}

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()",)
    }
}

pub(super) struct Resize {
    pub size: i32,
}
impl RequestParser<'_> for Resize {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            size: parser.int()?,
        })
    }
}
impl Debug for Resize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "resize(size: {})", self.size,)
    }
}
