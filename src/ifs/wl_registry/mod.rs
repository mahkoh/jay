mod types;

use crate::client::{Client, DynEventFormatter};
use crate::globals::{Global, GlobalName};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const BIND: u32 = 0;

const GLOBAL: u32 = 0;
const GLOBAL_REMOVE: u32 = 1;

id!(WlRegistryId);

pub struct WlRegistry {
    id: WlRegistryId,
    client: Rc<Client>,
}

impl WlRegistry {
    pub fn new(id: WlRegistryId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
        }
    }

    pub fn global(self: &Rc<Self>, global: &Rc<dyn Global>) -> DynEventFormatter {
        Box::new(GlobalE {
            obj: self.clone(),
            global: global.clone(),
        })
    }

    pub fn global_remove(self: &Rc<Self>, name: GlobalName) -> DynEventFormatter {
        Box::new(GlobalRemove {
            obj: self.clone(),
            name,
        })
    }

    fn bind(&self, parser: MsgParser<'_, '_>) -> Result<(), BindError> {
        let bind: Bind = self.client.parse(self, parser)?;
        let global = self.client.state.globals.get(bind.name)?;
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
