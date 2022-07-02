use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_output::*, JayOutputId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrJayOutput {
    pub id: JayOutputId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayOutputOwner>>>,
}

pub trait UsrJayOutputOwner {
    fn linear_id(self: Rc<Self>, ev: &LinearId) {
        let _ = ev;
    }

    fn destroyed(&self) {}
}

impl UsrJayOutput {
    fn linear_id(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: LinearId = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.linear_id(&ev);
        }
        Ok(())
    }

    fn destroyed(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Destroyed = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }
}

impl Drop for UsrJayOutput {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrJayOutput, JayOutput;

    LINEAR_ID => linear_id,
    DESTROYED => destroyed,
}

impl UsrObject for UsrJayOutput {
    fn break_loops(&self) {
        self.owner.set(None);
    }
}
