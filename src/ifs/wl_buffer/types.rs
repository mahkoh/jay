use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_buffer::{WlBuffer, RELEASE};
use crate::object::Object;
use crate::render::RenderError;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use crate::ClientMemError;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlBufferError {
    #[error("The requested memory region is out of bounds for the pool")]
    OutOfBounds,
    #[error("The stride does not fit all pixels in a row")]
    StrideTooSmall,
    #[error("Could not handle a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not access the client memory")]
    ClientMemError(#[source] Box<ClientMemError>),
    #[error("GLES could not import the client image")]
    GlesError(#[source] Box<RenderError>),
}
efrom!(WlBufferError, ClientMemError);
efrom!(WlBufferError, GlesError, RenderError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

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

pub(super) struct Release {
    pub obj: Rc<WlBuffer>,
}
impl EventFormatter for Release {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, RELEASE);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Release {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "release()")
    }
}
