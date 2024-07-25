use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{jay_toplevel::*, JayToplevelId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJayToplevel {
    pub id: JayToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayToplevelOwner>>>,
    pub version: Version,
}

pub trait UsrJayToplevelOwner {
    fn destroyed(&self) {}
}

impl JayToplevelEventHandler for UsrJayToplevel {
    type Error = Infallible;

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayToplevel = JayToplevel;
    version = self.version;
}

impl UsrObject for UsrJayToplevel {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
