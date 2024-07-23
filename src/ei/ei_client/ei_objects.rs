use {
    crate::{
        ei::{
            ei_client::ei_error::EiClientError,
            ei_ifs::ei_handshake::EiHandshake,
            ei_object::{EiObject, EiObjectId},
        },
        utils::{copyhashmap::CopyHashMap, numcell::NumCell},
    },
    std::rc::Rc,
};

pub struct EiObjects {
    registry: CopyHashMap<EiObjectId, Rc<dyn EiObject>>,
    next_sever_id: NumCell<u64>,
}

pub const MIN_SERVER_ID: u64 = 0xff00_0000_0000_0000;

impl EiObjects {
    pub fn new() -> Self {
        Self {
            registry: Default::default(),
            next_sever_id: NumCell::new(MIN_SERVER_ID),
        }
    }

    pub fn destroy(&self) {
        for obj in self.registry.lock().values_mut() {
            obj.break_loops();
        }
        self.registry.clear();
    }

    pub fn id<T>(&self) -> T
    where
        EiObjectId: Into<T>,
    {
        EiObjectId::from_raw(self.next_sever_id.fetch_add(1)).into()
    }

    pub fn get_obj(&self, id: EiObjectId) -> Option<Rc<dyn EiObject>> {
        self.registry.get(&id)
    }

    pub fn add_server_object(&self, obj: Rc<dyn EiObject>) {
        let id = obj.id();
        assert!(id.raw() >= MIN_SERVER_ID);
        assert!(!self.registry.contains(&id));
        self.registry.set(id, obj.clone());
    }

    pub fn add_handshake(&self, obj: &Rc<EiHandshake>) {
        assert_eq!(obj.id.raw(), 0);
        assert!(self.registry.is_empty());
        self.registry.set(obj.id.into(), obj.clone());
    }

    pub fn add_client_object(&self, obj: Rc<dyn EiObject>) -> Result<(), EiClientError> {
        let id = obj.id();
        let res = (|| {
            if id.raw() == 0 || id.raw() >= MIN_SERVER_ID {
                return Err(EiClientError::ClientIdOutOfBounds);
            }
            if self.registry.contains(&id) {
                return Err(EiClientError::IdAlreadyInUse);
            }
            self.registry.set(id, obj.clone());
            Ok(())
        })();
        if let Err(e) = res {
            return Err(EiClientError::AddObjectError(id, Box::new(e)));
        }
        Ok(())
    }

    pub fn remove_obj(&self, id: EiObjectId) -> Result<(), EiClientError> {
        match self.registry.remove(&id) {
            Some(_) => Ok(()),
            _ => Err(EiClientError::UnknownId),
        }
    }
}
