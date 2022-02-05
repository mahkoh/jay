use crate::client::{ClientError, RequestParser};
use crate::ifs::wl_seat::WlSeatId;
use crate::ifs::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1Id;
use crate::ifs::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1Id;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionDeviceManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `create_source` request")]
    CreateSourceError(#[from] CreateSourceError),
    #[error("Could not process `get_device` request")]
    GetDeviceError(#[from] GetDeviceError),
}
efrom!(ZwpPrimarySelectionDeviceManagerV1Error, ClientError);

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
pub enum CreateSourceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateSourceError, ParseFailed, MsgParserError);
efrom!(CreateSourceError, ClientError);

#[derive(Debug, Error)]
pub enum GetDeviceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetDeviceError, ParseFailed, MsgParserError);
efrom!(GetDeviceError, ClientError);

pub(super) struct CreateSource {
    pub id: ZwpPrimarySelectionSourceV1Id,
}
impl RequestParser<'_> for CreateSource {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for CreateSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_source(id: {})", self.id)
    }
}

pub(super) struct GetDevice {
    pub id: ZwpPrimarySelectionDeviceV1Id,
    pub seat: WlSeatId,
}
impl RequestParser<'_> for GetDevice {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            seat: parser.object()?,
        })
    }
}
impl Debug for GetDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_device(id: {}, seat: {})", self.id, self.seat,)
    }
}

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()")
    }
}
