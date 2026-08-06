use crate::utils::bhash::BHashMap;
use crate::utils::markers::JayClone;
use crate::utils::markers::JayHash;
use crate::utils::numcell::NumCell;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use ahash::RandomState;
use derivative::Derivative;
use hashbrown::HashMap;
use std::borrow::Borrow;
use std::cell::Cell;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::Hash;
use std::mem;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct CopyHashMap<K, V> {
    map: UnsafeCell<BHashMap<K, V>>,
    is_locked_map: Cell<bool>,
    access_count: NumCell<u64>,
}

impl<K: Debug, V: Debug> Debug for CopyHashMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.map.fmt(f)
    }
}

impl<K, V> CopyHashMap<K, V> {
    const LOCKED_MAP: BHashMap<K, V> = {
        const RANDOM_STATE: RandomState = RandomState::with_seeds(0, 0, 0, 0);
        HashMap::with_hasher(RANDOM_STATE)
    };

    #[inline(always)]
    unsafe fn get_map(&self) -> &BHashMap<K, V> {
        self.access_count.fetch_add(1);
        unsafe { self.map.get().deref() }
    }

    #[inline(always)]
    unsafe fn get_map_mut(&self) -> &mut BHashMap<K, V> {
        self.access_count.fetch_add(1);
        unsafe { self.map.get().deref_mut() }
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
        unsafe { self.get_map_mut().insert(k, v) }
    }

    pub fn get<Q>(&self, k: &Q) -> Option<V>
    where
        V: JayClone,
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.get_map().get(k).cloned() }
    }

    pub fn remove<Q>(&self, k: &Q) -> Option<V>
    where
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.get_map_mut().remove(k) }
    }

    pub fn contains<Q>(&self, k: &Q) -> bool
    where
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        unsafe { self.get_map().contains_key(k) }
    }

    pub fn not_contains<Q>(&self, k: &Q) -> bool
    where
        Q: JayHash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        !self.contains(k)
    }

    pub fn lock(&self) -> Locked<'_, K, V> {
        let map = unsafe { mem::replace(self.get_map_mut(), Self::LOCKED_MAP) };
        let is_locked_map = self.is_locked_map.replace(true);
        let access_count = self.access_count.get();
        Locked {
            source: (!is_locked_map).then_some(self),
            map: ManuallyDrop::new(map),
            access_count,
        }
    }

    pub fn clear(&self) -> BHashMap<K, V> {
        unsafe { mem::take(self.get_map_mut()) }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.get_map().is_empty() }
    }

    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    pub fn len(&self) -> usize {
        unsafe { self.get_map().len() }
    }
}

pub struct Locked<'a, K, V> {
    source: Option<&'a CopyHashMap<K, V>>,
    map: ManuallyDrop<BHashMap<K, V>>,
    access_count: u64,
}

impl<'a, K, V> Drop for Locked<'a, K, V> {
    fn drop(&mut self) {
        unsafe {
            let drop;
            match self.source {
                None => drop = true,
                Some(source) => {
                    drop = self.access_count != source.access_count.get();
                    mem::swap(&mut *self.map, source.get_map_mut());
                    source.is_locked_map.set(false);
                }
            }
            if drop {
                #[cold]
                fn cold() {}
                cold();
                ManuallyDrop::drop(&mut self.map);
            }
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
