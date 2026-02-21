use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{XdgSurfaceId, xdg_surface::*},
        wl_usr::{UsrCon, usr_ifs::usr_xdg_toplevel::UsrXdgToplevel, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

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

    fn configure(&self, _ev: Configure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
