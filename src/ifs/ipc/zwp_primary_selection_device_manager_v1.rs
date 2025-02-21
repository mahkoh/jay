use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ipc::{
            zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
            zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpPrimarySelectionDeviceManagerV1Id, zwp_primary_selection_device_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPrimarySelectionDeviceManagerV1Global {
    name: GlobalName,
}

pub struct ZwpPrimarySelectionDeviceManagerV1 {
    pub id: ZwpPrimarySelectionDeviceManagerV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionDeviceManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpPrimarySelectionDeviceManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpPrimarySelectionDeviceManagerV1Error> {
        let obj = Rc::new(ZwpPrimarySelectionDeviceManagerV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl ZwpPrimarySelectionDeviceManagerV1RequestHandler for ZwpPrimarySelectionDeviceManagerV1 {
    type Error = ZwpPrimarySelectionDeviceManagerV1Error;

    fn create_source(&self, req: CreateSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let res = Rc::new(ZwpPrimarySelectionSourceV1::new(
            req.id,
            &self.client,
            self.version,
        ));
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        Ok(())
    }

    fn get_device(&self, req: GetDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(ZwpPrimarySelectionDeviceV1::new(
            req.id,
            &self.client,
            self.version,
            &seat.global,
        ));
        track!(self.client, dev);
        seat.global.add_primary_selection_device(&dev);
        self.client.add_client_obj(&dev)?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(
    ZwpPrimarySelectionDeviceManagerV1Global,
    ZwpPrimarySelectionDeviceManagerV1,
    ZwpPrimarySelectionDeviceManagerV1Error
);

impl Global for ZwpPrimarySelectionDeviceManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpPrimarySelectionDeviceManagerV1Global);

object_base! {
    self = ZwpPrimarySelectionDeviceManagerV1;
    version = self.version;
}

impl Object for ZwpPrimarySelectionDeviceManagerV1 {}

simple_add_obj!(ZwpPrimarySelectionDeviceManagerV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionDeviceManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPrimarySelectionDeviceManagerV1Error, ClientError);
