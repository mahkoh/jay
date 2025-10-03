use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        tree::ToplevelNode,
        wire::{JayToplevelId, jay_toplevel::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub const ID_SINCE: Version = Version(12);
pub const CLIENT_ID_SINCE: Version = Version(18);

pub struct JayToplevel {
    pub id: JayToplevelId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub toplevel: Rc<dyn ToplevelNode>,
    pub destroyed: Cell<bool>,
    pub version: Version,
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

    pub fn send_id(&self) {
        let s = self.toplevel.tl_data().identifier.get().to_string();
        self.client.event(Id {
            self_id: self.id,
            id: &s,
        })
    }

    pub fn send_client_id(&self) {
        if let Some(cl) = &self.toplevel.tl_data().client {
            self.client.event(ClientId {
                self_id: self.id,
                id: cl.id.raw(),
            })
        }
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id })
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
    fn break_loops(self: Rc<Self>) {
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
