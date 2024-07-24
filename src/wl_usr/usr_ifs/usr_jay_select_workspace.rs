use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{jay_select_workspace::*, JaySelectWorkspaceId},
        wl_usr::{usr_ifs::usr_jay_workspace::UsrJayWorkspace, usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJaySelectWorkspace {
    pub id: JaySelectWorkspaceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySelectWorkspaceOwner>>>,
    pub version: Version,
}

pub trait UsrJaySelectWorkspaceOwner {
    fn done(&self, output: u32, ws: Option<Rc<UsrJayWorkspace>>);
}

impl JaySelectWorkspaceEventHandler for UsrJaySelectWorkspace {
    type Error = Infallible;

    fn cancelled(&self, _ev: Cancelled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.done(0, None);
        }
        self.con.remove_obj(self);
        Ok(())
    }

    fn selected(&self, ev: Selected, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = Rc::new(UsrJayWorkspace {
            id: ev.id,
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
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
    self = UsrJaySelectWorkspace = JaySelectWorkspace;
    version = self.version;
}

impl UsrObject for UsrJaySelectWorkspace {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
