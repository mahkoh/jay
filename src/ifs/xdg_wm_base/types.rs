use crate::client::ClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgWmBaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(XdgWmBaseError, ClientError, ClientError);
