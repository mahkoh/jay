use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{WlOutputId, XdgToplevelId, xdg_toplevel::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrXdgToplevel {
    pub id: XdgToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrXdgToplevelOwner>>>,
    pub version: Version,
}

impl UsrXdgToplevel {
    pub fn set_title(&self, title: &str) {
        self.con.request(SetTitle {
            self_id: self.id,
            title,
        });
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        match fullscreen {
            true => {
                self.con.request(SetFullscreen {
                    self_id: self.id,
                    output: WlOutputId::NONE,
                });
            }
            false => {
                self.con.request(UnsetFullscreen { self_id: self.id });
            }
        }
    }
}

pub trait UsrXdgToplevelOwner {
    fn configure(&self, width: i32, height: i32) {
        let _ = width;
        let _ = height;
    }

    fn close(&self) {
        // nothing
    }
}

impl UsrXdgToplevel {}

impl XdgToplevelEventHandler for UsrXdgToplevel {
    type Error = Infallible;

    fn configure(&self, ev: Configure<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.configure(ev.width, ev.height);
        }
        Ok(())
    }

    fn close(&self, _ev: Close, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.close();
        }
        Ok(())
    }

    fn configure_bounds(&self, _ev: ConfigureBounds, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn wm_capabilities(&self, _ev: WmCapabilities<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrXdgToplevel = XdgToplevel;
    version = self.version;
}

impl UsrObject for UsrXdgToplevel {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
