use crate::client::{ClientError, RequestParser};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevelId;
use crate::ifs::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1Id;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZxdgDecorationManagerV1Error {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `get_toplevel_decoration` request")]
    GetToplevelDecorationError(#[from] GetToplevelDecorationError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZxdgDecorationManagerV1Error, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, MsgParserError);

#[derive(Debug, Error)]
pub enum GetToplevelDecorationError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetToplevelDecorationError, ClientError);
efrom!(GetToplevelDecorationError, MsgParserError);

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

pub(super) struct GetToplevelDecoration {
    pub id: ZxdgToplevelDecorationV1Id,
    pub toplevel: XdgToplevelId,
}
impl RequestParser<'_> for GetToplevelDecoration {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            id: parser.object()?,
            toplevel: parser.object()?,
        })
    }
}
impl Debug for GetToplevelDecoration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "get_toplevel_decoration(id: {}, toplevel: {})",
            self.id, self.toplevel
        )
    }
}
