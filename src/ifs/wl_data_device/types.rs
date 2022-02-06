use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::fixed::Fixed;
use crate::ifs::wl_data_device::{WlDataDevice, DATA_OFFER, DROP, ENTER, LEAVE, MOTION, SELECTION};
use crate::ifs::wl_data_offer::WlDataOfferId;
use crate::ifs::wl_data_source::{WlDataSourceError, WlDataSourceId};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlDataDeviceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `start_drag` request")]
    StartDragError(#[from] StartDragError),
    #[error("Could not process `set_selection` request")]
    SetSelectionError(#[from] SetSelectionError),
    #[error("Could not process `release` request")]
    ReleaseError(#[from] ReleaseError),
}
efrom!(WlDataDeviceError, ClientError);

#[derive(Debug, Error)]
pub enum StartDragError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(StartDragError, ParseFailed, MsgParserError);
efrom!(StartDragError, ClientError);

#[derive(Debug, Error)]
pub enum SetSelectionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlDataSourceError(Box<WlDataSourceError>),
}
efrom!(SetSelectionError, ParseFailed, MsgParserError);
efrom!(SetSelectionError, ClientError);
efrom!(SetSelectionError, WlDataSourceError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseFailed, MsgParserError);
efrom!(ReleaseError, ClientError);
