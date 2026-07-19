use crate::globals::GlobalName;
use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::JaySelectWorkspaceId;
use crate::wire::jay_select_workspace::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_jay_workspace::UsrJayWorkspace;
use crate::wl_usr::usr_ifs::usr_jay_workspace::UsrJayWorkspaceOwner;
use crate::wl_usr::usr_object::UsrObject;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrJaySelectWorkspace {
    pub id: JaySelectWorkspaceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySelectWorkspaceOwner>>>,
    pub version: Version,
}

pub trait UsrJaySelectWorkspaceOwner {
    fn done(&self, output: GlobalName, ws: Option<Rc<UsrJayWorkspace>>);
}

impl JaySelectWorkspaceEventHandler for UsrJaySelectWorkspace {
    type Error = Infallible;

    fn cancelled(&self, _ev: Cancelled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.done(GlobalName::from_raw(0), None);
        }
        self.con.remove_obj(self);
        Ok(())
    }

    fn selected(&self, ev: Selected, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = Rc::new(UsrJayWorkspace {
            id: ev.id,
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
            linear_id: Default::default(),
            output: Cell::new(GlobalName::from_raw(0)),
            name: Default::default(),
        });
        self.con.add_object(tl.clone());
        tl.owner.set(Some(slf.clone()));
        Ok(())
    }
}

impl UsrJayWorkspaceOwner for UsrJaySelectWorkspace {
    fn done(&self, ws: &Rc<UsrJayWorkspace>) {
        ws.owner.take();
        match self.owner.get() {
            Some(owner) => owner.done(ws.output.get(), Some(ws.clone())),
            _ => self.con.remove_obj(&**ws),
        }
        self.con.remove_obj(self);
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
