use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{wl_buffer::*, WlBufferId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

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
