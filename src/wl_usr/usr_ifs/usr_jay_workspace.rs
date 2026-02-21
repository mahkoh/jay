use {
    crate::{
        globals::GlobalName,
        object::Version,
        utils::clonecell::CloneCell,
        wire::{JayWorkspaceId, jay_workspace::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{
        cell::{Cell, RefCell},
        convert::Infallible,
        rc::Rc,
    },
};

pub struct UsrJayWorkspace {
    pub id: JayWorkspaceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayWorkspaceOwner>>>,
    pub version: Version,
    pub linear_id: Cell<u32>,
    pub output: Cell<GlobalName>,
    pub name: RefCell<Option<String>>,
}

pub trait UsrJayWorkspaceOwner {
    fn destroyed(&self, ws: &UsrJayWorkspace) {
        let _ = ws;
    }

    fn done(&self, ws: &Rc<UsrJayWorkspace>) {
        let _ = ws;
    }

    fn output(self: Rc<Self>, ev: &Output) {
        let _ = ev;
    }

    fn visible(&self, visible: bool) {
        let _ = visible;
    }
}

impl JayWorkspaceEventHandler for UsrJayWorkspace {
    type Error = Infallible;

    fn linear_id(&self, ev: LinearId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.linear_id.set(ev.linear_id);
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.name.borrow_mut() = Some(ev.name.to_string());
        Ok(())
    }

    fn destroyed(&self, _ev: Destroyed, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed(slf);
        }
        Ok(())
    }

    fn done(&self, _ev: Done, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.done(slf);
        }
        Ok(())
    }

    fn output(&self, ev: Output, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.output.set(GlobalName::from_raw(ev.global_name));
        if let Some(owner) = self.owner.get() {
            owner.output(&ev);
        }
        Ok(())
    }

    fn visible(&self, ev: Visible, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.visible(ev.visible != 0);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayWorkspace = JayWorkspace;
    version = self.version;
}

impl UsrObject for UsrJayWorkspace {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
