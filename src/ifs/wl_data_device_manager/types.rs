use crate::client::{ClientError, RequestParser};
use crate::ifs::wl_data_device::WlDataDeviceId;
use crate::ifs::wl_data_source::WlDataSourceId;
use crate::ifs::wl_seat::WlSeatId;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlDataDeviceManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `create_data_source` request")]
    CreateDataSourceError(#[from] CreateDataSourceError),
    #[error("Could not process `get_data_device` request")]
    GetDataDeviceError(#[from] GetDataDeviceError),
}
efrom!(WlDataDeviceManagerError, ClientError);

#[derive(Debug, Error)]
pub enum CreateDataSourceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateDataSourceError, ParseFailed, MsgParserError);
efrom!(CreateDataSourceError, ClientError);

#[derive(Debug, Error)]
pub enum GetDataDeviceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetDataDeviceError, ParseFailed, MsgParserError);
efrom!(GetDataDeviceError, ClientError);

pub(super) struct CreateDataSource {
    pub id: WlDataSourceId,
}
impl RequestParser<'_> for CreateDataSource {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for CreateDataSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_data_source(id: {})", self.id)
    }
}

pub(super) struct GetDataDevice {
    pub id: WlDataDeviceId,
    pub seat: WlSeatId,
}
impl RequestParser<'_> for GetDataDevice {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            seat: parser.object()?,
        })
    }
}
impl Debug for GetDataDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_data_device(id: {}, seat: {})", self.id, self.seat,)
    }
}
