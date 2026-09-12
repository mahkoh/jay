use crate::client::Client;
use crate::client::ClientError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::wp_tearing_control_v1::WpTearingControlV1;
use crate::ifs::wl_surface::wp_tearing_control_v1::WpTearingControlV1Error;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpTearingControlManagerV1Id;
use crate::wire::wp_tearing_control_manager_v1::*;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

pub struct WpTearingControlManagerV1Global {
    name: GlobalName,
}

impl WpTearingControlManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpTearingControlManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpTearingControlManagerV1Error> {
        let obj = Rc::new(WpTearingControlManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(WpTearingControlManagerV1Global, WpTearingControlManagerV1);

impl Global for WpTearingControlManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpTearingControlManagerV1Global);

#[derive(Object)]
pub struct WpTearingControlManagerV1 {
    id: WpTearingControlManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl WpTearingControlManagerV1RequestHandler for WpTearingControlManagerV1 {
    type Error = WpTearingControlManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_tearing_control(
        &self,
        req: GetTearingControl,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let control = Rc::new(WpTearingControlV1 {
            id: req.id,
            surface,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, control);
        self.client.add_client_obj(&control)?;
        control.install()?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WpTearingControlManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpTearingControlV1Error(#[from] WpTearingControlV1Error),
}
efrom!(WpTearingControlManagerV1Error, ClientError);
