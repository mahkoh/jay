mod types;

use crate::client::{Client, DynEventFormatter};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const BIND: u32 = 0;

const GLOBAL: u32 = 0;
const GLOBAL_REMOVE: u32 = 1;

pub struct WlRegistry {
    id: ObjectId,
    client: Rc<Client>,
}

impl WlRegistry {
    pub fn new(id: ObjectId, client: &Rc<Client>) -> Self {
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

    async fn bind(&self, parser: MsgParser<'_, '_>) -> Result<(), BindError> {
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
        global.bind(&self.client, bind.id, bind.version).await?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlRegistryError> {
        match request {
            BIND => self.bind(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlRegistry);

impl Object for WlRegistry {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlRegistry
    }

    fn num_requests(&self) -> u32 {
        BIND + 1
    }
}
