use crate::client::{EventFormatter, RequestParser};
use crate::globals::{Global, GlobalsError, GlobalName};
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

efrom!(BindError, ParseError, MsgParserError);
efrom!(BindError, GlobalsError);

pub(super) struct GlobalE {
    pub obj: Rc<WlRegistry>,
    pub global: Rc<dyn Global>,
}
impl EventFormatter for GlobalE {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, GLOBAL)
            .uint(self.global.name().raw())
            .string(self.global.interface().name())
            .uint(self.global.version());
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for GlobalE {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "global(name: {}, interface: {:?}, version: {})",
            self.global.name(),
            self.global.interface().name(),
            self.global.version()
        )
    }
}

pub(super) struct GlobalRemove {
    pub obj: Rc<WlRegistry>,
    pub name: GlobalName,
}
impl EventFormatter for GlobalRemove {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, GLOBAL_REMOVE).uint(self.name.raw());
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for GlobalRemove {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "global_remove(name: {})", self.name)
    }
}

pub(super) struct Bind<'a> {
    pub name: GlobalName,
    pub id: ObjectId,
    pub interface: &'a BStr,
    pub version: u32,
}
impl<'a> RequestParser<'a> for Bind<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            name: parser.global()?,
            interface: parser.bstr()?,
            version: parser.uint()?,
            id: parser.object()?,
        })
    }
}
impl Debug for Bind<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bind(name: {}, interface: {:?}, version: {}, id: {})",
            self.name, self.interface, self.version, self.id
        )
    }
}
