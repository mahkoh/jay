mod types;

use crate::globals::{Global, GlobalName};
use crate::objects::{Interface, Object, ObjectError, ObjectId};
use crate::utils::buffd::WlParser;
use crate::wl_client::WlClientData;
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
        client: &WlClientData,
        _version: u32,
    ) -> Result<(), WlSubcompositorError> {
        let obj = Rc::new(WlSubcompositorObj { global: self, id });
        client.attach_client_object(obj)?;
        Ok(())
    }
}

impl WlSubcompositorObj {
    async fn handle_request_(
        &self,
        request: u32,
        parser: WlParser<'_, '_>,
    ) -> Result<(), ObjectError> {
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
