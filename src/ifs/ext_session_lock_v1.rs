use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::ext_session_lock_surface_v1::{
            ExtSessionLockSurfaceV1, ExtSessionLockSurfaceV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ExtSessionLockV1Id, ext_session_lock_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ExtSessionLockV1 {
    pub id: ExtSessionLockV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub did_lock: bool,
    pub finished: Cell<bool>,
    pub version: Version,
}

impl ExtSessionLockV1 {
    pub fn send_locked(&self) {
        self.client.event(Locked { self_id: self.id })
    }

    fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id })
    }

    pub fn finish(&self) {
        self.send_finished();
        self.finished.set(true);
    }
}

impl ExtSessionLockV1RequestHandler for ExtSessionLockV1 {
    type Error = ExtSessionLockV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.finished.get() {
            self.client.state.lock.lock.take();
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_lock_surface(&self, req: GetLockSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?;
        let surface = self.client.lookup(req.surface)?;
        let new = Rc::new(ExtSessionLockSurfaceV1 {
            id: req.id,
            node_id: self.client.state.node_ids.next(),
            client: self.client.clone(),
            surface,
            tracker: Default::default(),
            serial: Default::default(),
            output: output.global.clone(),
            seat_state: Default::default(),
            version: self.version,
        });
        track!(new.client, new);
        new.install()?;
        self.client.add_client_obj(&new)?;
        if !self.finished.get()
            && let Some(node) = output.global.node()
        {
            if node.lock_surface.is_some() {
                return Err(ExtSessionLockV1Error::OutputAlreadyLocked);
            }
            node.set_lock_surface(Some(new.clone()));
            let pos = node.global.pos.get();
            new.change_extents(pos);
            new.surface.set_output(&node);
            self.client.state.tree_changed();
        }
        Ok(())
    }

    fn unlock_and_destroy(
        &self,
        _req: UnlockAndDestroy,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if !self.did_lock {
            return Err(ExtSessionLockV1Error::NeverLocked);
        }
        if !self.finished.get() {
            self.client.state.do_unlock();
        }
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ExtSessionLockV1;
    version = self.version;
}

impl Object for ExtSessionLockV1 {
    fn break_loops(&self) {
        if !self.finished.get() {
            self.client.state.lock.lock.take();
        }
    }
}

simple_add_obj!(ExtSessionLockV1);

#[derive(Debug, Error)]
pub enum ExtSessionLockV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The lock was not accepted")]
    NeverLocked,
    #[error("The output already has a lock surface attached")]
    OutputAlreadyLocked,
    #[error(transparent)]
    ExtSessionLockSurfaceV1Error(#[from] ExtSessionLockSurfaceV1Error),
}
efrom!(ExtSessionLockV1Error, ClientError);
