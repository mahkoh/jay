use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{jay_workspace::*, JayWorkspaceId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJayWorkspace {
    pub id: JayWorkspaceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayWorkspaceOwner>>>,
    pub version: Version,
}

pub trait UsrJayWorkspaceOwner {
    fn linear_id(self: Rc<Self>, ev: &LinearId) {
        let _ = ev;
    }

    fn name(&self, ev: &Name) {
        let _ = ev;
    }

    fn destroyed(&self) {}

    fn done(&self) {}

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
        if let Some(owner) = self.owner.get() {
            owner.linear_id(&ev);
        }
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.name(&ev);
        }
        Ok(())
    }

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.destroyed();
        }
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.done();
        }
        Ok(())
    }

    fn output(&self, ev: Output, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
