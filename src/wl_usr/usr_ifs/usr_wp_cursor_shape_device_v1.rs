use {
    crate::{
        cursor::KnownCursor,
        object::Version,
        wire::{WpCursorShapeDeviceV1Id, wp_cursor_shape_device_v1::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWpCursorShapeDeviceV1 {
    pub id: WpCursorShapeDeviceV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWpCursorShapeDeviceV1 {
    pub fn set_shape(&self, serial: u32, cursor: KnownCursor) {
        self.con.request(SetShape {
            self_id: self.id,
            serial,
            shape: cursor.to_shape(),
        });
    }
}

impl WpCursorShapeDeviceV1EventHandler for UsrWpCursorShapeDeviceV1 {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWpCursorShapeDeviceV1 = WpCursorShapeDeviceV1;
    version = self.version;
}

impl UsrObject for UsrWpCursorShapeDeviceV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
