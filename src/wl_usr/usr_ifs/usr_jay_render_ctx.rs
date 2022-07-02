use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_render_ctx::*, JayRenderCtxId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct UsrJayRenderCtx {
    pub id: JayRenderCtxId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayRenderCtxOwner>>>,
}

pub trait UsrJayRenderCtxOwner {
    fn no_device(&self) {}
    fn device(&self, fd: Rc<OwnedFd>) {
        let _ = fd;
    }
}

impl UsrJayRenderCtx {
    fn no_device(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: NoDevice = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.no_device();
        }
        Ok(())
    }

    fn device(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Device = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.device(ev.fd);
        }
        Ok(())
    }
}

impl Drop for UsrJayRenderCtx {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrJayRenderCtx, JayRenderCtx;

    NO_DEVICE => no_device,
    DEVICE => device,
}

impl UsrObject for UsrJayRenderCtx {
    fn break_loops(&self) {
        self.owner.take();
    }
}
