use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_seat::wl_keyboard::WlKeyboardId;
use crate::ifs::wl_seat::wl_pointer::WlPointerId;
use crate::ifs::wl_seat::wl_touch::WlTouchId;
use crate::ifs::wl_seat::{WlSeatObj, CAPABILITIES, NAME};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlSeatError {
    #[error("Could not handle `get_pointer` request")]
    GetPointerError(#[from] GetPointerError),
    #[error("Could not handle `get_keyboard` request")]
    GetKeyboardError(#[from] GetKeyboardError),
    #[error("Could not handle `get_touch` request")]
    GetTouchError(#[from] GetTouchError),
    #[error("Could not handle `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlSeatError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum GetPointerError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetPointerError, ClientError, ClientError);
efrom!(GetPointerError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum GetKeyboardError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetKeyboardError, ClientError, ClientError);
efrom!(GetKeyboardError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum GetTouchError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetTouchError, ClientError, ClientError);
efrom!(GetTouchError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ClientError, ClientError);
efrom!(ReleaseError, ParseError, MsgParserError);

pub(super) struct GetPointer {
    pub id: WlPointerId,
}
impl RequestParser<'_> for GetPointer {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for GetPointer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_pointer(id: {})", self.id)
    }
}

pub(super) struct GetKeyboard {
    pub id: WlKeyboardId,
}
impl RequestParser<'_> for GetKeyboard {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for GetKeyboard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_keyboard(id: {})", self.id)
    }
}

pub(super) struct GetTouch {
    pub id: WlTouchId,
}
impl RequestParser<'_> for GetTouch {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for GetTouch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_touch(id: {})", self.id)
    }
}

pub(super) struct Release;
impl RequestParser<'_> for Release {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Release {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "release()")
    }
}

pub(super) struct Capabilities {
    pub obj: Rc<WlSeatObj>,
    pub capabilities: u32,
}
impl EventFormatter for Capabilities {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CAPABILITIES)
            .uint(self.capabilities);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Capabilities {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "capabilities(capabilities: {})", self.capabilities)
    }
}

pub(super) struct Name {
    pub obj: Rc<WlSeatObj>,
    pub name: String,
}
impl EventFormatter for Name {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, NAME).string(&self.name);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Name {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "name(name: {})", self.name)
    }
}
