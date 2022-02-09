use crate::client::{Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::ifs::ipc::wl_data_device::WlDataDevice;
use crate::ifs::ipc::wl_data_source::WlDataSource;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_device_manager::*;
use crate::wire::WlDataDeviceManagerId;
use std::rc::Rc;
use thiserror::Error;

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
        });
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WlDataDeviceManager {
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
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(WlDataDevice::new(req.id, self, &seat));
        seat.add_data_device(&dev);
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
    WlDataDeviceManager, WlDataDeviceManagerError;

    CREATE_DATA_SOURCE => create_data_source,
    GET_DATA_DEVICE => get_data_device,
}

impl Object for WlDataDeviceManager {
    fn num_requests(&self) -> u32 {
        GET_DATA_DEVICE + 1
    }
}

simple_add_obj!(WlDataDeviceManager);

#[derive(Debug, Error)]
pub enum WlDataDeviceManagerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `create_data_source` request")]
    CreateDataSourceError(#[from] CreateDataSourceError),
    #[error("Could not process `get_data_device` request")]
    GetDataDeviceError(#[from] GetDataDeviceError),
}
efrom!(WlDataDeviceManagerError, ClientError);

#[derive(Debug, Error)]
pub enum CreateDataSourceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateDataSourceError, ParseFailed, MsgParserError);
efrom!(CreateDataSourceError, ClientError);

#[derive(Debug, Error)]
pub enum GetDataDeviceError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetDataDeviceError, ParseFailed, MsgParserError);
efrom!(GetDataDeviceError, ClientError);
