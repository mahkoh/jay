use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::org_kde_kwin_server_decoration::{OrgKdeKwinServerDecoration, MODE};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationError {
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error("Could not process a `request_mode` request")]
    RequestModeError(#[from] RequestModeError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OrgKdeKwinServerDecorationError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(ReleaseError, ClientError);
efrom!(ReleaseError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum RequestModeError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Mode {0} does not exist")]
    InvalidMode(u32),
}
efrom!(RequestModeError, ClientError);
efrom!(RequestModeError, ParseError, MsgParserError);

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

pub(super) struct RequestMode {
    pub mode: u32,
}
impl RequestParser<'_> for RequestMode {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            mode: parser.uint()?,
        })
    }
}
impl Debug for RequestMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "request_mode(mode: {})", self.mode)
    }
}

pub(super) struct Mode {
    pub obj: Rc<OrgKdeKwinServerDecoration>,
    pub mode: u32,
}
impl EventFormatter for Mode {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, MODE).uint(self.mode);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "mode(mode: {})", self.mode)
    }
}
