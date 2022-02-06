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
