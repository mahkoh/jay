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

pub(super) struct Create {
    pub id: OrgKdeKwinServerDecorationId,
    pub surface: WlSurfaceId,
}
impl RequestParser<'_> for Create {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            surface: parser.object()?,
        })
    }
}
impl Debug for Create {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create(id: {}, surface: {})", self.id, self.surface)
    }
}

pub(super) struct DefaultMode {
    pub obj: Rc<OrgKdeKwinServerDecorationManager>,
    pub mode: u32,
}
impl EventFormatter for DefaultMode {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DEFAULT_MODE).uint(self.mode);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for DefaultMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "default_mode(mode: {})", self.mode)
    }
}
