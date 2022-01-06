use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_output::{WlOutputObj, DONE, GEOMETRY, MODE, SCALE};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlOutputError {
    #[error("Could not handle `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlOutputError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ClientError, ClientError);
efrom!(ReleaseError, ParseError, MsgParserError);

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

pub(super) struct Geometry {
    pub obj: Rc<WlOutputObj>,
    pub x: i32,
    pub y: i32,
    pub physical_width: i32,
    pub physical_height: i32,
    pub subpixel: i32,
    pub make: String,
    pub model: String,
    pub transform: i32,
}
impl EventFormatter for Geometry {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, GEOMETRY)
            .int(self.x)
            .int(self.y)
            .int(self.physical_width)
            .int(self.physical_height)
            .int(self.subpixel)
            .string(&self.make)
            .string(&self.model)
            .int(self.transform);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Geometry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "geometry(x: {}, y: {}, physical_width: {}, physical_height: {}, subpixel: {}, make: {}, model: {}, transform: {})",
        self.x, self.y, self.physical_width, self.physical_height, self.subpixel, self.make, self.model, self.transform)
    }
}

pub(super) struct Mode {
    pub obj: Rc<WlOutputObj>,
    pub flags: u32,
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
}
impl EventFormatter for Mode {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, MODE)
            .uint(self.flags)
            .int(self.width)
            .int(self.height)
            .int(self.refresh);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mode(flags: 0x{:x}, width: {}, height: {}, refresh: {})",
            self.flags, self.width, self.height, self.refresh
        )
    }
}

pub(super) struct Done {
    pub obj: Rc<WlOutputObj>,
}
impl EventFormatter for Done {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DONE);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Done {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "done()")
    }
}

pub(super) struct Scale {
    pub obj: Rc<WlOutputObj>,
    pub factor: i32,
}
impl EventFormatter for Scale {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, SCALE).int(self.factor);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Scale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "scale(factor: {})", self.factor)
    }
}
