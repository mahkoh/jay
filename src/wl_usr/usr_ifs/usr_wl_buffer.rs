use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::WlBufferId;
use crate::wire::wl_buffer::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWlBuffer {
    pub id: WlBufferId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlBufferOwner>>>,
    pub version: Version,
}

pub trait UsrWlBufferOwner {
    fn release(&self) {}
}

impl WlBufferEventHandler for UsrWlBuffer {
    type Error = Infallible;

    fn release(&self, _ev: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.release();
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlBuffer = WlBuffer;
    version = self.version;
}

impl UsrObject for UsrWlBuffer {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
