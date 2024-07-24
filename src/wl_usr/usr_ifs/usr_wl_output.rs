use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{wl_output::*, WlOutputId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWlOutput {
    pub id: WlOutputId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlOutputOwner>>>,
    pub version: Version,
}

pub trait UsrWlOutputOwner {
    fn geometry(&self, ev: &Geometry) {
        let _ = ev;
    }

    fn mode(&self, ev: &Mode) {
        let _ = ev;
    }

    fn done(&self) {}

    fn scale(&self, ev: &Scale) {
        let _ = ev;
    }

    fn name(&self, ev: &Name) {
        let _ = ev;
    }

    fn description(&self, ev: &Description) {
        let _ = ev;
    }
}

impl WlOutputEventHandler for UsrWlOutput {
    type Error = Infallible;

    fn geometry(&self, ev: Geometry<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.geometry(&ev);
        }
        Ok(())
    }

    fn mode(&self, ev: Mode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.mode(&ev);
        }
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.done();
        }
        Ok(())
    }

    fn scale(&self, ev: Scale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.scale(&ev);
        }
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.name(&ev);
        }
        Ok(())
    }

    fn description(&self, ev: Description<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.description(&ev);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlOutput = WlOutput;
    version = self.version;
}

impl UsrObject for UsrWlOutput {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
