use crate::client::{ClientError, RequestParser};
use crate::utils::buffd::{WlParser, WlParserError};
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
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, WlParserError);
efrom!(DestroyError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum AddError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(AddError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum SubtractError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(SubtractError, ParseFailed, WlParserError);

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()")
    }
}

pub(super) struct Add {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for Add {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for Add {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "add(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height,
        )
    }
}

pub(super) struct Subtract {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for Subtract {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for Subtract {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "subtract(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height,
        )
    }
}
