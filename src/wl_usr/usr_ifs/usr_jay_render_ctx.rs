use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{jay_render_ctx::*, JayRenderCtxId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrJayRenderCtx {
    pub id: JayRenderCtxId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayRenderCtxOwner>>>,
    pub version: Version,
}

pub trait UsrJayRenderCtxOwner {
    fn no_device(&self) {}
    fn device(&self, fd: Rc<OwnedFd>) {
        let _ = fd;
    }
}

impl JayRenderCtxEventHandler for UsrJayRenderCtx {
    type Error = Infallible;

    fn no_device(&self, _ev: NoDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.no_device();
        }
        Ok(())
    }

    fn device(&self, ev: Device, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.device(ev.fd);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayRenderCtx = JayRenderCtx;
    version = self.version;
}

impl UsrObject for UsrJayRenderCtx {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
