use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_select_workspace::*, JaySelectWorkspaceId},
        wl_usr::{usr_ifs::usr_jay_workspace::UsrJayWorkspace, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct UsrJaySelectWorkspace {
    pub id: JaySelectWorkspaceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySelectWorkspaceOwner>>>,
}

pub trait UsrJaySelectWorkspaceOwner {
    fn done(&self, output: u32, ws: Option<Rc<UsrJayWorkspace>>);
}

impl UsrJaySelectWorkspace {
    fn cancelled(&self, parser: MsgParser<'_, '_>) -> Result<(), UsrJaySelectWorkspaceError> {
        let _ev: Cancelled = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.done(0, None);
        }
        self.con.remove_obj(self);
        Ok(())
    }

    fn selected(&self, parser: MsgParser<'_, '_>) -> Result<(), UsrJaySelectWorkspaceError> {
        let ev: Selected = self.con.parse(self, parser)?;
        let tl = Rc::new(UsrJayWorkspace {
            id: ev.id,
            con: self.con.clone(),
            owner: Default::default(),
        });
        self.con.add_object(tl.clone());
        match self.owner.get() {
            Some(owner) => owner.done(ev.output, Some(tl)),
            _ => self.con.remove_obj(&*tl),
        }
        self.con.remove_obj(self);
        Ok(())
    }
}

usr_object_base! {
    UsrJaySelectWorkspace, JaySelectWorkspace;

    CANCELLED => cancelled,
    SELECTED => selected,
}

impl UsrObject for UsrJaySelectWorkspace {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}

#[derive(Debug, Error)]
pub enum UsrJaySelectWorkspaceError {
    #[error("Parsing failed")]
    MsgParserError(#[from] MsgParserError),
}
