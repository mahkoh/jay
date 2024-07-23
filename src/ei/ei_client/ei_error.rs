use {
    crate::{
        ei::ei_object::{EiInterface, EiObjectId},
        utils::buffd::{BufFdError, EiMsgParserError},
    },
    std::error::Error,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum EiClientError {
    #[error("An error occurred reading from/writing to the client")]
    Io(#[from] BufFdError),
    #[error("An error occurred while processing a request")]
    RequestError(#[source] Box<EiClientError>),
    #[error("Client tried to invoke a non-existent method")]
    InvalidMethod,
    #[error("The message size is < 16")]
    MessageSizeTooSmall,
    #[error("The message size is > 2^16")]
    MessageSizeTooLarge,
    #[error("The size of the message is not a multiple of 4")]
    UnalignedMessage,
    #[error("The object id is unknown")]
    UnknownId,
    #[error("Client tried to access non-existent object {0}")]
    InvalidObject(EiObjectId),
    #[error("The id is already in use")]
    IdAlreadyInUse,
    #[error("The client object id is out of bounds")]
    ClientIdOutOfBounds,
    #[error("Could not process a `{}.{}` request", .interface.name(), .method)]
    MethodError {
        interface: EiInterface,
        method: &'static str,
        #[source]
        error: Box<dyn Error + 'static>,
    },
    #[error("Could not add object {0} to the client")]
    AddObjectError(EiObjectId, #[source] Box<EiClientError>),
}

#[derive(Debug, Error)]
#[error("Parsing failed")]
pub struct EiParserError(#[source] pub EiMsgParserError);
