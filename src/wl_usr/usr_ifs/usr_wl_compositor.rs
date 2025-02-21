use {
    crate::{
        object::Version,
        wire::{
            WlCompositorId,
            wl_compositor::{CreateSurface, WlCompositorEventHandler},
        },
        wl_usr::{UsrCon, usr_ifs::usr_wl_surface::UsrWlSurface, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

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
