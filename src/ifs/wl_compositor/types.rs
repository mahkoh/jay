use crate::client::{ClientError, RequestParser};
use crate::object::ObjectId;
use crate::utils::buffd::{WlParser, WlParserError};
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

efrom!(WlCompositorError, ClientError, ClientError);
efrom!(WlCompositorError, CreateSurfaceError, CreateSurfaceError);
efrom!(WlCompositorError, CreateRegionError, CreateRegionError);

#[derive(Debug, Error)]
pub enum CreateSurfaceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(CreateSurfaceError, ParseFailed, WlParserError);
efrom!(CreateSurfaceError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum CreateRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(CreateRegionError, ParseFailed, WlParserError);
efrom!(CreateRegionError, ClientError, ClientError);

pub(super) struct CreateSurface {
    pub id: ObjectId,
}
impl RequestParser<'_> for CreateSurface {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for CreateSurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_surface(id: {})", self.id)
    }
}

pub(super) struct CreateRegion {
    pub id: ObjectId,
}
impl RequestParser<'_> for CreateRegion {
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
        Ok(Self {
            id: parser.object()?,
        })
    }
}
impl Debug for CreateRegion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "create_region(id: {})", self.id)
    }
}
