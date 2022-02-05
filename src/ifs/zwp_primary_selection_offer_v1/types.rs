use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::zwp_primary_selection_offer_v1::{ZwpPrimarySelectionOfferV1, OFFER};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionOfferV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `receive` request")]
    ReceiveError(#[from] ReceiveError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
}
efrom!(ZwpPrimarySelectionOfferV1Error, ClientError);

#[derive(Debug, Error)]
pub enum ReceiveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReceiveError, ParseFailed, MsgParserError);
efrom!(ReceiveError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

pub(super) struct Receive<'a> {
    pub mime_type: &'a str,
    pub fd: OwnedFd,
}
impl<'a> RequestParser<'a> for Receive<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            mime_type: parser.str()?,
            fd: parser.fd()?,
        })
    }
}
impl Debug for Receive<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "receive(mime_type: {:?}, fd: {})",
            self.mime_type,
            self.fd.raw()
        )
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

pub(super) struct Offer {
    pub obj: Rc<ZwpPrimarySelectionOfferV1>,
    pub mime_type: String,
}
impl EventFormatter for Offer {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, OFFER).string(&self.mime_type);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Offer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "offer(mime_type: {:?})", self.mime_type)
    }
}
