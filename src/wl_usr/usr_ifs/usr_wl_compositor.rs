use crate::object::Version;
use crate::wire::WlCompositorId;
use crate::wire::wl_compositor::CreateSurface;
use crate::wire::wl_compositor::WlCompositorEventHandler;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_wl_surface::UsrWlSurface;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWlCompositor {
    pub id: WlCompositorId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWlCompositor {
    pub fn create_surface(&self) -> Rc<UsrWlSurface> {
        let sfc = Rc::new(UsrWlSurface {
            id: self.con.id(),
            con: self.con.clone(),
            version: self.version,
        });
        self.con.request(CreateSurface {
            self_id: self.id,
            id: sfc.id,
        });
        self.con.add_object(sfc.clone());
        sfc
    }
}

impl WlCompositorEventHandler for UsrWlCompositor {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWlCompositor = WlCompositor;
    version = self.version;
}

impl UsrObject for UsrWlCompositor {
    fn destroy(&self) {
        // nothing
    }
}
