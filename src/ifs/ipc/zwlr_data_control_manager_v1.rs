use {
    crate::{
        client::{Client, ClientCaps, ClientError, CAP_DATA_CONTROL_MANAGER},
        globals::{Global, GlobalName},
        ifs::ipc::{
            zwlr_data_control_device_v1::{ZwlrDataControlDeviceV1, PRIMARY_SELECTION_SINCE},
            zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwlr_data_control_manager_v1::*, ZwlrDataControlManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwlrDataControlManagerV1Global {
    name: GlobalName,
}

pub struct ZwlrDataControlManagerV1 {
    pub id: ZwlrDataControlManagerV1Id,
    pub client: Rc<Client>,
    pub version: Version,
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
    ) -> Result<(), ZwlrDataControlManagerV1Error> {
        let obj = Rc::new(ZwlrDataControlManagerV1 {
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

impl ZwlrDataControlManagerV1RequestHandler for ZwlrDataControlManagerV1 {
    type Error = ZwlrDataControlManagerV1Error;

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
        self.client.add_client_obj(&res)?;
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
        seat.global.add_wlr_device(&dev);
        self.client.add_client_obj(&dev)?;
        match seat.global.get_selection() {
            Some(s) => s.offer_to_wlr_device(&dev),
            _ => dev.send_selection(None),
        }
        if self.version >= PRIMARY_SELECTION_SINCE {
            match seat.global.get_primary_selection() {
                Some(s) => s.offer_to_wlr_device(&dev),
                _ => dev.send_primary_selection(None),
            }
        }
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(
    ZwlrDataControlManagerV1Global,
    ZwlrDataControlManagerV1,
    ZwlrDataControlManagerV1Error
);

impl Global for ZwlrDataControlManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        2
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_DATA_CONTROL_MANAGER
    }
}

simple_add_global!(ZwlrDataControlManagerV1Global);

object_base! {
    self = ZwlrDataControlManagerV1;
    version = self.version;
}

impl Object for ZwlrDataControlManagerV1 {}

simple_add_obj!(ZwlrDataControlManagerV1);

#[derive(Debug, Error)]
pub enum ZwlrDataControlManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrDataControlManagerV1Error, ClientError);
