use crate::client::{ClientError, RequestParser};
use crate::ifs::wl_surface::{SurfaceRole, WlSurfaceId};
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlSubsurfaceError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_position` request")]
    SetPosition(#[from] SetPositionError),
    #[error("Could not process `place_above` request")]
    PlaceAbove(#[from] PlaceAboveError),
    #[error("Could not process `place_below` request")]
    PlaceBelow(#[from] PlaceBelowError),
    #[error("Could not process `set_sync` request")]
    SetSync(#[from] SetSyncError),
    #[error("Could not process `set_desync` request")]
    SetDesync(#[from] SetDesyncError),
    #[error("Surface {0} cannot be assigned the role `Subsurface` because it already has the role `{1:?}`")]
    IncompatibleType(WlSurfaceId, SurfaceRole),
    #[error("Surface {0} already has an attached `wl_subsurface`")]
    AlreadyAttached(WlSurfaceId),
    #[error("Surface {0} cannot be made its own parent")]
    OwnParent(WlSurfaceId),
    #[error("Surface {0} cannot be made a subsurface of {1} because it's an ancestor of {1}")]
    Ancestor(WlSurfaceId, WlSurfaceId),
    #[error("Subsurfaces cannot be nested deeper than 100 levels")]
    MaxDepthExceeded,
}

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
pub enum SetPositionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetPositionError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum PlaceAboveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    PlacementError(#[from] PlacementError),
}
efrom!(PlaceAboveError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum PlacementError {
    #[error("Cannot place {0} above/below itself")]
    AboveSelf(WlSurfaceId),
    #[error("{0} is not a sibling of {1}")]
    NotASibling(WlSurfaceId, WlSurfaceId),
}

#[derive(Debug, Error)]
pub enum PlaceBelowError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    PlacementError(#[from] PlacementError),
}
efrom!(PlaceBelowError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetSyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetSyncError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetDesyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetDesyncError, ParseFailed, MsgParserError);

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

pub(super) struct SetPosition {
    pub x: i32,
    pub y: i32,
}
impl RequestParser<'_> for SetPosition {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
        })
    }
}
impl Debug for SetPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_position(x: {}, y: {})", self.x, self.y)
    }
}

pub(super) struct PlaceAbove {
    pub sibling: WlSurfaceId,
}
impl RequestParser<'_> for PlaceAbove {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            sibling: parser.object()?,
        })
    }
}
impl Debug for PlaceAbove {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "place_above(sibling: {})", self.sibling,)
    }
}

pub(super) struct PlaceBelow {
    pub sibling: WlSurfaceId,
}
impl RequestParser<'_> for PlaceBelow {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            sibling: parser.object()?,
        })
    }
}
impl Debug for PlaceBelow {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "place_below(sibling: {})", self.sibling,)
    }
}

pub(super) struct SetSync;
impl RequestParser<'_> for SetSync {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for SetSync {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_sync()")
    }
}

pub(super) struct SetDesync;
impl RequestParser<'_> for SetDesync {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for SetDesync {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_desync()")
    }
}
