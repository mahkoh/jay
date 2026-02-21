use {
    crate::{
        object::Version,
        wire::{ZwpPrimarySelectionDeviceManagerV1Id, zwp_primary_selection_device_manager_v1::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

#[expect(dead_code)]
pub struct UsrZwpPrimarySelectionDeviceManagerV1 {
    pub id: ZwpPrimarySelectionDeviceManagerV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrZwpPrimarySelectionDeviceManagerV1 {}

impl ZwpPrimarySelectionDeviceManagerV1EventHandler for UsrZwpPrimarySelectionDeviceManagerV1 {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrZwpPrimarySelectionDeviceManagerV1 = ZwpPrimarySelectionDeviceManagerV1;
    version = self.version;
}

impl UsrObject for UsrZwpPrimarySelectionDeviceManagerV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
