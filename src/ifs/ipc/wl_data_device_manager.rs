use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ipc::{wl_data_device::WlDataDevice, wl_data_source::WlDataSource},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_data_device_manager::*, WlDataDeviceManagerId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub(super) const DND_NONE: u32 = 0;
#[allow(dead_code)]
pub(super) const DND_COPY: u32 = 1;
#[allow(dead_code)]
pub(super) const DND_MOVE: u32 = 2;
#[allow(dead_code)]
pub(super) const DND_ASK: u32 = 4;
pub(super) const DND_ALL: u32 = 7;

pub struct WlDataDeviceManagerGlobal {
    name: GlobalName,
}

pub struct WlDataDeviceManager {
    pub id: WlDataDeviceManagerId,
    pub client: Rc<Client>,
    pub version: u32,
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
        version: u32,
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

impl WlDataDeviceManager {
    fn create_data_source(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataDeviceManagerError> {
        let req: CreateDataSource = self.client.parse(self, parser)?;
        let res = Rc::new(WlDataSource::new(req.id, &self.client, false, self.version));
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        Ok(())
    }

    fn get_data_device(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataDeviceManagerError> {
        let req: GetDataDevice = self.client.parse(&**self, parser)?;
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(WlDataDevice::new(
            req.id,
            &self.client,
            self.version,
            &seat.global,
            false,
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

    CREATE_DATA_SOURCE => create_data_source,
    GET_DATA_DEVICE => get_data_device,
}

impl Object for WlDataDeviceManager {}

simple_add_obj!(WlDataDeviceManager);

#[derive(Debug, Error)]
pub enum WlDataDeviceManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(WlDataDeviceManagerError, ClientError);
efrom!(WlDataDeviceManagerError, MsgParserError);
