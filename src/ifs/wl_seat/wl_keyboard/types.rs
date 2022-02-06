use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_seat::wl_keyboard::{
    WlKeyboard, ENTER, KEY, KEYMAP, LEAVE, MODIFIERS, REPEAT_INFO,
};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlKeyboardError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error("Could not create a keymap memfd")]
    KeymapMemfd(#[source] std::io::Error),
    #[error("Could not copy the keymap")]
    KeymapCopy(#[source] std::io::Error),
}
efrom!(WlKeyboardError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseError, MsgParserError);
efrom!(ReleaseError, ClientError, ClientError);
