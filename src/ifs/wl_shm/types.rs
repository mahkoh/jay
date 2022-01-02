use crate::ifs::wl_shm::{Format, WlShmObj, FORMAT};
use crate::ifs::wl_shm_pool::WlShmPoolError;
use crate::objects::{Object, ObjectError, ObjectId};
use crate::utils::buffd::{WlFormatter, WlParser, WlParserError};
use crate::wl_client::{EventFormatter, RequestParser, WlClientError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlShmError {
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
    #[error("Could not process a `create_pool` request")]
    CreatePoolError(#[from] CreatePoolError),
}
efrom!(WlShmError, ObjectError, ObjectError);
efrom!(WlShmError, ClientError, WlClientError);

#[derive(Debug, Error)]
pub enum CreatePoolError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<WlParserError>),
    #[error("The passed size is negative")]
    NegativeSize,
    #[error(transparent)]
    WlShmPoolError(Box<WlShmPoolError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
}
efrom!(CreatePoolError, ParseError, WlParserError);
efrom!(CreatePoolError, WlShmPoolError, WlShmPoolError);
efrom!(CreatePoolError, ClientError, WlClientError);

pub(super) struct CreatePool {
    pub id: ObjectId,
    pub fd: OwnedFd,
    pub size: i32,
}
impl RequestParser<'_> for CreatePool {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    pub obj: Rc<WlShmObj>,
    pub format: Format,
}
impl EventFormatter for FormatE {
    fn format(self: Box<Self>, fmt: &mut WlFormatter<'_>) {
        fmt.header(self.obj.id, FORMAT).uint(self.format.uint());
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for FormatE {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "format(format: {:?})", self.format)
    }
}
