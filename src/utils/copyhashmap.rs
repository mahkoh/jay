use crate::utils::fx_hash::FxBuildHasher;
use crate::utils::markers::JayClone;
use crate::utils::markers::JayHash;
use crate::utils::numcell::NumCell;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use crate::utils::w_hash::WBuildHasher;
use ahash::RandomState;
use derivative::Derivative;
use hashbrown::HashMap;
use std::borrow::Borrow;
use std::cell::Cell;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::BuildHasher;
use std::hash::Hash;
use std::mem;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Derivative)]
#[derivative(Default(bound = "S: Default"))]
pub struct CopyHashMap<K, V, S = RandomState> {
    map: UnsafeCell<HashMap<K, V, S>>,
    is_locked_map: Cell<bool>,
    access_count: NumCell<u64>,
}

pub type FCopyHashMap<K, V> = CopyHashMap<K, V, FxBuildHasher>;

pub type WCopyHashMap<K, V> = CopyHashMap<K, V, WBuildHasher>;

impl<K, V, S> Debug for CopyHashMap<K, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.map.fmt(f)
    }
}

pub trait LockableRandomState: Default + BuildHasher + Sized {
    const LOCKED_STATE: Self;
}

impl<K, V, S> CopyHashMap<K, V, S>
where
    S: LockableRandomState,
{
    const LOCKED_MAP: HashMap<K, V, S> = HashMap::with_hasher(S::LOCKED_STATE);

    #[inline(always)]
    unsafe fn get_map(&self) -> &HashMap<K, V, S> {
        self.access_count.fetch_add(1);
        unsafe { self.map.get().deref() }
    }

    #[inline(always)]
    unsafe fn get_map_mut(&self) -> &mut HashMap<K, V, S> {
        self.access_count.fetch_add(1);
        unsafe { self.map.get().deref_mut() }
    }
}

impl<K: Eq + Hash, V, S> CopyHashMap<K, V, S>
where
    S: LockableRandomState,
{
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

    pub fn lock(&self) -> Locked<'_, K, V, S> {
        let map = unsafe { mem::replace(self.get_map_mut(), Self::LOCKED_MAP) };
        let is_locked_map = self.is_locked_map.replace(true);
        let access_count = self.access_count.get();
        Locked {
            source: (!is_locked_map).then_some(self),
            map: ManuallyDrop::new(map),
            access_count,
        }
    }

    pub fn clear(&self) -> HashMap<K, V, S> {
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

pub struct Locked<'a, K, V, S = RandomState>
where
    S: LockableRandomState,
{
    source: Option<&'a CopyHashMap<K, V, S>>,
    map: ManuallyDrop<HashMap<K, V, S>>,
    access_count: u64,
}

impl<'a, K, V, S> Drop for Locked<'a, K, V, S>
where
    S: LockableRandomState,
{
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

impl<'a, K, V, S> Deref for Locked<'a, K, V, S>
where
    S: LockableRandomState,
{
    type Target = HashMap<K, V, S>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<'a, K, V, S> DerefMut for Locked<'a, K, V, S>
where
    S: LockableRandomState,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}
