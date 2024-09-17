use {
    crate::{
        client::ClientId,
        object::{Interface, ObjectId},
        utils::buffd::{BufFdError, MsgParserError},
        wire::WlDisplayId,
    },
    std::error::Error,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ClientError {
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
    #[error("Could not process a `{}#{}.{}` request", .interface.name(), .id, .method)]
    MethodError {
        interface: Interface,
        id: ObjectId,
        method: &'static str,
        #[source]
        error: Box<dyn Error + 'static>,
    },
    #[error(transparent)]
    LookupError(LookupError),
    #[error("Could not add object {0} to the client")]
    AddObjectError(ObjectId, #[source] Box<ClientError>),
}

#[derive(Debug, Error)]
#[error("Parsing failed")]
pub struct ParserError(#[source] pub MsgParserError);

impl ClientError {
    pub fn peer_closed(&self) -> bool {
        matches!(self, ClientError::Io(BufFdError::Closed))
    }
}

#[derive(Debug, Error)]
#[error("There is no `{}` with id {}", .interface.name(), .id)]
pub struct LookupError {
    pub interface: Interface,
    pub id: ObjectId,
}
