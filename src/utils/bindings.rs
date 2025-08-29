use {
    crate::{
        client::{Client, ClientId},
        object::{Object, ObjectId, Version},
        utils::copyhashmap::{CopyHashMap, Locked},
    },
    ahash::AHashMap,
    std::{
        cell::{Ref, RefCell},
        collections::hash_map::Entry,
        rc::Rc,
    },
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

    pub fn lock(&self) -> Locked<'_, (ClientId, ObjectId), Rc<P>> {
        self.bindings.lock()
    }
}

pub struct PerClientBindings<P> {
    bindings: RefCell<AHashMap<ClientId, AHashMap<ObjectId, Rc<P>>>>,
}

impl<P> Default for PerClientBindings<P> {
    fn default() -> Self {
        Self {
            bindings: Default::default(),
        }
    }
}

impl<P: Object> PerClientBindings<P> {
    pub fn add(&self, client: &Client, obj: &Rc<P>) {
        let prev = self
            .bindings
            .borrow_mut()
            .entry(client.id)
            .or_default()
            .insert(obj.id(), obj.clone());
        assert!(prev.is_none());
    }

    pub fn remove(&self, client: &Client, obj: &P) {
        if let Entry::Occupied(mut oe) = self.bindings.borrow_mut().entry(client.id) {
            oe.get_mut().remove(&obj.id());
            if oe.get().is_empty() {
                oe.remove();
            }
        }
    }

    pub fn clear(&self) {
        self.bindings.borrow_mut().clear();
    }

    pub fn for_each(&self, client: ClientId, version: Version, mut f: impl FnMut(&P)) {
        if let Some(bindings) = self.bindings.borrow().get(&client) {
            for obj in bindings.values() {
                if obj.version() >= version {
                    f(obj);
                }
            }
        }
    }

    pub fn borrow(&self) -> Ref<'_, AHashMap<ClientId, AHashMap<ObjectId, Rc<P>>>> {
        self.bindings.borrow()
    }
}
