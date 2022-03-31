use ahash::AHashMap;
use std::borrow::Borrow;
use std::cell::{RefCell, RefMut};
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::mem;

pub struct CopyHashMap<K, V> {
    map: RefCell<AHashMap<K, V>>,
}

impl<K: Debug, V: Debug> Debug for CopyHashMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.map.fmt(f)
    }
}

impl<K, V> Default for CopyHashMap<K, V> {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl<K: Eq + Hash, V> CopyHashMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, k: K, v: V) {
        self.map.borrow_mut().insert(k, v);
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<V>
    where
        V: Clone,
        Q: Hash + Eq,
        K: Borrow<Q>,
    {
        self.map.borrow_mut().get(k).cloned()
    }

    pub fn remove(&self, k: &K) -> Option<V> {
        self.map.borrow_mut().remove(k)
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        Q: Hash + Eq,
        K: Borrow<Q>,
    {
        self.map.borrow_mut().contains_key(k)
    }

    pub fn lock(&self) -> RefMut<AHashMap<K, V>> {
        self.map.borrow_mut()
    }

    pub fn clear(&self) {
        mem::take(&mut *self.map.borrow_mut());
    }

    pub fn is_empty(&self) -> bool {
        self.map.borrow_mut().is_empty()
    }
}
