use {
    crate::{
        client::{Client, ClientError, ClientId},
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            JayEiSessionId,
            jay_ei_session::{Created, Destroyed, Failed, JayEiSessionRequestHandler, Release},
        },
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct JayEiSession {
    pub id: JayEiSessionId,
    pub client: Rc<Client>,
    pub ei_client_id: Option<ClientId>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayEiSession {
    pub fn send_created(&self, fd: &Rc<OwnedFd>) {
        self.client.event(Created {
            self_id: self.id,
            fd: fd.clone(),
        });
    }

    pub fn send_failed(&self, reason: &str) {
        self.client.event(Failed {
            self_id: self.id,
            reason,
        });
    }

    fn send_destroyed(&self) {
        self.client.event(Destroyed { self_id: self.id });
    }

    fn kill(&self, send_destroyed: bool) {
        if let Some(id) = self.ei_client_id {
            self.client.state.ei_clients.shutdown(id);
        }
        if send_destroyed {
            self.send_destroyed();
        }
    }
}

impl JayEiSessionRequestHandler for JayEiSession {
    type Error = JayEiSessionError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.kill(false);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayEiSession;
    version = self.version;
}

impl Object for JayEiSession {
    fn break_loops(&self) {
        self.kill(false);
    }
}

simple_add_obj!(JayEiSession);

#[derive(Debug, Error)]
pub enum JayEiSessionError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayEiSessionError, ClientError);
