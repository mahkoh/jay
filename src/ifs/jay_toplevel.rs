use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        tree::ToplevelNode,
        wire::{jay_toplevel::*, JayToplevelId},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JayToplevel {
    pub id: JayToplevelId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub toplevel: Rc<dyn ToplevelNode>,
    pub destroyed: Cell<bool>,
}

impl JayToplevel {
    fn detach(&self) {
        self.destroyed.set(true);
        self.toplevel
            .tl_data()
            .jay_toplevels
            .remove(&(self.client.id, self.id));
    }

    pub fn destroy(&self) {
        self.destroyed.set(true);
        self.send_destroyed();
    }

    fn send_destroyed(&self) {
        self.client.event(Destroyed { self_id: self.id });
    }
}

impl JayToplevelRequestHandler for JayToplevel {
    type Error = JayToplevelError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayToplevel;
    version = Version(1);
}

impl Object for JayToplevel {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(JayToplevel, JayToplevelId, jay_toplevels);

#[derive(Debug, Error)]
pub enum JayToplevelError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayToplevelError, ClientError);
