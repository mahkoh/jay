use crate::objects::ObjectError;
use crate::wl_client::WlClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlSubcompositorError {
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
}

efrom!(WlSubcompositorError, ObjectError, ObjectError);
efrom!(WlSubcompositorError, ClientError, WlClientError);
