use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::XdgSurfaceId;
use crate::wire::xdg_surface::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_xdg_toplevel::UsrXdgToplevel;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrXdgSurface {
    pub id: XdgSurfaceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrXdgSurfaceOwner>>>,
    pub version: Version,
}

pub trait UsrXdgSurfaceOwner {
    fn configure(&self) {
        // nothing
    }
}

impl UsrXdgSurface {
    pub fn get_toplevel(&self) -> Rc<UsrXdgToplevel> {
        let obj = Rc::new(UsrXdgToplevel {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(GetToplevel {
            self_id: self.id,
            id: obj.id,
        });
        self.con.add_object(obj.clone());
        obj
    }
}

impl XdgSurfaceEventHandler for UsrXdgSurface {
    type Error = Infallible;

    fn configure(&self, ev: Configure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.con.request(AckConfigure {
            self_id: self.id,
            serial: ev.serial,
        });
        if let Some(owner) = self.owner.get() {
            owner.configure();
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrXdgSurface = XdgSurface;
    version = self.version;
}

impl UsrObject for UsrXdgSurface {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
