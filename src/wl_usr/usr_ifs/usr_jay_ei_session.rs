use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::JayEiSessionId;
use crate::wire::jay_ei_session::Created;
use crate::wire::jay_ei_session::Destroyed;
use crate::wire::jay_ei_session::Failed;
use crate::wire::jay_ei_session::JayEiSessionEventHandler;
use crate::wire::jay_ei_session::Release;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct UsrJayEiSession {
    pub id: JayEiSessionId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayEiSessionOwner>>>,
    pub version: Version,
}

pub trait UsrJayEiSessionOwner {
    fn destroyed(&self) {}

    fn created(&self, fd: &Rc<OwnedFd>) {
        let _ = fd;
    }

    fn failed(&self, reason: &str) {
        let _ = reason;
    }
}

impl JayEiSessionEventHandler for UsrJayEiSession {
    type Error = Infallible;

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }

    fn created(&self, ev: Created, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.created(&ev.fd);
        }
        Ok(())
    }

    fn failed(&self, ev: Failed<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.failed(ev.reason);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayEiSession = JayEiSession;
    version = self.version;
}

impl UsrObject for UsrJayEiSession {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
