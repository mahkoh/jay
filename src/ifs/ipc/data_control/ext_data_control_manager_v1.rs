use {
    crate::{
        client::{CAP_DATA_CONTROL_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::ipc::{
            IpcLocation,
            data_control::{
                DynDataControlDevice, ext_data_control_device_v1::ExtDataControlDeviceV1,
                ext_data_control_source_v1::ExtDataControlSourceV1,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ExtDataControlManagerV1Id, ext_data_control_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtDataControlManagerV1Global {
    name: GlobalName,
}

pub struct ExtDataControlManagerV1 {
    pub id: ExtDataControlManagerV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    tracker: Tracker<Self>,
}

impl ExtDataControlManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtDataControlManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtDataControlManagerV1Error> {
        let obj = Rc::new(ExtDataControlManagerV1 {
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

impl ExtDataControlManagerV1RequestHandler for ExtDataControlManagerV1 {
    type Error = ExtDataControlManagerV1Error;

    fn create_data_source(
        &self,
        req: CreateDataSource,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let res = Rc::new(ExtDataControlSourceV1::new(
            req.id,
            &self.client,
            self.version,
        ));
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        Ok(())
    }

    fn get_data_device(&self, req: GetDataDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(ExtDataControlDeviceV1::new(
            req.id,
            &self.client,
            self.version,
            &seat.global,
        ));
        track!(self.client, dev);
        seat.global.add_data_control_device(dev.clone());
        self.client.add_client_obj(&dev)?;
        dev.clone()
            .handle_new_source(IpcLocation::Clipboard, seat.global.get_selection());
        dev.clone().handle_new_source(
            IpcLocation::PrimarySelection,
            seat.global.get_primary_selection(),
        );
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(
    ExtDataControlManagerV1Global,
    ExtDataControlManagerV1,
    ExtDataControlManagerV1Error
);

impl Global for ExtDataControlManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_DATA_CONTROL_MANAGER
    }
}

simple_add_global!(ExtDataControlManagerV1Global);

object_base! {
    self = ExtDataControlManagerV1;
    version = self.version;
}

impl Object for ExtDataControlManagerV1 {}

simple_add_obj!(ExtDataControlManagerV1);

#[derive(Debug, Error)]
pub enum ExtDataControlManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtDataControlManagerV1Error, ClientError);
