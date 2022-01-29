use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_buffer::WlBufferId;
use crate::ifs::zwp_linux_buffer_params_v1::{ZwpLinuxBufferParamsV1, CREATED, FAILED};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use crate::RenderError;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum ZwpLinuxBufferParamsV1Error {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `add` request")]
    AddError(#[from] AddError),
    #[error("Could not process a `create` request")]
    Create(#[from] CreateError),
    #[error("Could not process a `create_immed` request")]
    CreateImmed(#[from] CreateImmedError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpLinuxBufferParamsV1Error, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum AddError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("A buffer can contain at most 4 planes")]
    MaxPlane,
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The modifier {0} is not supported")]
    InvalidModifier(u64),
    #[error("The plane {0} was already set")]
    AlreadySet(u32),
}
efrom!(AddError, ClientError);
efrom!(AddError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum DoCreateError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The compositor has no render context attached")]
    NoRenderContext,
    #[error("The format {0} is not supported")]
    InvalidFormat(u32),
    #[error("Plane {0} was not set")]
    MissingPlane(usize),
    #[error("Could not import the buffer")]
    ImportError(#[from] RenderError),
}
efrom!(DoCreateError, ClientError);

#[derive(Debug, Error)]
pub enum CreateError {
    #[error("The params object has already been used")]
    AlreadyUsed,
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateError, ClientError, ClientError);
efrom!(CreateError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateImmedError {
    #[error("The params object has already been used")]
    AlreadyUsed,
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    DoCreateError(#[from] DoCreateError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateImmedError, ClientError, ClientError);
efrom!(CreateImmedError, ParseError, MsgParserError);

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

pub(super) struct Add {
    pub fd: OwnedFd,
    pub plane_idx: u32,
    pub offset: u32,
    pub stride: u32,
    pub modifier_hi: u32,
    pub modifier_lo: u32,
}
impl RequestParser<'_> for Add {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            fd: parser.fd()?,
            plane_idx: parser.uint()?,
            offset: parser.uint()?,
            stride: parser.uint()?,
            modifier_hi: parser.uint()?,
            modifier_lo: parser.uint()?,
        })
    }
}
impl Debug for Add {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "add(fd: {}, plane_idx: {}, offset: {}, stride: {}, modifier: {})",
            self.fd.raw(),
            self.plane_idx,
            self.offset,
            self.stride,
            (self.modifier_hi as u64) << 32 | self.modifier_lo as u64,
        )
    }
}

pub(super) struct Create {
    pub width: i32,
    pub height: i32,
    pub format: u32,
    pub flags: u32,
}
impl RequestParser<'_> for Create {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            width: parser.int()?,
            height: parser.int()?,
            format: parser.uint()?,
            flags: parser.uint()?,
        })
    }
}
impl Debug for Create {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "create(width: {}, height: {}, format: {}, flags: {})",
            self.width, self.height, self.format, self.flags,
        )
    }
}

pub(super) struct CreateImmed {
    pub buffer_id: WlBufferId,
    pub width: i32,
    pub height: i32,
    pub format: u32,
    pub flags: u32,
}
impl RequestParser<'_> for CreateImmed {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            buffer_id: parser.object()?,
            width: parser.int()?,
            height: parser.int()?,
            format: parser.uint()?,
            flags: parser.uint()?,
        })
    }
}
impl Debug for CreateImmed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "create_immed(buffer_id: {}, width: {}, height: {}, format: {}, flags: {})",
            self.buffer_id, self.width, self.height, self.format, self.flags,
        )
    }
}

pub(super) struct Created {
    pub obj: Rc<ZwpLinuxBufferParamsV1>,
    pub buffer: WlBufferId,
}
impl EventFormatter for Created {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CREATED).object(self.buffer);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Created {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "created(buffer: {})", self.buffer)
    }
}

pub(super) struct Failed {
    pub obj: Rc<ZwpLinuxBufferParamsV1>,
}
impl EventFormatter for Failed {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, FAILED);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Failed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed()")
    }
}
