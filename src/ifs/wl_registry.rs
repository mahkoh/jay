use crate::client::Client;
use crate::globals::{Global, GlobalName, GlobalsError};
use crate::leaks::Tracker;
use crate::object::{Interface, Object};
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_registry::*;
use crate::wire::WlRegistryId;
use std::rc::Rc;
use thiserror::Error;

pub struct WlRegistry {
    id: WlRegistryId,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WlRegistry {
    pub fn new(id: WlRegistryId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
        }
    }

    pub fn send_global(self: &Rc<Self>, global: &Rc<dyn Global>) {
        self.client.event(crate::wire::wl_registry::Global {
            self_id: self.id,
            name: global.name().raw(),
            interface: global.interface().name(),
            version: global.version(),
        })
    }

    pub fn send_global_remove(self: &Rc<Self>, name: GlobalName) {
        self.client.event(GlobalRemove {
            self_id: self.id,
            name: name.raw(),
        })
    }

    fn bind(&self, parser: MsgParser<'_, '_>) -> Result<(), BindError> {
        let bind: Bind = self.client.parse(self, parser)?;
        let global = self
            .client
            .state
            .globals
            .get(GlobalName::from_raw(bind.name))?;
        if global.interface().name() != bind.interface {
            return Err(BindError::InvalidInterface(InterfaceError {
                name: global.name(),
                interface: global.interface(),
                actual: bind.interface.to_string(),
            }));
        }
        if bind.version > global.version() {
            return Err(BindError::InvalidVersion(VersionError {
                name: global.name(),
                interface: global.interface(),
                version: global.version(),
                actual: bind.version,
            }));
        }
        global.bind(&self.client, bind.id, bind.version)?;
        Ok(())
    }
}

object_base! {
    WlRegistry, WlRegistryError;

    BIND => bind,
}

impl Object for WlRegistry {
    fn num_requests(&self) -> u32 {
        BIND + 1
    }
}

simple_add_obj!(WlRegistry);

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
