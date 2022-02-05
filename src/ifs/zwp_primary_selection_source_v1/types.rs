use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::zwp_primary_selection_source_v1::{ZwpPrimarySelectionSourceV1, CANCELLED, SEND};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionSourceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("The data source is already attached")]
    AlreadyAttached,
}
efrom!(ZwpPrimarySelectionSourceV1Error, ClientError);

#[derive(Debug, Error)]
pub enum OfferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OfferError, ParseFailed, MsgParserError);
efrom!(OfferError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

pub(super) struct Offer<'a> {
    pub mime_type: &'a str,
}
impl<'a> RequestParser<'a> for Offer<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            mime_type: parser.str()?,
        })
    }
}
impl Debug for Offer<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "offer(mime_type: {:?})", self.mime_type)
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
        write!(f, "destroy()",)
    }
}

pub(super) struct Send {
    pub obj: Rc<ZwpPrimarySelectionSourceV1>,
    pub mime_type: String,
    pub fd: Rc<OwnedFd>,
}
impl EventFormatter for Send {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, SEND)
            .string(&self.mime_type)
            .fd(self.fd);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Send {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "send(mime_type: {:?}, fd: {})",
            self.mime_type,
            self.fd.raw()
        )
    }
}

pub(super) struct Cancelled {
    pub obj: Rc<ZwpPrimarySelectionSourceV1>,
}
impl EventFormatter for Cancelled {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CANCELLED);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Cancelled {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cancelled()")
    }
}
