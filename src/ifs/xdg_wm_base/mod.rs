mod types;

use crate::client::{AddObj, Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
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
        client: &Client,
        version: u32,
    ) -> Result<(), XdgWmBaseError> {
        let obj = Rc::new(XdgWmBaseObj {
            global: self,
            id,
            version,
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl XdgWmBaseObj {
    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ClientError> {
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
