use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_buffer::WlBufferId;
use crate::ifs::wl_drm::{WlDrm, AUTHENTICATED, CAPABILITIES, DEVICE, FORMAT};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::ffi::CString;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlDrmError {
    #[error("Could not process a `authenticate` request")]
    AuthenticateError(#[from] AuthenticateError),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `create_planar_buffer` request")]
    CreatePlanarBufferError(#[from] CreatePlanarBufferError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlDrmError, ClientError);

#[derive(Debug, Error)]
pub enum AuthenticateError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(AuthenticateError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreateBufferError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreatePlanarBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreatePlanarBufferError, ParseError, MsgParserError);
