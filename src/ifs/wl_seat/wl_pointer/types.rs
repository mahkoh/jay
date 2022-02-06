use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::fixed::Fixed;
use crate::ifs::wl_seat::wl_pointer::{
    WlPointer, AXIS, AXIS_DISCRETE, AXIS_SOURCE, AXIS_STOP, BUTTON, ENTER, FRAME, LEAVE, MOTION,
};
use crate::ifs::wl_surface::{WlSurfaceError, WlSurfaceId};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlPointerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `set_cursor` request")]
    SetCursorError(#[from] SetCursorError),
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
}
efrom!(WlPointerError, ClientError);

#[derive(Debug, Error)]
pub enum SetCursorError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(SetCursorError, ParseError, MsgParserError);
efrom!(SetCursorError, ClientError);
efrom!(SetCursorError, WlSurfaceError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseError, MsgParserError);
efrom!(ReleaseError, ClientError, ClientError);
