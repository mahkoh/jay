use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1Id;
use crate::ifs::zwp_linux_dmabuf_v1::{ZwpLinuxDmabufV1Obj, FORMAT, MODIFIER};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZwpLinuxDmabufV1Error {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `create_params` request")]
    CreateParamsError(#[from] CreateParamsError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpLinuxDmabufV1Error, ClientError);

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
pub enum CreateParamsError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateParamsError, ClientError);
efrom!(CreateParamsError, ParseError, MsgParserError);

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

pub(super) struct CreateParams {
    pub params_id: ZwpLinuxBufferParamsV1Id,
}
impl RequestParser<'_> for CreateParams {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            params_id: parser.object()?,
        })
    }
}
impl Debug for CreateParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_params(params_id: {})", self.params_id)
    }
}

pub(super) struct Format {
    pub obj: Rc<ZwpLinuxDmabufV1Obj>,
    pub format: u32,
}
impl EventFormatter for Format {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, FORMAT).uint(self.format);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Format {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "format(format: {})", self.format)
    }
}

pub(super) struct Modifier {
    pub obj: Rc<ZwpLinuxDmabufV1Obj>,
    pub format: u32,
    pub modifier: u64,
}
impl EventFormatter for Modifier {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, MODIFIER)
            .uint(self.format)
            .uint((self.modifier >> 32) as u32)
            .uint(self.modifier as u32);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Modifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "modifiers(format: {}, modifier: {})",
            self.format, self.modifier
        )
    }
}
