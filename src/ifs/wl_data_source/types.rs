use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_data_source::{
    WlDataSource, ACTION, CANCELLED, DND_DROP_PERFORMED, DND_FINISHED, SEND, TARGET,
};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use bstr::{BStr, BString};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlDataSourceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
}
efrom!(WlDataSourceError, ClientError);

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

#[derive(Debug, Error)]
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError);

pub(super) struct Offer<'a> {
    pub mime_type: &'a BStr,
}
impl<'a> RequestParser<'a> for Offer<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            mime_type: parser.string()?,
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

pub(super) struct SetActions {
    pub actions: u32,
}
impl RequestParser<'_> for SetActions {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            actions: parser.uint()?,
        })
    }
}
impl Debug for SetActions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_actions(actions: {})", self.actions)
    }
}

pub(super) struct Target {
    pub obj: Rc<WlDataSource>,
    pub mime_type: BString,
}
impl EventFormatter for Target {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, TARGET).string(&self.mime_type);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "target(mime_type: {:?})", self.mime_type)
    }
}

pub(super) struct Send {
    pub obj: Rc<WlDataSource>,
    pub mime_type: BString,
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
    pub obj: Rc<WlDataSource>,
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

pub(super) struct DndDropPerformed {
    pub obj: Rc<WlDataSource>,
}
impl EventFormatter for DndDropPerformed {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DND_DROP_PERFORMED);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for DndDropPerformed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "dnd_drop_performed()")
    }
}

pub(super) struct DndFinished {
    pub obj: Rc<WlDataSource>,
}
impl EventFormatter for DndFinished {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DND_FINISHED);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for DndFinished {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "dnd_finished()")
    }
}

pub(super) struct Action {
    pub obj: Rc<WlDataSource>,
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
        write!(f, "action(dnd_action: {})", self.dnd_action)
    }
}
