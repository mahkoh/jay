use {
    crate::{
        object::Version,
        wire::{WlDataDeviceManagerId, wl_data_device_manager::*},
        wl_usr::{
            UsrCon,
            usr_ifs::{
                usr_wl_data_device::UsrWlDataDevice, usr_wl_data_source::UsrWlDataSource,
                usr_wl_seat::UsrWlSeat,
            },
            usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWlDataDeviceManager {
    pub id: WlDataDeviceManagerId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWlDataDeviceManager {
    pub fn create_data_source(&self) -> Rc<UsrWlDataSource> {
        let obj = Rc::new(UsrWlDataSource {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(CreateDataSource {
            self_id: self.id,
            id: obj.id,
        });
        self.con.add_object(obj.clone());
        obj
    }

    pub fn get_data_device(&self, seat: &UsrWlSeat) -> Rc<UsrWlDataDevice> {
        let obj = Rc::new(UsrWlDataDevice {
            id: self.con.id(),
            con: self.con.clone(),
            version: self.version,
            offer: Default::default(),
            selection: Default::default(),
        });
        self.con.request(GetDataDevice {
            self_id: self.id,
            id: obj.id,
            seat: seat.id,
        });
        self.con.add_object(obj.clone());
        obj
    }
}

impl WlDataDeviceManagerEventHandler for UsrWlDataDeviceManager {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWlDataDeviceManager = WlDataDeviceManager;
    version = self.version;
}

impl UsrObject for UsrWlDataDeviceManager {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }
}
