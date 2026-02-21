use {
    crate::{
        object::Version,
        wire::{WpCursorShapeManagerV1Id, wp_cursor_shape_manager_v1::*},
        wl_usr::{
            UsrCon,
            usr_ifs::{
                usr_wl_pointer::UsrWlPointer,
                usr_wp_cursor_shape_device_v1::UsrWpCursorShapeDeviceV1,
            },
            usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWpCursorShapeManagerV1 {
    pub id: WpCursorShapeManagerV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWpCursorShapeManagerV1 {
    #[expect(dead_code)]
    pub fn get_pointer(&self, pointer: &UsrWlPointer) -> Rc<UsrWpCursorShapeDeviceV1> {
        let obj = Rc::new(UsrWpCursorShapeDeviceV1 {
            id: self.con.id(),
            con: self.con.clone(),
            version: self.version,
        });
        self.con.request(GetPointer {
            self_id: self.id,
            cursor_shape_device: obj.id,
            pointer: pointer.id,
        });
        self.con.add_object(obj.clone());
        obj
    }
}

impl WpCursorShapeManagerV1EventHandler for UsrWpCursorShapeManagerV1 {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWpCursorShapeManagerV1 = WpCursorShapeManagerV1;
    version = self.version;
}

impl UsrObject for UsrWpCursorShapeManagerV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
