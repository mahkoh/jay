use {
    crate::{
        object::Version,
        wire::{XdgWmBaseId, xdg_wm_base::*},
        wl_usr::{
            UsrCon,
            usr_ifs::{usr_wl_surface::UsrWlSurface, usr_xdg_surface::UsrXdgSurface},
            usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrXdgWmBase {
    pub id: XdgWmBaseId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrXdgWmBase {
    pub fn get_xdg_surface(&self, surface: &UsrWlSurface) -> Rc<UsrXdgSurface> {
        let obj = Rc::new(UsrXdgSurface {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(GetXdgSurface {
            self_id: self.id,
            id: obj.id,
            surface: surface.id,
        });
        self.con.add_object(obj.clone());
        obj
    }
}

impl XdgWmBaseEventHandler for UsrXdgWmBase {
    type Error = Infallible;

    fn ping(&self, ev: Ping, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.con.request(Pong {
            self_id: self.id,
            serial: ev.serial,
        });
        Ok(())
    }
}

usr_object_base! {
    self = UsrXdgWmBase = XdgWmBase;
    version = self.version;
}

impl UsrObject for UsrXdgWmBase {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
