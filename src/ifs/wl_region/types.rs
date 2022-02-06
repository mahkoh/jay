use crate::client::{ClientError, RequestParser};
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlRegionError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `add` request")]
    AddError(#[from] AddError),
    #[error("Could not process `subtract` request")]
    SubtractError(#[from] SubtractError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum AddError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(AddError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SubtractError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(SubtractError, ParseFailed, MsgParserError);
