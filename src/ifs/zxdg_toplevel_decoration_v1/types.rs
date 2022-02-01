use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::zxdg_toplevel_decoration_v1::{ZxdgToplevelDecorationV1, CONFIGURE};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZxdgToplevelDecorationV1Error {
    #[error("Could not process a `destroy` request")]
    DestoryError(#[from] DestroyError),
    #[error("Could not process a `set_mode` request")]
    SetModeError(#[from] SetModeError),
    #[error("Could not process a `unset_mode` request")]
    UnsetModeError(#[from] UnsetModeError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetModeError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(SetModeError, MsgParserError);

#[derive(Debug, Error)]
pub enum UnsetModeError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(UnsetModeError, MsgParserError);

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()")
    }
}

pub(super) struct SetMode {
    pub mode: u32,
}
impl RequestParser<'_> for SetMode {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            mode: parser.uint()?,
        })
    }
}
impl Debug for SetMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_mode(mode: {})", self.mode)
    }
}

pub(super) struct UnsetMode;
impl RequestParser<'_> for UnsetMode {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for UnsetMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unset_mode()")
    }
}

pub(super) struct Configure {
    pub obj: Rc<ZxdgToplevelDecorationV1>,
    pub mode: u32,
}
impl EventFormatter for Configure {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CONFIGURE).uint(self.mode);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Configure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "configure(mode: {})", self.mode)
    }
}
