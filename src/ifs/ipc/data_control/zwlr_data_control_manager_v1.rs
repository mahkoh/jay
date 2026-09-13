use crate::client::CAP_DATA_CONTROL_MANAGER;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::ipc::IpcLocation;
use crate::ifs::ipc::data_control::DynDataControlDevice;
use crate::ifs::ipc::data_control::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1;
use crate::ifs::ipc::data_control::zwlr_data_control_source_v1::ZwlrDataControlSourceV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZwlrDataControlManagerV1Id;
use crate::wire::zwlr_data_control_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct ZwlrDataControlManagerV1Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct ZwlrDataControlManagerV1 {
    id: ZwlrDataControlManagerV1Id,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
}

impl ZwlrDataControlManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrDataControlManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ZwlrDataControlManagerV1 {
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

impl ZwlrDataControlManagerV1RequestHandler for ZwlrDataControlManagerV1 {
    type Error = LookupError;

    fn create_data_source(
        &self,
        req: CreateDataSource,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let res = Rc::new(ZwlrDataControlSourceV1::new(
            req.id,
            &self.client,
            self.version,
        ));
        track!(self.client, res);
        self.client.add_client_obj(&res);
        Ok(())
    }

    fn get_data_device(&self, req: GetDataDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(ZwlrDataControlDeviceV1::new(
            req.id,
            &self.client,
            self.version,
            &seat.global,
        ));
        track!(self.client, dev);
        seat.global.add_data_control_device(dev.clone());
        self.client.add_client_obj(&dev);
        dev.clone()
            .handle_new_source(IpcLocation::Clipboard, seat.global.get_selection());
        dev.clone().handle_new_source(
            IpcLocation::PrimarySelection,
            seat.global.get_primary_selection(),
        );
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

impl Global for ZwlrDataControlManagerV1Global {
    fn version(&self) -> u32 {
        2
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_DATA_CONTROL_MANAGER
    }
}
