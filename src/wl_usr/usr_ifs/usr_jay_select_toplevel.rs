use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_select_toplevel::*, JaySelectToplevelId},
        wl_usr::{usr_ifs::usr_jay_toplevel::UsrJayToplevel, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct UsrJaySelectToplevel {
    pub id: JaySelectToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySelectToplevelOwner>>>,
}

pub trait UsrJaySelectToplevelOwner {
    fn done(&self, toplevel: Option<Rc<UsrJayToplevel>>);
}

impl UsrJaySelectToplevel {
    fn done(&self, parser: MsgParser<'_, '_>) -> Result<(), UsrJaySelectToplevelError> {
        let ev: Done = self.con.parse(self, parser)?;
        let tl = if ev.id.is_none() {
            None
        } else {
            let tl = Rc::new(UsrJayToplevel {
                id: ev.id,
                con: self.con.clone(),
                owner: Default::default(),
            });
            self.con.add_object(tl.clone());
            Some(tl)
        };
        match self.owner.get() {
            Some(owner) => owner.done(tl),
            _ => {
                if let Some(tl) = tl {
                    self.con.remove_obj(&*tl);
                }
            }
        }
        self.con.remove_obj(self);
        Ok(())
    }
}

usr_object_base! {
    UsrJaySelectToplevel, JaySelectToplevel;

    DONE => done,
}

impl UsrObject for UsrJaySelectToplevel {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}

#[derive(Debug, Error)]
pub enum UsrJaySelectToplevelError {
    #[error("Parsing failed")]
    MsgParserError(#[from] MsgParserError),
}
