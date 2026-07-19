use crate::object::Version;
use crate::wire::WpCursorShapeManagerV1Id;
use crate::wire::wp_cursor_shape_manager_v1::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_wl_pointer::UsrWlPointer;
use crate::wl_usr::usr_ifs::usr_wp_cursor_shape_device_v1::UsrWpCursorShapeDeviceV1;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWpCursorShapeManagerV1 {
    pub id: WpCursorShapeManagerV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWpCursorShapeManagerV1 {
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
