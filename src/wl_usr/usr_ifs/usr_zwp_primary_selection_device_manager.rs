use crate::object::Version;
use crate::wire::ZwpPrimarySelectionDeviceManagerV1Id;
use crate::wire::zwp_primary_selection_device_manager_v1::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

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
