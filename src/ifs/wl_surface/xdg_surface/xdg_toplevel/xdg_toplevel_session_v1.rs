use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
            xdg_session_v1::{XdgSessionV1, XdgSessionV1Error},
        },
        leaks::Tracker,
        object::{Object, Version},
        sm::{SessionGetStatus, SessionManagementError, ToplevelSession, ToplevelSessionOwner},
        utils::clonecell::CloneCell,
        wire::{
            XdgToplevelId, XdgToplevelSessionV1Id,
            xdg_toplevel_session_v1::{
                Destroy, Rename, Restored, XdgToplevelSessionV1RequestHandler,
            },
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct XdgToplevelSessionV1 {
    id: XdgToplevelSessionV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
    parent: Rc<XdgSessionV1>,
    session: CloneCell<Option<Rc<ToplevelSession>>>,
    toplevel: CloneCell<Option<Rc<XdgToplevel>>>,
    restored: Cell<Option<Rc<Cell<bool>>>>,
}

impl XdgSessionV1 {
    pub fn create_session(
        self: &Rc<Self>,
        id: XdgToplevelSessionV1Id,
        toplevel: XdgToplevelId,
        restore: bool,
        name: &str,
    ) -> Result<(), XdgSessionV1Error> {
        self.bump();
        let toplevel = self.client.lookup(toplevel)?;
        if restore && toplevel.committed.get() {
            return Err(XdgSessionV1Error::Committed);
        }
        if toplevel.toplevel_data.session.is_some() {
            return Err(XdgSessionV1Error::HasSession);
        }
        let obj = Rc::new(XdgToplevelSessionV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            parent: self.clone(),
            session: Default::default(),
            toplevel: Default::default(),
            restored: Default::default(),
        });
        track!(self.client, &obj);
        self.client.add_client_obj(&obj)?;
        let Some(session) = self.session.get() else {
            return Ok(());
        };
        let (session, status) = session.get(name, obj.clone(), restore)?;
        toplevel.toplevel_data.set_session(&session, restore);
        obj.session.set(Some(session));
        obj.toplevel.set(Some(toplevel.clone()));
        if let Some(SessionGetStatus::Restored) = status {
            obj.send_restored();
        } else if restore {
            let restored = Rc::new(Cell::new(false));
            toplevel.xdg.pending().restored = Some(restored.clone());
            obj.restored.set(Some(restored));
        }
        Ok(())
    }
}

impl XdgToplevelSessionV1 {
    fn restored(&self) {
        let Some(restored) = self.restored.take() else {
            return;
        };
        restored.set(true);
        if let Some(toplevel) = self.toplevel.get() {
            toplevel.xdg.surface.commit_timeline.toplevel_restored();
        }
    }

    fn disown_(&self, to_peer: bool) {
        self.restored();
        if let Some(session) = self.session.take() {
            if to_peer {
                session.disown_from_peer();
            }
        }
        if let Some(tl) = self.toplevel.take() {
            tl.toplevel_data.session.take();
        }
    }

    fn disown_to_peer(&self) {
        self.disown_(true);
    }
}

impl ToplevelSessionOwner for XdgToplevelSessionV1 {
    fn disown_from_peer(self: Rc<Self>) {
        self.disown_(false);
    }

    fn loaded(&self, status: SessionGetStatus) {
        if let SessionGetStatus::Restored = status {
            self.send_restored();
        }
        self.restored();
    }
}

impl XdgToplevelSessionV1RequestHandler for XdgToplevelSessionV1 {
    type Error = XdgToplevelSessionV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.parent.bump();
        self.client.remove_obj(self)?;
        self.disown_to_peer();
        Ok(())
    }

    fn rename(&self, req: Rename<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.parent.bump();
        let Some(session) = self.session.get() else {
            return Ok(());
        };
        session.rename(req.name)?;
        Ok(())
    }
}

impl XdgToplevelSessionV1 {
    fn send_restored(&self) {
        self.client.event(Restored { self_id: self.id });
    }
}

object_base! {
    self = XdgToplevelSessionV1;
    version = self.version;
}

impl Object for XdgToplevelSessionV1 {
    fn break_loops(&self) {
        self.disown_to_peer();
    }
}

simple_add_obj!(XdgToplevelSessionV1);

#[derive(Debug, Error)]
pub enum XdgToplevelSessionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    SessionManagementError(#[from] SessionManagementError),
}
efrom!(XdgToplevelSessionV1Error, ClientError);
