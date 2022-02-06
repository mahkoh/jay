use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_buffer::{WlBuffer, RELEASE};
use crate::object::Object;
use crate::render::RenderError;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use crate::ClientMemError;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlBufferError {
    #[error("The requested memory region is out of bounds for the pool")]
    OutOfBounds,
    #[error("The stride does not fit all pixels in a row")]
    StrideTooSmall,
    #[error("Could not handle a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not access the client memory")]
    ClientMemError(#[source] Box<ClientMemError>),
    #[error("GLES could not import the client image")]
    GlesError(#[source] Box<RenderError>),
}
efrom!(WlBufferError, ClientMemError);
efrom!(WlBufferError, GlesError, RenderError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);
