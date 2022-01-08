use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::fixed::Fixed;
use crate::ifs::wl_seat::wl_pointer::{
    WlPointer, AXIS, AXIS_DISCRETE, AXIS_SOURCE, AXIS_STOP, BUTTON, ENTER, FRAME, LEAVE, MOTION,
};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlPointerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `set_cursor` request")]
    SetCursorError(#[from] SetCursorError),
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
}
efrom!(WlPointerError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetCursorError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetCursorError, ParseError, MsgParserError);
efrom!(SetCursorError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseError, MsgParserError);
efrom!(ReleaseError, ClientError, ClientError);

pub(super) struct SetCursor {
    pub serial: u32,
    pub surface: WlSurfaceId,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
}
impl RequestParser<'_> for SetCursor {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            serial: parser.uint()?,
            surface: parser.object()?,
            hotspot_x: parser.int()?,
            hotspot_y: parser.int()?,
        })
    }
}
impl Debug for SetCursor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_cursor(serial: {}, surface: {}, hotspot_x: {}, hotspot_y: {})",
            self.serial, self.surface, self.hotspot_x, self.hotspot_y
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
        write!(f, "destroy()",)
    }
}

pub(super) struct Enter {
    pub obj: Rc<WlPointer>,
    pub serial: u32,
    pub surface: WlSurfaceId,
    pub surface_x: Fixed,
    pub surface_y: Fixed,
}
impl EventFormatter for Enter {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, ENTER)
            .uint(self.serial)
            .object(self.surface)
            .fixed(self.surface_x)
            .fixed(self.surface_y);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Enter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "enter(serial: {}, surface: {}, surface_x: {}, surface_y: {})",
            self.serial, self.surface, self.surface_x, self.surface_y
        )
    }
}

pub(super) struct Leave {
    pub obj: Rc<WlPointer>,
    pub serial: u32,
    pub surface: WlSurfaceId,
}
impl EventFormatter for Leave {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, LEAVE)
            .uint(self.serial)
            .object(self.surface);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Leave {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "leave(serial: {}, surface: {})",
            self.serial, self.surface
        )
    }
}

pub(super) struct Motion {
    pub obj: Rc<WlPointer>,
    pub time: u32,
    pub surface_x: Fixed,
    pub surface_y: Fixed,
}
impl EventFormatter for Motion {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, MOTION)
            .uint(self.time)
            .fixed(self.surface_x)
            .fixed(self.surface_y);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Motion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "motion(time: {}, surface_x: {}, surface_y: {})",
            self.time, self.surface_x, self.surface_y
        )
    }
}

pub(super) struct Button {
    pub obj: Rc<WlPointer>,
    pub serial: u32,
    pub time: u32,
    pub button: u32,
    pub state: u32,
}
impl EventFormatter for Button {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, BUTTON)
            .uint(self.serial)
            .uint(self.time)
            .uint(self.button)
            .uint(self.state);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Button {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "button(serial: {}, time: {}, button: 0x{:x}, state: {})",
            self.serial, self.time, self.button, self.state
        )
    }
}

pub(super) struct Axis {
    pub obj: Rc<WlPointer>,
    pub time: u32,
    pub axis: u32,
    pub value: Fixed,
}
impl EventFormatter for Axis {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, AXIS)
            .uint(self.time)
            .uint(self.axis)
            .fixed(self.value);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Axis {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "axis(time: {}, axis: {}, value: {:?})",
            self.time, self.axis, self.value
        )
    }
}

pub(super) struct Frame {
    pub obj: Rc<WlPointer>,
}
impl EventFormatter for Frame {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, FRAME);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "frame()")
    }
}

pub(super) struct AxisSource {
    pub obj: Rc<WlPointer>,
    pub axis_source: u32,
}
impl EventFormatter for AxisSource {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, AXIS_SOURCE).uint(self.axis_source);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for AxisSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "axis_source(axis_source: {})", self.axis_source)
    }
}

pub(super) struct AxisStop {
    pub obj: Rc<WlPointer>,
    pub time: u32,
    pub axis: u32,
}
impl EventFormatter for AxisStop {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, AXIS_STOP)
            .uint(self.time)
            .uint(self.axis);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for AxisStop {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "axis_stop(time: {}, axis: {})", self.time, self.axis)
    }
}

pub(super) struct AxisDiscrete {
    pub obj: Rc<WlPointer>,
    pub axis: u32,
    pub discrete: i32,
}
impl EventFormatter for AxisDiscrete {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, AXIS_DISCRETE)
            .uint(self.axis)
            .int(self.discrete);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for AxisDiscrete {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "axis_discrete(axis: {}, discrete: {})",
            self.axis, self.discrete
        )
    }
}
