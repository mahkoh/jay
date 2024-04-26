use {
    crate::{
        client::{Client, ClientId},
        object::{Object, ObjectId},
        utils::copyhashmap::{CopyHashMap, Locked},
    },
    std::rc::Rc,
};

pub struct Bindings<P> {
    bindings: CopyHashMap<(ClientId, ObjectId), Rc<P>>,
}

impl<P> Default for Bindings<P> {
    fn default() -> Self {
        Self {
            bindings: Default::default(),
        }
    }
}

impl<P: Object> Bindings<P> {
    pub fn add(&self, client: &Client, obj: &Rc<P>) {
        let prev = self.bindings.set((client.id, obj.id()), obj.clone());
        assert!(prev.is_none());
    }

    pub fn remove(&self, client: &Client, obj: &P) {
        self.bindings.remove(&(client.id, obj.id()));
    }

    pub fn clear(&self) {
        self.bindings.clear();
    }

    pub fn lock(&self) -> Locked<(ClientId, ObjectId), Rc<P>> {
        self.bindings.lock()
    }
}
