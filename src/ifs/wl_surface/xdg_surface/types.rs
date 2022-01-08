use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopupId;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevelId;
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceId, CONFIGURE};
use crate::ifs::wl_surface::{SurfaceRole, WlSurfaceId};
use crate::ifs::xdg_positioner::XdgPositionerId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgSurfaceError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `get_toplevel` request")]
    GetToplevelError(#[from] GetToplevelError),
    #[error("Could not process `get_popup` request")]
    GetPopupError(#[from] GetPopupError),
    #[error("Could not process `set_window_geometry` request")]
    SetWindowGeometryError(#[from] SetWindowGeometryError),
    #[error("Could not process `ack_configure` request")]
    AckConfigureError(#[from] AckConfigureError),
    #[error("Surface {0} cannot be turned into a xdg_surface because it already has the role {}", .1.name())]
    IncompatibleRole(WlSurfaceId, SurfaceRole),
    #[error("Surface {0} cannot be turned into a xdg_surface because it already has an attached xdg_surface")]
    AlreadyAttached(WlSurfaceId),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Cannot destroy xdg_surface {0} because it's associated xdg_toplevel/popup is not yet destroyed")]
    RoleNotYetDestroyed(XdgSurfaceId),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum GetToplevelError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a different role")]
    IncompatibleRole,
    #[error("The surface already has an assigned xdg_toplevel")]
    AlreadyConstructed,
}
efrom!(GetToplevelError, ParseFailed, MsgParserError);
efrom!(GetToplevelError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum GetPopupError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a different role")]
    IncompatibleRole,
    #[error("The surface already has an assigned xdg_popup")]
    AlreadyConstructed,
}
efrom!(GetPopupError, ParseFailed, MsgParserError);
efrom!(GetPopupError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetWindowGeometryError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Tried no set a non-positive width/height")]
    NonPositiveWidthHeight,
}
efrom!(SetWindowGeometryError, ParseFailed, MsgParserError);
efrom!(SetWindowGeometryError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum AckConfigureError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(AckConfigureError, ParseFailed, MsgParserError);
efrom!(AckConfigureError, ClientError, ClientError);

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

pub(super) struct GetToplevel {
    pub id: XdgToplevelId,
}
impl RequestParser<'_> for GetToplevel {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for GetToplevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_toplevel(id: {})", self.id)
    }
}

pub(super) struct GetPopup {
    pub id: XdgPopupId,
    pub parent: XdgSurfaceId,
    pub positioner: XdgPositionerId,
}
impl RequestParser<'_> for GetPopup {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            parent: parser.object()?,
            positioner: parser.object()?,
        })
    }
}
impl Debug for GetPopup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "get_popup(id: {}, parent: {}, positioner: {})",
            self.id, self.parent, self.positioner
        )
    }
}

pub(super) struct SetWindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for SetWindowGeometry {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for SetWindowGeometry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_window_geometry(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height
        )
    }
}

pub(super) struct AckConfigure {
    pub serial: u32,
}
impl RequestParser<'_> for AckConfigure {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            serial: parser.uint()?,
        })
    }
}
impl Debug for AckConfigure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ack_configure(serial: {})", self.serial)
    }
}

pub(super) struct Configure {
    pub obj: Rc<XdgSurface>,
    pub serial: u32,
}
impl EventFormatter for Configure {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CONFIGURE).uint(self.serial);
    }

    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Configure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "configure(serial: {})", self.serial)
    }
}
