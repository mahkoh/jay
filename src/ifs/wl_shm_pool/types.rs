use crate::clientmem::ClientMemError;
use crate::objects::{ObjectError, ObjectId};
use crate::utils::buffd::{WlParser, WlParserError};
use crate::wl_client::{RequestParser, WlClientError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlShmPoolError {
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `resize` request")]
    ResizeError(#[from] ResizeError),
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(WlShmPoolError, ObjectError, ObjectError);
efrom!(WlShmPoolError, ClientError, WlClientError);
efrom!(WlShmPoolError, ClientMemError, ClientMemError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<WlParserError>),
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
}
efrom!(CreateBufferError, ParseError, WlParserError);
efrom!(CreateBufferError, ObjectError, ObjectError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<WlParserError>),
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
}
efrom!(DestroyError, ParseError, WlParserError);
efrom!(DestroyError, ObjectError, ObjectError);

#[derive(Debug, Error)]
pub enum ResizeError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<WlParserError>),
    #[error("Tried to shrink the pool")]
    CannotShrink,
    #[error("Requested size is negative")]
    NegativeSize,
    #[error(transparent)]
    ClientMemError(Box<ClientMemError>),
}
efrom!(ResizeError, ParseError, WlParserError);
efrom!(ResizeError, ClientMemError, ClientMemError);

pub(super) struct CreateBuffer {
    pub id: ObjectId,
    pub offset: i32,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub format: u32,
}
impl RequestParser<'_> for CreateBuffer {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(_parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
