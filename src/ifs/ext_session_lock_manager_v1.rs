use {
    crate::{
        client::{Client, ClientCaps, ClientError, CAP_SESSION_LOCK_MANAGER},
        globals::{Global, GlobalName},
        ifs::ext_session_lock_v1::ExtSessionLockV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ext_session_lock_manager_v1::*, ExtSessionLockManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ExtSessionLockManagerV1Global {
    pub name: GlobalName,
}

impl ExtSessionLockManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtSessionLockManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtSessionLockManagerV1Error> {
        let obj = Rc::new(ExtSessionLockManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

pub struct ExtSessionLockManagerV1 {
    pub id: ExtSessionLockManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ExtSessionLockManagerV1RequestHandler for ExtSessionLockManagerV1 {
    type Error = ExtSessionLockManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn lock(&self, req: Lock, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let did_lock = self.client.state.lock.locked.get() == false;
        let new = Rc::new(ExtSessionLockV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            did_lock,
            finished: Cell::new(false),
            version: self.version,
        });
        track!(new.client, new);
        self.client.add_client_obj(&new)?;
        if did_lock {
            log::info!("Client {} locks the screen", self.client.id);
            let state = &self.client.state;
            for seat in state.globals.seats.lock().values() {
                seat.prepare_for_lock();
            }
            state.lock.locked.set(true);
            state.lock.lock.set(Some(new.clone()));
            state.tree_changed();
            state.damage(state.root.extents.get());
            new.send_locked();
        } else {
            new.finish();
        }
        Ok(())
    }
}

global_base!(
    ExtSessionLockManagerV1Global,
    ExtSessionLockManagerV1,
    ExtSessionLockManagerV1Error
);

impl Global for ExtSessionLockManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_SESSION_LOCK_MANAGER
    }
}

simple_add_global!(ExtSessionLockManagerV1Global);

object_base! {
    self = ExtSessionLockManagerV1;
    version = self.version;
}

impl Object for ExtSessionLockManagerV1 {}

simple_add_obj!(ExtSessionLockManagerV1);

#[derive(Debug, Error)]
pub enum ExtSessionLockManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtSessionLockManagerV1Error, ClientError);
