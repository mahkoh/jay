use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_toplevel::*, JayToplevelId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrJayToplevel {
    pub id: JayToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayToplevelOwner>>>,
}

pub trait UsrJayToplevelOwner {
    fn destroyed(&self) {}
}

impl UsrJayToplevel {
    fn destroyed(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Destroyed = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }
}

usr_object_base! {
    UsrJayToplevel, JayToplevel;

    DESTROYED => destroyed,
}

impl UsrObject for UsrJayToplevel {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
