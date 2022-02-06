use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::globals::GlobalsError;
use crate::ifs::wl_callback::WlCallbackId;
use crate::ifs::wl_display::{WlDisplay, DELETE_ID, ERROR};
use crate::ifs::wl_registry::WlRegistryId;
use crate::object::{Object, ObjectId, WL_DISPLAY_ID};
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlDisplayError {
    #[error("Could not process a get_registry request")]
    GetRegistryError(#[source] Box<GetRegistryError>),
    #[error("A client error occurred")]
    SyncError(#[source] Box<SyncError>),
}

efrom!(WlDisplayError, GetRegistryError);
efrom!(WlDisplayError, SyncError);

#[derive(Debug, Error)]
pub enum GetRegistryError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("An error occurred while processing globals")]
    GlobalsError(#[source] Box<GlobalsError>),
}

efrom!(GetRegistryError, ParseFailed, MsgParserError);
efrom!(GetRegistryError, GlobalsError);
efrom!(GetRegistryError, ClientError);

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(SyncError, ParseFailed, MsgParserError);
efrom!(SyncError, ClientError);
