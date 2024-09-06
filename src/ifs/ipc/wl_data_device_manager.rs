use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ipc::{wl_data_device::WlDataDevice, wl_data_source::WlDataSource},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wl_data_device_manager::*, WlDataDeviceManagerId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub(super) const DND_NONE: u32 = 0;
#[expect(dead_code)]
pub(super) const DND_COPY: u32 = 1;
#[expect(dead_code)]
pub(super) const DND_MOVE: u32 = 2;
#[expect(dead_code)]
pub(super) const DND_ASK: u32 = 4;
pub(super) const DND_ALL: u32 = 7;

pub struct WlDataDeviceManagerGlobal {
    name: GlobalName,
}

pub struct WlDataDeviceManager {
    pub id: WlDataDeviceManagerId,
    pub client: Rc<Client>,
    pub version: Version,
    tracker: Tracker<Self>,
}

impl WlDataDeviceManagerGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlDataDeviceManagerId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlDataDeviceManagerError> {
        let obj = Rc::new(WlDataDeviceManager {
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

impl WlDataDeviceManagerRequestHandler for WlDataDeviceManager {
    type Error = WlDataDeviceManagerError;

    fn create_data_source(
        &self,
        req: CreateDataSource,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let res = Rc::new(WlDataSource::new(req.id, &self.client, self.version));
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        Ok(())
    }

    fn get_data_device(&self, req: GetDataDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(WlDataDevice::new(
            req.id,
            &self.client,
            self.version,
            &seat.global,
        ));
        track!(self.client, dev);
        seat.global.add_data_device(&dev);
        self.client.add_client_obj(&dev)?;
        Ok(())
    }
}

global_base!(
    WlDataDeviceManagerGlobal,
    WlDataDeviceManager,
    WlDataDeviceManagerError
);

impl Global for WlDataDeviceManagerGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        3
    }
}

simple_add_global!(WlDataDeviceManagerGlobal);

object_base! {
    self = WlDataDeviceManager;
    version = self.version;
}

impl Object for WlDataDeviceManager {}

simple_add_obj!(WlDataDeviceManager);

#[derive(Debug, Error)]
pub enum WlDataDeviceManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlDataDeviceManagerError, ClientError);
