use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::org_kde_kwin_server_decoration::OrgKdeKwinServerDecorationId;
use crate::ifs::org_kde_kwin_server_decoration_manager::{
    OrgKdeKwinServerDecorationManager, DEFAULT_MODE,
};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrgKdeKwinServerDecorationManagerError {
    #[error("Could not process a `create` request")]
    CreateError(#[from] CreateError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(
    OrgKdeKwinServerDecorationManagerError,
    ClientError,
    ClientError
);

#[derive(Debug, Error)]
pub enum CreateError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(CreateError, ClientError);
efrom!(CreateError, ParseError, MsgParserError);
