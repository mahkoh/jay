use std::ffi::CString;
use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_buffer::WlBufferId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::wl_drm::{AUTHENTICATED, CAPABILITIES, DEVICE, FORMAT, WlDrmObj};

#[derive(Debug, Error)]
pub enum WlDrmError {
    #[error("Could not process a `authenticate` request")]
    AuthenticateError(#[from] AuthenticateError),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `create_planar_buffer` request")]
    CreatePlanarBufferError(#[from] CreatePlanarBufferError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlDrmError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum AuthenticateError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(AuthenticateError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreateBufferError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreatePlanarBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreatePlanarBufferError, ParseError, MsgParserError);

pub(super) struct Authenticate {
    id: u32,
}
impl RequestParser<'_> for Authenticate {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self { id: parser.uint()? })
    }
}
impl Debug for Authenticate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "authenticate(id: {})", self.id)
    }
}

pub(super) struct CreateBuffer {
    pub id: WlBufferId,
    pub name: u32,
    pub width: i32,
    pub height: i32,
    pub stride: u32,
    pub format: u32,
}
impl RequestParser<'_> for CreateBuffer {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            name: parser.uint()?,
            width: parser.int()?,
            height: parser.int()?,
            stride: parser.uint()?,
            format: parser.uint()?,
        })
    }
}
impl Debug for CreateBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "create_buffer(id: {}, name: {}, width: {}, height: {}, stride: {}, format: {})",
            self.id, self.name, self.width, self.height, self.stride, self.format,
        )
    }
}

pub(super) struct CreatePlanarBuffer {
    id: WlBufferId,
    name: u32,
    width: i32,
    height: i32,
    format: u32,
    offset0: i32,
    stride0: i32,
    offset1: i32,
    stride1: i32,
    offset2: i32,
    stride2: i32,
}
impl RequestParser<'_> for CreatePlanarBuffer {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            name: parser.uint()?,
            width: parser.int()?,
            height: parser.int()?,
            format: parser.uint()?,
            offset0: parser.int()?,
            stride0: parser.int()?,
            offset1: parser.int()?,
            stride1: parser.int()?,
            offset2: parser.int()?,
            stride2: parser.int()?,
        })
    }
}
impl Debug for CreatePlanarBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_params(id: {}, name: {}, width: {}, height: {}, format: {}, offset0: {}, stride0: {}, offset1: {}, stride1: {}, offset2: {}, stride2: {})",
               self.id,
               self.name,
               self.width,
               self.height,
               self.format,
               self.offset0,
               self.stride0,
               self.offset1,
               self.stride1,
               self.offset2,
               self.stride2,
        )
    }
}

pub(super) struct Device {
    pub obj: Rc<WlDrmObj>,
    pub name: Rc<CString>,
}
impl EventFormatter for Device {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DEVICE).string(self.name.as_bytes());
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Device {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "device(name: {:?})", self.name)
    }
}

pub(super) struct Format {
    pub obj: Rc<WlDrmObj>,
    pub format: u32,
}
impl EventFormatter for Format {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, FORMAT)
            .uint(self.format);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Format {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "format(format: {})",
            self.format,
        )
    }
}

pub(super) struct Authenticated {
    pub obj: Rc<WlDrmObj>,
}
impl EventFormatter for Authenticated {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, AUTHENTICATED);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Authenticated {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "authenticated()",
        )
    }
}

pub(super) struct Capabilities {
    pub obj: Rc<WlDrmObj>,
    pub value: u32,
}
impl EventFormatter for Capabilities {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CAPABILITIES)
            .uint(self.value);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Capabilities {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "capabilities(value: {})",
            self.value,
        )
    }
}
