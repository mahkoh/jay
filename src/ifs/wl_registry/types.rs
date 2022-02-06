use crate::client::{EventFormatter, RequestParser};
use crate::globals::{Global, GlobalName, GlobalsError};
use crate::ifs::wl_registry::{WlRegistry, GLOBAL, GLOBAL_REMOVE};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use bstr::BStr;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlRegistryError {
    #[error("Could not process bind request")]
    BindError(#[source] Box<BindError>),
}

efrom!(WlRegistryError, BindError);

#[derive(Debug, Error)]
pub enum BindError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    GlobalsError(Box<GlobalsError>),
    #[error("Tried to bind to global {} of type {} using interface {}", .0.name, .0.interface.name(), .0.actual)]
    InvalidInterface(InterfaceError),
    #[error("Tried to bind to global {} of type {} and version {} using version {}", .0.name, .0.interface.name(), .0.version, .0.actual)]
    InvalidVersion(VersionError),
}
efrom!(BindError, ParseError, MsgParserError);
efrom!(BindError, GlobalsError);

#[derive(Debug)]
pub struct InterfaceError {
    pub name: GlobalName,
    pub interface: Interface,
    pub actual: String,
}

#[derive(Debug)]
pub struct VersionError {
    pub name: GlobalName,
    pub interface: Interface,
    pub version: u32,
    pub actual: u32,
}

