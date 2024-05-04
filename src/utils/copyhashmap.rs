use {
    crate::utils::{
        clonecell::UnsafeCellCloneSafe,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    ahash::AHashMap,
    std::{
        borrow::Borrow,
        cell::UnsafeCell,
        fmt::{Debug, Formatter},
        hash::Hash,
        mem,
        ops::{Deref, DerefMut},
    },
};

pub struct CopyHashMap<K, V> {
    map: UnsafeCell<AHashMap<K, V>>,
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

    pub fn set(&self, k: K, v: V) -> Option<V> {
        unsafe { self.map.get().deref_mut().insert(k, v) }
    }

    pub fn get<Q>(&self, k: &Q) -> Option<V>
    where
        V: UnsafeCellCloneSafe,
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.map.get().deref().get(k).cloned() }
    }

    pub fn remove<Q>(&self, k: &Q) -> Option<V>
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.map.get().deref_mut().remove(k) }
    }

    pub fn contains<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.map.get().deref().contains_key(k) }
    }

    pub fn lock(&self) -> Locked<'_, K, V> {
        Locked {
            source: self,
            map: self.clear(),
        }
    }

    pub fn clear(&self) -> AHashMap<K, V> {
        unsafe { mem::take(self.map.get().deref_mut()) }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.map.get().deref().is_empty() }
    }

    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    pub fn len(&self) -> usize {
        unsafe { self.map.get().deref().len() }
    }
}

pub struct Locked<'a, K, V> {
    source: &'a CopyHashMap<K, V>,
    map: AHashMap<K, V>,
}

impl<'a, K, V> Drop for Locked<'a, K, V> {
    fn drop(&mut self) {
        unsafe {
            mem::swap(&mut self.map, self.source.map.get().deref_mut());
        }
    }
}

impl<'a, K, V> Deref for Locked<'a, K, V> {
    type Target = AHashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<'a, K, V> DerefMut for Locked<'a, K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}
