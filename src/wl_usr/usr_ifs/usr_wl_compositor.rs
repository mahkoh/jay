use {
    crate::{
        wire::{wl_compositor::CreateSurface, WlCompositorId},
        wl_usr::{usr_ifs::usr_wl_surface::UsrWlSurface, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlCompositor {
    pub id: WlCompositorId,
    pub con: Rc<UsrCon>,
}

impl UsrWlCompositor {
    pub fn create_surface(&self) -> Rc<UsrWlSurface> {
        let sfc = Rc::new(UsrWlSurface {
            id: self.con.id(),
            con: self.con.clone(),
        });
        self.con.request(CreateSurface {
            self_id: self.id,
            id: sfc.id,
        });
        self.con.add_object(sfc.clone());
        sfc
    }
}

usr_object_base! {
    UsrWlCompositor, WlCompositor;
}

impl UsrObject for UsrWlCompositor {
    fn destroy(&self) {
        // nothing
    }
}
