mod types;

use crate::client::{AddObj, Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::WlParser;
use std::rc::Rc;
pub use types::*;

pub struct WlSubcompositorGlobal {
    name: GlobalName,
}

pub struct WlSubcompositorObj {
    global: Rc<WlSubcompositorGlobal>,
    id: ObjectId,
}

impl WlSubcompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    async fn bind_(
        self: Rc<Self>,
        id: ObjectId,
        client: &Client,
        _version: u32,
    ) -> Result<(), WlSubcompositorError> {
        let obj = Rc::new(WlSubcompositorObj { global: self, id });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlSubcompositorObj {
    async fn handle_request_(
        &self,
        request: u32,
        parser: WlParser<'_, '_>,
    ) -> Result<(), ClientError> {
        unreachable!();
    }
}

bind!(WlSubcompositorGlobal);

impl Global for WlSubcompositorGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::WlSubcompositor
    }

    fn version(&self) -> u32 {
        1
    }

    fn pre_remove(&self) {
        unreachable!()
    }
}

handle_request!(WlSubcompositorObj);

impl Object for WlSubcompositorObj {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlSubcompositor
    }

    fn num_requests(&self) -> u32 {
        0
    }
}
