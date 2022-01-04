use crate::client::{ClientError, RequestParser};
use crate::ifs::wl_surface::wl_subsurface::{WlSubsurfaceError, WlSubsurfaceId};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlSubcompositorError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `get_subsurface` request")]
    GetSubsurfaceError(#[from] GetSubsurfaceError),
}
efrom!(WlSubcompositorError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum GetSubsurfaceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    SubsurfaceError(Box<WlSubsurfaceError>),
}
efrom!(GetSubsurfaceError, ParseFailed, MsgParserError);
efrom!(GetSubsurfaceError, ClientError, ClientError);
efrom!(GetSubsurfaceError, SubsurfaceError, WlSubsurfaceError);

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

pub(super) struct GetSubsurface {
    pub id: WlSubsurfaceId,
    pub surface: WlSurfaceId,
    pub parent: WlSurfaceId,
}
impl RequestParser<'_> for GetSubsurface {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            surface: parser.object()?,
            parent: parser.object()?,
        })
    }
}
impl Debug for GetSubsurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "get_subsurface(id: {}, surface: {}, parent: {})",
            self.id, self.surface, self.parent,
        )
    }
}
