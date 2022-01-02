use crate::objects::ObjectError;
use crate::wl_client::WlClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgWmBaseError {
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
}

efrom!(XdgWmBaseError, ObjectError, ObjectError);
efrom!(XdgWmBaseError, ClientError, WlClientError);
