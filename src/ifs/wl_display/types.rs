use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::globals::GlobalError;
use crate::ifs::wl_display::{WlDisplay, DELETE_ID, ERROR};
use crate::object::{Object, ObjectId, WL_DISPLAY_ID};
use crate::utils::buffd::{WlFormatter, WlParser, WlParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlDisplayError {
    #[error("Could not process a get_registry request")]
    GetRegistry(#[source] Box<GetRegistryError>),
    #[error("A client error occurred")]
    SyncError(#[source] Box<SyncError>),
}

efrom!(WlDisplayError, GetRegistry, GetRegistryError);
efrom!(WlDisplayError, SyncError, SyncError);

#[derive(Debug, Error)]
pub enum GetRegistryError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("An error occurred while processing globals")]
    GlobalError(#[source] Box<GlobalError>),
}

efrom!(GetRegistryError, ParseFailed, WlParserError);
efrom!(GetRegistryError, GlobalError, GlobalError);
efrom!(GetRegistryError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(SyncError, ParseFailed, WlParserError);
efrom!(SyncError, ClientError, ClientError);

pub(super) struct GetRegistry {
    pub registry: ObjectId,
}
impl RequestParser<'_> for GetRegistry {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self {
            registry: parser.object()?,
        })
    }
}
impl Debug for GetRegistry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_registry(registry: {})", self.registry)
    }
}

pub(super) struct Sync {
    pub callback: ObjectId,
}
impl RequestParser<'_> for Sync {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self {
            callback: parser.object()?,
        })
    }
}
impl Debug for Sync {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "sync(callback: {})", self.callback)
    }
}

pub(super) struct DeleteId {
    pub obj: Rc<WlDisplay>,
    pub id: ObjectId,
}
impl EventFormatter for DeleteId {
    fn format(self: Box<Self>, fmt: &mut WlFormatter<'_>) {
        fmt.header(WL_DISPLAY_ID, DELETE_ID).object(self.id);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for DeleteId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "delete_id(id: {})", self.id)
    }
}

pub(super) struct Error {
    pub obj: Rc<WlDisplay>,
    pub object_id: ObjectId,
    pub code: u32,
    pub message: String,
}
impl EventFormatter for Error {
    fn format(self: Box<Self>, fmt: &mut WlFormatter<'_>) {
        fmt.header(WL_DISPLAY_ID, ERROR)
            .object(self.object_id)
            .uint(self.code)
            .string(&self.message);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error(object_id: {}, code: {}, message: {:?})",
            self.object_id, self.code, self.message
        )
    }
}
