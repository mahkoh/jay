use crate::client::ClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlSubcompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(WlSubcompositorError, ClientError, ClientError);
