use crate::client::Client;
use crate::client::ClientError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZwpIdleInhibitManagerV1Id;
use crate::wire::zwp_idle_inhibit_manager_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;
use thiserror::Error;

pub struct ZwpIdleInhibitManagerV1Global {
    name: GlobalName,
}

impl ZwpIdleInhibitManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpIdleInhibitManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ZwpIdleInhibitManagerV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

global_base!(ZwpIdleInhibitManagerV1Global, ZwpIdleInhibitManagerV1);

impl Global for ZwpIdleInhibitManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpIdleInhibitManagerV1Global);

#[derive(Object)]
pub struct ZwpIdleInhibitManagerV1 {
    id: ZwpIdleInhibitManagerV1Id,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
}

impl ZwpIdleInhibitManagerV1RequestHandler for ZwpIdleInhibitManagerV1 {
    type Error = ZwpIdleInhibitManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_inhibitor(&self, req: CreateInhibitor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let inhibit = Rc::new(ZwpIdleInhibitorV1 {
            id: req.id,
            inhibit_id: self.client.state.idle_inhibitor_ids.next(),
            client: self.client.clone(),
            surface,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, inhibit);
        self.client.add_client_obj(&inhibit);
        inhibit.install();
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ZwpIdleInhibitManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpIdleInhibitManagerV1Error, ClientError);
