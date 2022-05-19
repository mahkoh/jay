use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::ext_session_lock_surface_v1::{
            ExtSessionLockSurfaceV1, ExtSessionLockSurfaceV1Error,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{ext_session_lock_v1::*, ExtSessionLockV1Id},
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

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        if !self.finished.get() {
            self.client.state.lock.lock.take();
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_lock_surface(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockV1Error> {
        let req: GetLockSurface = self.client.parse(self, msg)?;
        let output = self.client.lookup(req.output)?;
        let surface = self.client.lookup(req.surface)?;
        let new = Rc::new(ExtSessionLockSurfaceV1 {
            id: req.id,
            node_id: self.client.state.node_ids.next(),
            client: self.client.clone(),
            surface,
            tracker: Default::default(),
            serial: Default::default(),
            output: output.global.node.get(),
            seat_state: Default::default(),
        });
        track!(new.client, new);
        new.install()?;
        self.client.add_client_obj(&new)?;
        if !output.global.destroyed.get() && !self.finished.get() {
            if let Some(node) = output.global.node.get() {
                if node.lock_surface.get().is_some() {
                    return Err(ExtSessionLockV1Error::OutputAlreadyLocked);
                }
                node.lock_surface.set(Some(new.clone()));
                let pos = output.global.pos.get();
                new.change_extents(pos);
                self.client.state.tree_changed();
            }
        }
        Ok(())
    }

    fn unlock_and_destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockV1Error> {
        let _req: UnlockAndDestroy = self.client.parse(self, msg)?;
        if !self.did_lock {
            return Err(ExtSessionLockV1Error::NeverLocked);
        }
        if !self.finished.get() {
            let state = &self.client.state;
            state.lock.locked.set(false);
            state.lock.lock.take();
            for output in state.outputs.lock().values() {
                if let Some(surface) = output.node.lock_surface.take() {
                    surface.destroy_node();
                }
            }
            state.tree_changed();
            state.damage();
        }
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ExtSessionLockV1;

    DESTROY => destroy,
    GET_LOCK_SURFACE => get_lock_surface,
    UNLOCK_AND_DESTROY => unlock_and_destroy,
}

impl Object for ExtSessionLockV1 {
    fn num_requests(&self) -> u32 {
        UNLOCK_AND_DESTROY + 1
    }

    fn break_loops(&self) {
        if !self.finished.get() {
            self.client.state.lock.lock.take();
        }
    }
}

simple_add_obj!(ExtSessionLockV1);

#[derive(Debug, Error)]
pub enum ExtSessionLockV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The lock was not accepted")]
    NeverLocked,
    #[error("The output already has a lock surface attached")]
    OutputAlreadyLocked,
    #[error(transparent)]
    ExtSessionLockSurfaceV1Error(#[from] ExtSessionLockSurfaceV1Error),
}
efrom!(ExtSessionLockV1Error, MsgParserError);
efrom!(ExtSessionLockV1Error, ClientError);
