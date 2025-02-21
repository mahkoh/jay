use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{JayOutputId, jay_output::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJayOutput {
    pub id: JayOutputId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayOutputOwner>>>,
    pub version: Version,
}

pub trait UsrJayOutputOwner {
    fn linear_id(self: Rc<Self>, ev: &LinearId) {
        let _ = ev;
    }

    fn destroyed(&self) {}
}

impl JayOutputEventHandler for UsrJayOutput {
    type Error = Infallible;

    fn linear_id(&self, ev: LinearId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.linear_id(&ev);
        }
        Ok(())
    }

    fn unused(&self, _ev: Unused, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unimplemented!();
    }

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayOutput = JayOutput;
    version = self.version;
}

impl UsrObject for UsrJayOutput {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
