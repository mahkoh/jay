use {
    crate::{
        object::Version,
        wire::{WlDataDeviceManagerId, wl_data_device_manager::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWlDataDeviceManager {
    pub id: WlDataDeviceManagerId,
    #[expect(dead_code)]
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWlDataDeviceManager {}

impl WlDataDeviceManagerEventHandler for UsrWlDataDeviceManager {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWlDataDeviceManager = WlDataDeviceManager;
    version = self.version;
}

impl UsrObject for UsrWlDataDeviceManager {
    fn destroy(&self) {
        // nothing
    }
}
