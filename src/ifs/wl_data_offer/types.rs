use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_data_offer::{WlDataOffer, ACTION, OFFER, SOURCE_ACTIONS};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use bstr::{BStr, BString};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlDataOfferError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `accept` request")]
    AcceptError(#[from] AcceptError),
    #[error("Could not process `receive` request")]
    ReceiveError(#[from] ReceiveError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `finish` request")]
    FinishError(#[from] FinishError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
}
efrom!(WlDataOfferError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum AcceptError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(AcceptError, ParseFailed, MsgParserError);
efrom!(AcceptError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum ReceiveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReceiveError, ParseFailed, MsgParserError);
efrom!(ReceiveError, ClientError, ClientError);

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
pub enum FinishError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(FinishError, ParseFailed, MsgParserError);
efrom!(FinishError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError, ClientError);

pub(super) struct Accept<'a> {
    pub serial: u32,
    pub mime_type: &'a BStr,
}
impl<'a> RequestParser<'a> for Accept<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            serial: parser.uint()?,
            mime_type: parser.string()?,
        })
    }
}
impl Debug for Accept<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "accept(serial: {}, mime_type: {:?})",
            self.serial, self.mime_type
        )
    }
}

pub(super) struct Receive<'a> {
    pub mime_type: &'a BStr,
    pub fd: OwnedFd,
}
impl<'a> RequestParser<'a> for Receive<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            mime_type: parser.string()?,
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

pub(super) struct Finish;
impl RequestParser<'_> for Finish {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Finish {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "finish()")
    }
}

pub(super) struct SetActions {
    pub dnd_actions: u32,
    pub preferred_action: u32,
}
impl<'a> RequestParser<'a> for SetActions {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            dnd_actions: parser.uint()?,
            preferred_action: parser.uint()?,
        })
    }
}
impl Debug for SetActions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_actions(dnd_actions: {}, preferred_action: {})",
            self.dnd_actions, self.preferred_action
        )
    }
}

pub(super) struct Offer {
    pub obj: Rc<WlDataOffer>,
    pub mime_type: BString,
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
        write!(f, "target(mime_type: {:?})", self.mime_type)
    }
}

pub(super) struct SourceActions {
    pub obj: Rc<WlDataOffer>,
    pub source_actions: u32,
}
impl EventFormatter for SourceActions {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, SOURCE_ACTIONS)
            .uint(self.source_actions);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for SourceActions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "source_actions(source_actions: {})", self.source_actions,)
    }
}

pub(super) struct Action {
    pub obj: Rc<WlDataOffer>,
    pub dnd_action: u32,
}
impl EventFormatter for Action {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, ACTION).uint(self.dnd_action);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "action(dnd_action: {})", self.dnd_action,)
    }
}
