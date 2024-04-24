use {
    crate::{
        client::{Client, ClientCaps, ClientError, CAP_SEAT_MANAGER},
        globals::{Global, GlobalName},
        ifs::wl_seat::ext_transient_seat_v1::ExtTransientSeatV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ext_transient_seat_manager_v1::*, ExtTransientSeatManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtTransientSeatManagerV1Global {
    pub name: GlobalName,
}

pub struct ExtTransientSeatManagerV1 {
    pub id: ExtTransientSeatManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
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
    ) -> Result<(), ExtTransientSeatManagerV1Error> {
        let obj = Rc::new(ExtTransientSeatManagerV1 {
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

global_base!(
    ExtTransientSeatManagerV1Global,
    ExtTransientSeatManagerV1,
    ExtTransientSeatManagerV1Error
);

impl Global for ExtTransientSeatManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_SEAT_MANAGER
    }
}

simple_add_global!(ExtTransientSeatManagerV1Global);

impl ExtTransientSeatManagerV1RequestHandler for ExtTransientSeatManagerV1 {
    type Error = ExtTransientSeatManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
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
        self.client.add_client_obj(&obj)?;
        obj.send_denied();
        Ok(())
    }
}

object_base! {
    self = ExtTransientSeatManagerV1;
    version = self.version;
}

impl Object for ExtTransientSeatManagerV1 {}

simple_add_obj!(ExtTransientSeatManagerV1);

#[derive(Debug, Error)]
pub enum ExtTransientSeatManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtTransientSeatManagerV1Error, ClientError);
