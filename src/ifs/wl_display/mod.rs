mod types;

use crate::client::{AddObj, Client, ClientError, DynEventFormatter};
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_registry::WlRegistry;
use crate::object::{Interface, Object, ObjectId, WL_DISPLAY_ID};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const SYNC: u32 = 0;
const GET_REGISTRY: u32 = 1;

const ERROR: u32 = 0;
const DELETE_ID: u32 = 1;

const INVALID_OBJECT: u32 = 0;
const INVALID_METHOD: u32 = 1;
const NO_MEMORY: u32 = 2;
const IMPLEMENTATION: u32 = 3;

pub struct WlDisplay {
    client: Rc<Client>,
}

impl WlDisplay {
    pub fn new(client: &Rc<Client>) -> Self {
        Self {
            client: client.clone(),
        }
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDisplayError> {
        match request {
            SYNC => self.sync(parser).await?,
            GET_REGISTRY => self.get_registry(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }

    async fn sync(&self, parser: MsgParser<'_, '_>) -> Result<(), SyncError> {
        let sync: Sync = self.client.parse(self, parser)?;
        let cb = Rc::new(WlCallback::new(sync.callback));
        self.client.add_client_obj(&cb)?;
        self.client.event(cb.done()).await?;
        self.client.remove_obj(&*cb).await?;
        Ok(())
    }

    async fn get_registry(&self, parser: MsgParser<'_, '_>) -> Result<(), GetRegistryError> {
        let gr: GetRegistry = self.client.parse(self, parser)?;
        let registry = Rc::new(WlRegistry::new(gr.registry, &self.client));
        self.client.add_client_obj(&registry)?;
        self.client
            .state
            .globals
            .notify_all(&self.client, &registry)
            .await?;
        Ok(())
    }

    fn error(
        self: &Rc<Self>,
        object_id: ObjectId,
        code: u32,
        message: String,
    ) -> DynEventFormatter {
        Box::new(Error {
            obj: self.clone(),
            object_id,
            code,
            message,
        })
    }

    pub fn invalid_request(self: &Rc<Self>, obj: &dyn Object, request: u32) -> DynEventFormatter {
        let id = obj.id();
        let msg = format!(
            "Object {} of type {} has no method {}",
            id,
            obj.interface().name(),
            request
        );
        self.error(id, INVALID_METHOD, msg)
    }

    pub fn invalid_object(self: &Rc<Self>, id: ObjectId) -> DynEventFormatter {
        let msg = format!("Object {} does not exist", id,);
        self.error(id, INVALID_OBJECT, msg)
    }

    pub fn implementation_error(self: &Rc<Self>, msg: String) -> DynEventFormatter {
        self.error(WL_DISPLAY_ID, IMPLEMENTATION, msg)
    }

    pub fn delete_id(self: &Rc<Self>, id: ObjectId) -> DynEventFormatter {
        Box::new(DeleteId {
            obj: self.clone(),
            id,
        })
    }
}

handle_request!(WlDisplay);

impl Object for WlDisplay {
    fn id(&self) -> ObjectId {
        WL_DISPLAY_ID
    }

    fn interface(&self) -> Interface {
        Interface::WlDisplay
    }

    fn num_requests(&self) -> u32 {
        GET_REGISTRY + 1
    }

    fn into_display(self: Rc<Self>) -> Result<Rc<WlDisplay>, ClientError> {
        Ok(self)
    }
}
