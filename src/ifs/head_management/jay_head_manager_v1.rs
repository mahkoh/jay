use {
    crate::{
        client::{CAP_HEAD_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::head_management::{
            HeadMgrCommon, head_management_macros::send_available_extensions,
            jay_head_manager_session_v1::JayHeadManagerSessionV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::numcell::NumCell,
        wire::{
            JayHeadManagerV1Id,
            jay_head_manager_v1::{
                CreateSession, Destroy, Done, Extension, ExtensionsDone,
                JayHeadManagerV1RequestHandler,
            },
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JayHeadManagerV1Global {
    pub name: GlobalName,
}

impl JayHeadManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayHeadManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayHeadManagerV1Error> {
        let mgr = Rc::new(JayHeadManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            done_scheduled: Cell::new(false),
            sessions: Default::default(),
            destroyed: Default::default(),
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        send_available_extensions(&mgr);
        mgr.send_extensions_done();
        Ok(())
    }
}

global_base!(
    JayHeadManagerV1Global,
    JayHeadManagerV1,
    JayHeadManagerV1Error
);

simple_add_global!(JayHeadManagerV1Global);

impl Global for JayHeadManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_HEAD_MANAGER
    }
}

pub(super) struct JayHeadManagerV1 {
    pub(super) id: JayHeadManagerV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
    pub(super) done_scheduled: Cell<bool>,
    pub(super) sessions: NumCell<u32>,
    pub(super) destroyed: Cell<bool>,
}

impl JayHeadManagerV1 {
    pub(super) fn send_extension(&self, name: u32, interface: &str, version: Version) {
        self.client.event(Extension {
            self_id: self.id,
            name,
            interface,
            version: version.0,
        });
    }

    fn send_extensions_done(&self) {
        self.client.event(ExtensionsDone { self_id: self.id });
    }

    pub(super) fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }
}

impl JayHeadManagerV1RequestHandler for JayHeadManagerV1 {
    type Error = JayHeadManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.sessions.get() > 0 {
            return Err(JayHeadManagerV1Error::HasSessions);
        }
        self.destroyed.set(true);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_session(&self, req: CreateSession, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayHeadManagerSessionV1 {
            id: req.session,
            mgr: slf.clone(),
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            common: Rc::new(HeadMgrCommon {
                state: Default::default(),
                in_transaction: Cell::new(false),
                transaction_failed: Cell::new(false),
            }),
            serial: Default::default(),
            heads: Default::default(),
            ext: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        self.sessions.fetch_add(1);
        Ok(())
    }
}

object_base! {
    self = JayHeadManagerV1;
    version = self.version;
}

impl Object for JayHeadManagerV1 {}

simple_add_obj!(JayHeadManagerV1);

#[derive(Debug, Error)]
pub enum JayHeadManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Manager still has sessions")]
    HasSessions,
}
efrom!(JayHeadManagerV1Error, ClientError);
