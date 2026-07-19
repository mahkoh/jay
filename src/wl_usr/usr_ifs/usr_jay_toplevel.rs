use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::JayToplevelId;
use crate::wire::jay_toplevel::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrJayToplevel {
    pub id: JayToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayToplevelOwner>>>,
    pub version: Version,
    pub toplevel_id: RefCell<Option<String>>,
}

pub trait UsrJayToplevelOwner {
    fn destroyed(&self) {}
    fn done(&self, tl: &Rc<UsrJayToplevel>);
}

impl JayToplevelEventHandler for UsrJayToplevel {
    type Error = Infallible;

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }

    fn id_(&self, ev: Id<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.toplevel_id.borrow_mut() = Some(ev.id.to_string());
        Ok(())
    }

    fn client_id(&self, _ev: ClientId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn done(&self, _ev: Done, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.done(slf);
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
