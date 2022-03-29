use crate::async_engine::AsyncError;
use crate::client::ClientId;
use crate::object::{Interface, ObjectId};
use crate::utils::buffd::{BufFdError, MsgParserError};
use crate::wire::WlDisplayId;
use std::error::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("An error occurred in the async engine")]
    Async(#[from] AsyncError),
    #[error("An error occurred reading from/writing to the client")]
    Io(#[from] BufFdError),
    #[error("An error occurred while processing a request")]
    RequestError(#[source] Box<ClientError>),
    #[error("Client tried to invoke a non-existent method")]
    InvalidMethod,
    #[error("Client tried to access non-existent object {0}")]
    InvalidObject(ObjectId),
    #[error("The message size is < 8")]
    MessageSizeTooSmall,
    #[error("The size of the message is not a multiple of 4")]
    UnalignedMessage,
    #[error("The requested client {0} does not exist")]
    ClientDoesNotExist(ClientId),
    #[error("Cannot parse the message")]
    ParserError(#[source] Box<MsgParserError>),
    #[error("Server tried to allocate more than 0x1_00_00_00 ids")]
    TooManyIds,
    #[error("The server object id is out of bounds")]
    ServerIdOutOfBounds,
    #[error("The object id is unknown")]
    UnknownId,
    #[error("The id is already in use")]
    IdAlreadyInUse,
    #[error("The client object id is out of bounds")]
    ClientIdOutOfBounds,
    #[error("Object {0} is not a display")]
    NotADisplay(WlDisplayId),
    #[error(transparent)]
    ObjectError(ObjectError),
    #[error(transparent)]
    LookupError(LookupError),
    #[error("Could not add object {0} to the client")]
    AddObjectError(ObjectId, #[source] Box<ClientError>),
}
efrom!(ClientError, ParserError, MsgParserError);

impl ClientError {
    pub fn peer_closed(&self) -> bool {
        matches!(self, ClientError::Io(BufFdError::Closed))
    }
}

#[derive(Debug, Error)]
#[error("An error occurred in a `{}`", .interface.name())]
pub struct ObjectError {
    pub interface: Interface,
    #[source]
    pub error: Box<dyn Error + 'static>,
}

#[derive(Debug, Error)]
#[error("There is no `{}` with id {}", .interface.name(), .id)]
pub struct LookupError {
    pub interface: Interface,
    pub id: ObjectId,
}
