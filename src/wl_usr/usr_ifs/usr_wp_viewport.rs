use crate::fixed::Fixed;
use crate::object::Version;
use crate::wire::WpViewportId;
use crate::wire::wp_viewport::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWpViewport {
    pub id: WpViewportId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWpViewport {
    #[expect(dead_code)]
    pub fn set_source(&self, x: Fixed, y: Fixed, width: Fixed, height: Fixed) {
        self.con.request(SetSource {
            self_id: self.id,
            x,
            y,
            width,
            height,
        });
    }

    pub fn set_destination(&self, width: i32, height: i32) {
        self.con.request(SetDestination {
            self_id: self.id,
            width,
            height,
        });
    }
}

impl WpViewportEventHandler for UsrWpViewport {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWpViewport = WpViewport;
    version = self.version;
}

impl UsrObject for UsrWpViewport {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
