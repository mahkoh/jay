use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::fixed::Fixed;
use crate::ifs::wl_data_device::{WlDataDevice, DATA_OFFER, DROP, ENTER, LEAVE, MOTION, SELECTION};
use crate::ifs::wl_data_source::WlDataSourceId;
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::wl_data_offer::WlDataOfferId;

#[derive(Debug, Error)]
pub enum WlDataDeviceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `start_drag` request")]
    StartDragError(#[from] StartDragError),
    #[error("Could not process `set_selection` request")]
    SetSelectionError(#[from] SetSelectionError),
    #[error("Could not process `release` request")]
    ReleaseError(#[from] ReleaseError),
}
efrom!(WlDataDeviceError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum StartDragError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(StartDragError, ParseFailed, MsgParserError);
efrom!(StartDragError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetSelectionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetSelectionError, ParseFailed, MsgParserError);
efrom!(SetSelectionError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseFailed, MsgParserError);
efrom!(ReleaseError, ClientError, ClientError);

pub(super) struct StartDrag {
    pub source: WlDataSourceId,
    pub origin: WlSurfaceId,
    pub icon: WlSurfaceId,
    pub serial: u32,
}
impl RequestParser<'_> for StartDrag {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            source: parser.object()?,
            origin: parser.object()?,
            icon: parser.object()?,
            serial: parser.uint()?,
        })
    }
}
impl Debug for StartDrag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "start_drag(source: {}, origin: {}, icon: {}, serial: {})",
            self.source, self.origin, self.icon, self.serial
        )
    }
}

pub(super) struct SetSelection {
    pub source: WlDataSourceId,
    pub serial: u32,
}
impl RequestParser<'_> for SetSelection {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            source: parser.object()?,
            serial: parser.uint()?,
        })
    }
}
impl Debug for SetSelection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_selection(source: {}, serial: {})",
            self.source, self.serial,
        )
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

pub(super) struct DataOffer {
    pub obj: Rc<WlDataDevice>,
    pub id: WlDataOfferId,
}
impl EventFormatter for DataOffer {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DATA_OFFER).object(self.id);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for DataOffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "data_offer(id: {})", self.id)
    }
}

pub(super) struct Enter {
    pub obj: Rc<WlDataDevice>,
    pub serial: u32,
    pub surface: WlSurfaceId,
    pub x: Fixed,
    pub y: Fixed,
    pub id: WlDataOfferId,
}
impl EventFormatter for Enter {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, ENTER)
            .uint(self.serial)
            .object(self.surface)
            .fixed(self.x)
            .fixed(self.y)
            .object(self.id);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Enter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "enter(serial: {}, surface: {}, x: {}, y: {}, id: {})",
            self.serial, self.surface, self.x, self.y, self.id
        )
    }
}

pub(super) struct Leave {
    pub obj: Rc<WlDataDevice>,
}
impl EventFormatter for Leave {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, LEAVE);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Leave {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "leave()")
    }
}

pub(super) struct Motion {
    pub obj: Rc<WlDataDevice>,
    pub time: u32,
    pub x: Fixed,
    pub y: Fixed,
}
impl EventFormatter for Motion {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, MOTION)
            .uint(self.time)
            .fixed(self.x)
            .fixed(self.y);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Motion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "motion(time: {}, x: {}, y: {})",
            self.time, self.x, self.y
        )
    }
}

pub(super) struct Drop {
    pub obj: Rc<WlDataDevice>,
}
impl EventFormatter for Drop {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DROP);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Drop {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "drop()")
    }
}

pub(super) struct Selection {
    pub obj: Rc<WlDataDevice>,
    pub id: WlDataOfferId,
}
impl EventFormatter for Selection {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, SELECTION).object(self.id);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Selection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "selection(id: {})", self.id)
    }
}
