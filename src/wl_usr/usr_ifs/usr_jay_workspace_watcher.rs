use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{JayWorkspaceWatcherId, jay_workspace_watcher::*},
        wl_usr::{UsrCon, usr_ifs::usr_jay_workspace::UsrJayWorkspace, usr_object::UsrObject},
    },
    std::{convert::Infallible, ops::Deref, rc::Rc},
};

pub struct UsrJayWorkspaceWatcher {
    pub id: JayWorkspaceWatcherId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayWorkspaceWatcherOwner>>>,
    pub version: Version,
}

pub trait UsrJayWorkspaceWatcherOwner {
    fn new(self: Rc<Self>, ev: Rc<UsrJayWorkspace>, linear_id: u32) {
        let _ = linear_id;
        ev.con.remove_obj(ev.deref());
    }
}

impl JayWorkspaceWatcherEventHandler for UsrJayWorkspaceWatcher {
    type Error = Infallible;

    fn new(&self, ev: New, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let jw = Rc::new(UsrJayWorkspace {
            id: ev.id,
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
            linear_id: Default::default(),
            output: Default::default(),
            name: Default::default(),
        });
        self.con.add_object(jw.clone());
        if let Some(owner) = self.owner.get() {
            owner.new(jw, ev.linear_id);
        } else {
            self.con.remove_obj(jw.deref());
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayWorkspaceWatcher = JayWorkspaceWatcher;
    version = self.version;
}

impl UsrObject for UsrJayWorkspaceWatcher {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
