mod types;

use crate::client::Client;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_data_device::WlDataDevice;
use crate::ifs::wl_data_source::WlDataSource;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const CREATE_DATA_SOURCE: u32 = 0;
const GET_DATA_DEVICE: u32 = 1;

#[allow(dead_code)]
const DND_NONE: u32 = 0;
#[allow(dead_code)]
const DND_COPY: u32 = 1;
#[allow(dead_code)]
const DND_MOVE: u32 = 2;
#[allow(dead_code)]
const DND_ASK: u32 = 4;

id!(WlDataDeviceManagerId);

pub struct WlDataDeviceManagerGlobal {
    name: GlobalName,
}

pub struct WlDataDeviceManagerObj {
    pub id: WlDataDeviceManagerId,
    pub client: Rc<Client>,
    pub version: u32,
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
        let obj = Rc::new(WlDataDeviceManagerObj {
            id,
            client: client.clone(),
            version,
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlDataDeviceManagerObj {
    fn create_data_source(&self, parser: MsgParser<'_, '_>) -> Result<(), CreateDataSourceError> {
        let req: CreateDataSource = self.client.parse(self, parser)?;
        let res = Rc::new(WlDataSource::new(req.id, &self.client));
        self.client.add_client_obj(&res)?;
        Ok(())
    }

    fn get_data_device(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), GetDataDeviceError> {
        let req: GetDataDevice = self.client.parse(&**self, parser)?;
        let seat = self.client.get_wl_seat(req.seat)?;
        let dev = Rc::new(WlDataDevice::new(req.id, self, &seat));
        seat.add_data_device(&dev);
        self.client.add_client_obj(&dev)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataDeviceManagerError> {
        match request {
            CREATE_DATA_SOURCE => self.create_data_source(parser)?,
            GET_DATA_DEVICE => self.get_data_device(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

bind!(WlDataDeviceManagerGlobal);

impl Global for WlDataDeviceManagerGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn singleton(&self) -> bool {
        true
    }

    fn interface(&self) -> Interface {
        Interface::WlDataDeviceManager
    }

    fn version(&self) -> u32 {
        3
    }
}

handle_request!(WlDataDeviceManagerObj);

impl Object for WlDataDeviceManagerObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataDeviceManager
    }

    fn num_requests(&self) -> u32 {
        GET_DATA_DEVICE + 1
    }
}
