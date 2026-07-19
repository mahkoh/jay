use crate::object::Version;
use crate::wire::WpViewporterId;
use crate::wire::wp_viewporter::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_wl_surface::UsrWlSurface;
use crate::wl_usr::usr_ifs::usr_wp_viewport::UsrWpViewport;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWpViewporter {
    pub id: WpViewporterId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWpViewporter {
    pub fn get_viewport(&self, surface: &UsrWlSurface) -> Rc<UsrWpViewport> {
        let wv = Rc::new(UsrWpViewport {
            id: self.con.id(),
            con: self.con.clone(),
            version: self.version,
        });
        self.con.add_object(wv.clone());
        self.con.request(GetViewport {
            self_id: self.id,
            id: wv.id,
            surface: surface.id,
        });
        wv
    }
}

impl WpViewporterEventHandler for UsrWpViewporter {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWpViewporter = WpViewporter;
    version = self.version;
}

impl UsrObject for UsrWpViewporter {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
