use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{jay_workspace_watcher::*, JayWorkspaceWatcherId},
        wl_usr::{usr_ifs::usr_jay_workspace::UsrJayWorkspace, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrJayWorkspaceWatcher {
    pub id: JayWorkspaceWatcherId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayWorkspaceWatcherOwner>>>,
}

pub trait UsrJayWorkspaceWatcherOwner {
    fn new(&self, ev: Rc<UsrJayWorkspace>) {
        let _ = ev;
    }
}

impl UsrJayWorkspaceWatcher {
    fn new(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: New = self.con.parse(self, parser)?;
        let jw = UsrJayWorkspace {
            id: ev.id,
            con: self.con.clone(),
            owner: Default::default(),
        };
        if let Some(owner) = self.owner.get() {
            owner.new(Rc::new(jw));
        }
        Ok(())
    }
}

impl Drop for UsrJayWorkspaceWatcher {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrJayWorkspaceWatcher, JayWorkspaceWatcher;

    NEW => new,
}

impl UsrObject for UsrJayWorkspaceWatcher {
    fn break_loops(&self) {
        self.owner.set(None);
    }
}
