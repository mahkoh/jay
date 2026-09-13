use crate::client::CAP_SEAT_MANAGER;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_seat::ext_transient_seat_v1::ExtTransientSeatV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ExtTransientSeatManagerV1Id;
use crate::wire::ext_transient_seat_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct ExtTransientSeatManagerV1Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct ExtTransientSeatManagerV1 {
    id: ExtTransientSeatManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl ExtTransientSeatManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtTransientSeatManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ExtTransientSeatManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for ExtTransientSeatManagerV1Global {
    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_SEAT_MANAGER
    }
}

impl ExtTransientSeatManagerV1RequestHandler for ExtTransientSeatManagerV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create(&self, req: Create, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(ExtTransientSeatV1 {
            id: req.seat,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        obj.send_denied();
        Ok(())
    }
}
