use crate::utils::bhash::BHashMap;
use crate::utils::markers::JayClone;
use crate::utils::markers::JayHash;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use derivative::Derivative;
use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::Hash;
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct CopyHashMap<K, V> {
    map: UnsafeCell<BHashMap<K, V>>,
}

impl<K: Debug, V: Debug> Debug for CopyHashMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.map.fmt(f)
    }
}

impl<K: Eq + Hash, V> CopyHashMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, k: K, v: V) -> Option<V>
    where
        K: JayHash,
    {
        unsafe { self.map.get().deref_mut().insert(k, v) }
    }

    pub fn get<Q>(&self, k: &Q) -> Option<V>
    where
        V: JayClone,
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.map.get().deref().get(k).cloned() }
    }

    pub fn remove<Q>(&self, k: &Q) -> Option<V>
    where
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.map.get().deref_mut().remove(k) }
    }

    pub fn contains<Q>(&self, k: &Q) -> bool
    where
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.map.get().deref().contains_key(k) }
    }

    pub fn not_contains<Q>(&self, k: &Q) -> bool
    where
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        !self.contains(k)
    }

    pub fn lock(&self) -> Locked<'_, K, V> {
        Locked {
            source: self,
            map: self.clear(),
        }
    }

    pub fn clear(&self) -> BHashMap<K, V> {
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
    map: BHashMap<K, V>,
}

impl<'a, K, V> Drop for Locked<'a, K, V> {
    fn drop(&mut self) {
        unsafe {
            mem::swap(&mut self.map, self.source.map.get().deref_mut());
        }
    }
}

impl<'a, K, V> Deref for Locked<'a, K, V> {
    type Target = BHashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<'a, K, V> DerefMut for Locked<'a, K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}
