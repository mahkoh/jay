use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::format::Format;
use crate::ifs::wl_shm::{WlShm, FORMAT};
use crate::ifs::wl_shm_pool::{WlShmPoolError, WlShmPoolId};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlShmError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `create_pool` request")]
    CreatePoolError(#[from] CreatePoolError),
}
efrom!(WlShmError, ClientError);

#[derive(Debug, Error)]
pub enum CreatePoolError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("The passed size is negative")]
    NegativeSize,
    #[error(transparent)]
    WlShmPoolError(Box<WlShmPoolError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreatePoolError, ParseError, MsgParserError);
efrom!(CreatePoolError, WlShmPoolError);
efrom!(CreatePoolError, ClientError);

pub(super) struct CreatePool {
    pub id: WlShmPoolId,
    pub fd: OwnedFd,
    pub size: i32,
}
impl RequestParser<'_> for CreatePool {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            fd: parser.fd()?,
            size: parser.int()?,
        })
    }
}
impl Debug for CreatePool {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "create_pool(id: {}, fd: {}, size: {})",
            self.id,
            self.fd.raw(),
            self.size
        )
    }
}

pub(super) struct FormatE {
    pub obj: Rc<WlShm>,
    pub format: &'static Format,
}
impl EventFormatter for FormatE {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, FORMAT)
            .uint(self.format.wl_id.unwrap_or(self.format.drm));
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for FormatE {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "format(format: \"{}\" (0x{:x}))",
            self.format.name,
            self.format.wl_id.unwrap_or(self.format.drm),
        )
    }
}
