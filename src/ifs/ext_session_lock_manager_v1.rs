use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ext_session_lock_v1::ExtSessionLockV1,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
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
        _version: u32,
    ) -> Result<(), ExtSessionLockManagerV1Error> {
        let obj = Rc::new(ExtSessionLockManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
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
}

impl ExtSessionLockManagerV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockManagerV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn lock(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockManagerV1Error> {
        let req: Lock = self.client.parse(self, msg)?;
        let did_lock = self.client.state.lock.locked.get() == false;
        let new = Rc::new(ExtSessionLockV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            did_lock,
            finished: Cell::new(false),
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
            state.damage();
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

    fn secure(&self) -> bool {
        true
    }
}

simple_add_global!(ExtSessionLockManagerV1Global);

object_base! {
    ExtSessionLockManagerV1;

    DESTROY => destroy,
    LOCK => lock,
}

impl Object for ExtSessionLockManagerV1 {
    fn num_requests(&self) -> u32 {
        LOCK + 1
    }
}

simple_add_obj!(ExtSessionLockManagerV1);

#[derive(Debug, Error)]
pub enum ExtSessionLockManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtSessionLockManagerV1Error, MsgParserError);
efrom!(ExtSessionLockManagerV1Error, ClientError);
