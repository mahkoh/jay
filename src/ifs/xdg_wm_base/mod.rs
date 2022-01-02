mod types;

use crate::globals::{Global, GlobalName};
use crate::objects::{Interface, Object, ObjectError, ObjectId};
use crate::utils::buffd::WlParser;
use crate::wl_client::WlClientData;
use std::rc::Rc;
pub use types::*;

pub struct XdgWmBaseGlobal {
    name: GlobalName,
}

pub struct XdgWmBaseObj {
    global: Rc<XdgWmBaseGlobal>,
    id: ObjectId,
    version: u32,
}

impl XdgWmBaseGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    async fn bind_(
        self: Rc<Self>,
        id: ObjectId,
        client: &WlClientData,
        version: u32,
    ) -> Result<(), XdgWmBaseError> {
        let obj = Rc::new(XdgWmBaseObj {
            global: self,
            id,
            version,
        });
        client.attach_client_object(obj)?;
        Ok(())
    }
}

impl XdgWmBaseObj {
    async fn handle_request_(
        &self,
        request: u32,
        parser: WlParser<'_, '_>,
    ) -> Result<(), ObjectError> {
        unreachable!();
    }
}

bind!(XdgWmBaseGlobal);

impl Global for XdgWmBaseGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::XdgWmBase
    }

    fn version(&self) -> u32 {
        3
    }

    fn pre_remove(&self) {
        unreachable!()
    }
}

handle_request!(XdgWmBaseObj);

impl Object for XdgWmBaseObj {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::XdgWmBase
    }

    fn num_requests(&self) -> u32 {
        0
    }
}
