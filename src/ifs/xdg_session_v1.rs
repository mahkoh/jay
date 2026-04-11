use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        sm::{Session, SessionGetStatus, SessionManagementError, SessionName, SessionOwner},
        utils::{clonecell::CloneCell, linkedlist::LinkedNode},
        wire::{
            XdgSessionV1Id,
            xdg_session_v1::{
                AddToplevel, Created, Destroy, Remove, RemoveToplevel, Replaced, RestoreToplevel,
                Restored, XdgSessionV1RequestHandler,
            },
        },
    },
    std::{cell::RefCell, error::Error, rc::Rc},
    thiserror::Error,
};

pub struct XdgSessionV1 {
    pub id: XdgSessionV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub session: CloneCell<Option<Rc<Session>>>,
    pub name: SessionName,
    pub link: RefCell<Option<LinkedNode<Rc<Self>>>>,
}

impl XdgSessionV1 {
    fn disown_(&self, to_peer: bool) {
        if let Some(session) = self.session.take() {
            if to_peer {
                session.disown_from_peer();
            }
        }
        if self.link.take().is_some() {
            self.client.num_live_sessions.fetch_sub(1);
        }
    }

    pub fn disown_to_peer(&self) {
        self.disown_(true);
    }

    pub fn bump(&self) {
        if let Some(link) = &*self.link.borrow() {
            self.client.live_sessions.add_last_existing(link);
        }
    }
}

impl SessionOwner for XdgSessionV1 {
    fn client(&self) -> Option<&Rc<Client>> {
        Some(&self.client)
    }

    fn loaded(&self, status: SessionGetStatus) {
        match status {
            SessionGetStatus::Created => self.send_created(self.name),
            SessionGetStatus::Restored => self.send_restored(),
        }
    }

    fn error(&self, e: &dyn Error) {
        self.client.error(e);
    }

    fn disown_from_peer(&self, replaced: bool) {
        if replaced {
            self.send_replaced();
        }
        self.disown_(false);
    }
}

impl XdgSessionV1 {
    pub fn send_created(&self, name: SessionName) {
        self.client.event(Created {
            self_id: self.id,
            session_id: &name.to_string(),
        });
    }

    pub fn send_restored(&self) {
        self.client.event(Restored { self_id: self.id });
    }

    pub fn send_replaced(&self) {
        self.client.event(Replaced { self_id: self.id });
    }
}

impl XdgSessionV1RequestHandler for XdgSessionV1 {
    type Error = XdgSessionV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.disown_to_peer();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn remove(&self, _req: Remove, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(session) = self.session.get() {
            session.remove();
        }
        self.disown_from_peer(false);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn add_toplevel(&self, req: AddToplevel<'_>, slf: &Rc<Self>) -> Result<(), Self::Error> {
        slf.create_session(req.id, req.toplevel, false, req.name)
    }

    fn restore_toplevel(
        &self,
        req: RestoreToplevel<'_>,
        slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        slf.create_session(req.id, req.toplevel, true, req.name)
    }

    fn remove_toplevel(&self, req: RemoveToplevel<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.bump();
        if let Some(session) = self.session.get() {
            session.remove_toplevel(req.name);
        }
        Ok(())
    }
}

object_base! {
    self = XdgSessionV1;
    version = self.version;
}

impl Object for XdgSessionV1 {
    fn break_loops(&self) {
        self.disown_to_peer();
    }
}

simple_add_obj!(XdgSessionV1);

#[derive(Debug, Error)]
pub enum XdgSessionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The toplevel is already part of a session")]
    HasSession,
    #[error("The toplevel has already been committed")]
    Committed,
    #[error(transparent)]
    SessionManagementError(#[from] SessionManagementError),
}
efrom!(XdgSessionV1Error, ClientError);
