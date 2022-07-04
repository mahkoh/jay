use {
    crate::{
        format::Format,
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
    pub width: i32,
    pub height: i32,
    pub stride: Option<i32>,
    pub format: &'static Format,
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

impl Drop for UsrWlBuffer {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrWlBuffer, WlBuffer;

    RELEASE => release,
}

impl UsrObject for UsrWlBuffer {
    fn break_loops(&self) {
        self.owner.take();
    }
}
