use ahash::AHashMap;
use std::cell::{RefCell, RefMut};
use std::hash::Hash;

pub struct CopyHashMap<K, V> {
    map: RefCell<AHashMap<K, V>>,
}

impl<K, V> Default for CopyHashMap<K, V> {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl<K: Eq + Hash, V: Clone> CopyHashMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, k: K, v: V) {
        self.map.borrow_mut().insert(k, v);
    }

    pub fn get(&self, k: &K) -> Option<V> {
        self.map.borrow_mut().get(k).cloned()
    }

    pub fn remove(&self, k: &K) -> Option<V> {
        self.map.borrow_mut().remove(k)
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.borrow_mut().contains_key(k)
    }

    pub fn lock(&self) -> RefMut<AHashMap<K, V>> {
        self.map.borrow_mut()
    }

    pub fn clear(&self) {
        self.map.borrow_mut().clear();
    }
}
