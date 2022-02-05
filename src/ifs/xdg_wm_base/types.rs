use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_surface::xdg_surface::{XdgSurfaceError, XdgSurfaceId};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::ifs::xdg_positioner::XdgPositionerId;
use crate::ifs::xdg_wm_base::{XdgWmBase, PING};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgWmBaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `create_positioner` request")]
    CreatePositionerError(#[from] CreatePositionerError),
    #[error("Could not process a `get_xdg_surface` request")]
    GetXdgSurfaceError(#[from] GetXdgSurfaceError),
    #[error("Could not process a `pong` request")]
    PongError(#[from] PongError),
}
efrom!(XdgWmBaseError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Tried to destroy xdg_wm_base object before destroying its surfaces")]
    DefunctSurfaces,
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseError, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum CreatePositionerError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreatePositionerError, ParseError, MsgParserError);
efrom!(CreatePositionerError, ClientError);

#[derive(Debug, Error)]
pub enum GetXdgSurfaceError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
}
efrom!(GetXdgSurfaceError, ParseError, MsgParserError);
efrom!(GetXdgSurfaceError, ClientError);
efrom!(GetXdgSurfaceError, XdgSurfaceError);

#[derive(Debug, Error)]
pub enum PongError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(PongError, ParseError, MsgParserError);

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()",)
    }
}

pub(super) struct CreatePositioner {
    pub id: XdgPositionerId,
}
impl RequestParser<'_> for CreatePositioner {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for CreatePositioner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_positioner(id: {})", self.id,)
    }
}

pub(super) struct GetXdgSurface {
    pub id: XdgSurfaceId,
    pub surface: WlSurfaceId,
}
impl RequestParser<'_> for GetXdgSurface {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            surface: parser.object()?,
        })
    }
}
impl Debug for GetXdgSurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "get_xdg_surface(id: {}, surface: {})",
            self.id, self.surface,
        )
    }
}

pub(super) struct Pong {
    pub serial: u32,
}
impl RequestParser<'_> for Pong {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            serial: parser.uint()?,
        })
    }
}
impl Debug for Pong {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "pong(serial: {})", self.serial,)
    }
}

pub(super) struct Ping {
    pub obj: Rc<XdgWmBase>,
    pub serial: u32,
}
impl EventFormatter for Ping {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, PING).uint(self.serial);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Ping {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ping(serial: {})", self.serial)
    }
}
