
use crate::client::{Client, ClientError, DynEventFormatter};
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_registry::WlRegistry;
use crate::object::{Object, ObjectId, WL_DISPLAY_ID};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
use thiserror::Error;
use crate::globals::GlobalsError;
use crate::wire::wl_display::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::WlDisplayId;

const INVALID_OBJECT: u32 = 0;
const INVALID_METHOD: u32 = 1;
#[allow(dead_code)]
const NO_MEMORY: u32 = 2;
const IMPLEMENTATION: u32 = 3;

pub struct WlDisplay {
    id: WlDisplayId,
    client: Rc<Client>,
}

impl WlDisplay {
    pub fn new(client: &Rc<Client>) -> Self {
        Self {
            id: WL_DISPLAY_ID,
            client: client.clone(),
        }
    }

    fn sync(&self, parser: MsgParser<'_, '_>) -> Result<(), SyncError> {
        let sync: Sync = self.client.parse(self, parser)?;
        let cb = Rc::new(WlCallback::new(sync.callback));
        self.client.add_client_obj(&cb)?;
        self.client.event(cb.done());
        self.client.remove_obj(&*cb)?;
        Ok(())
    }

    fn get_registry(&self, parser: MsgParser<'_, '_>) -> Result<(), GetRegistryError> {
        let gr: GetRegistry = self.client.parse(self, parser)?;
        let registry = Rc::new(WlRegistry::new(gr.registry, &self.client));
        self.client.add_client_obj(&registry)?;
        self.client
            .state
            .globals
            .notify_all(&self.client, &registry);
        Ok(())
    }

    pub fn error<O: Into<ObjectId>>(
        self: &Rc<Self>,
        object_id: O,
        code: u32,
        message: String,
    ) -> DynEventFormatter {
        Box::new(ErrorOut {
            self_id: self.id,
            object_id: object_id.into(),
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
            self_id: self.id,
            id: id.raw(),
        })
    }
}

object_base! {
    WlDisplay, WlDisplayError;

    SYNC => sync,
    GET_REGISTRY => get_registry,
}

impl Object for WlDisplay {
    fn num_requests(&self) -> u32 {
        GET_REGISTRY + 1
    }
}

#[derive(Debug, Error)]
pub enum WlDisplayError {
    #[error("Could not process a get_registry request")]
    GetRegistryError(#[source] Box<GetRegistryError>),
    #[error("A client error occurred")]
    SyncError(#[source] Box<SyncError>),
}

efrom!(WlDisplayError, GetRegistryError);
efrom!(WlDisplayError, SyncError);

#[derive(Debug, Error)]
pub enum GetRegistryError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("An error occurred while processing globals")]
    GlobalsError(#[source] Box<GlobalsError>),
}

efrom!(GetRegistryError, ParseFailed, MsgParserError);
efrom!(GetRegistryError, GlobalsError);
efrom!(GetRegistryError, ClientError);

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}

efrom!(SyncError, ParseFailed, MsgParserError);
efrom!(SyncError, ClientError);
