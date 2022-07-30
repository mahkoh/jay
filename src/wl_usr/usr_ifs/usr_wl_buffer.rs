use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wl_buffer::*, WlBufferId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlBuffer {
    pub id: WlBufferId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlBufferOwner>>>,
}

pub trait UsrWlBufferOwner {
    fn release(&self) {}
}

impl UsrWlBuffer {
    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Release = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.release();
        }
        Ok(())
    }
}

usr_object_base! {
    UsrWlBuffer, WlBuffer;

    RELEASE => release,
}

impl UsrObject for UsrWlBuffer {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
