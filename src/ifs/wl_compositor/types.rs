use crate::client::{ClientError, RequestParser};
use crate::ifs::wl_region::WlRegionId;
use crate::ifs::wl_surface::WlSurfaceId;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlCompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `create_surface` request")]
    CreateSurfaceError(#[source] Box<CreateSurfaceError>),
    #[error("Could not process `create_region` request")]
    CreateRegionError(#[source] Box<CreateRegionError>),
}

efrom!(WlCompositorError, ClientError);
efrom!(WlCompositorError, CreateSurfaceError);
efrom!(WlCompositorError, CreateRegionError);

#[derive(Debug, Error)]
pub enum CreateSurfaceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(CreateSurfaceError, ParseFailed, MsgParserError);
efrom!(CreateSurfaceError, ClientError);

#[derive(Debug, Error)]
pub enum CreateRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(CreateRegionError, ParseFailed, MsgParserError);
efrom!(CreateRegionError, ClientError, ClientError);
