use {
    crate::{
        wire::{wp_viewporter::*, WpViewporterId},
        wl_usr::{
            usr_ifs::{usr_wl_surface::UsrWlSurface, usr_wp_viewport::UsrWpViewport},
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::rc::Rc,
};

pub struct UsrWpViewporter {
    pub id: WpViewporterId,
    pub con: Rc<UsrCon>,
}

impl UsrWpViewporter {
    pub fn get_viewport(&self, surface: &UsrWlSurface) -> Rc<UsrWpViewport> {
        let wv = Rc::new(UsrWpViewport {
            id: self.con.id(),
            con: self.con.clone(),
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

usr_object_base! {
    UsrWpViewporter, WpViewporter;
}

impl UsrObject for UsrWpViewporter {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
