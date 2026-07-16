pub use cache::{ObjectRegistryNoCache, ObjectRegistryRandomCache};
use {
    crate::utils::{
        markers::JayHash,
        object_registry::cache::Cache,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    hashbrown::HashTable,
    std::{
        borrow::Borrow,
        cell::{Cell, UnsafeCell},
        fmt::{Debug, Formatter},
        ops::Deref,
        ptr,
        rc::{Rc, Weak},
    },
};

mod cache;
#[cfg(test)]
mod tests;

pub trait ObjectRegistryCache<K, V>: Cache<K, V>
where
    K: JayHash,
{
}

pub struct ObjectRegistry<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    inner: Rc<Inner<K, V, H>>,
}

pub type CachedObjectRegistry<K, V> = ObjectRegistry<K, V, ObjectRegistryRandomCache<K, V>>;

pub type UncachedObjectRegistry<K, V> = ObjectRegistry<K, V, ObjectRegistryNoCache>;

struct Inner<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    random_state: ahash::RandomState,
    mut_: UnsafeCell<Mut<K, V, H>>,
}

struct Mut<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    map: HashTable<(u64, Weak<RegisteredObject<K, V, H>>)>,
    cache: H,
}

pub struct RegisteredObject<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    inner: Rc<Inner<K, V, H>>,
    serial: Cell<H::Serial>,
    hash: u64,
    key: K,
    value: V,
}

#[expect(dead_code)]
pub type CachedRegisteredObject<K, V> = RegisteredObject<K, V, ObjectRegistryRandomCache<K, V>>;

#[expect(dead_code)]
pub type UncachedRegisteredObject<K, V> = RegisteredObject<K, V, ObjectRegistryNoCache>;

impl<K, V> UncachedObjectRegistry<K, V>
where
    K: JayHash,
{
    pub fn uncached() -> Self {
        Self::new(ObjectRegistryNoCache)
    }
}

impl<K, V> Default for UncachedObjectRegistry<K, V>
where
    K: JayHash,
{
    fn default() -> Self {
        Self::uncached()
    }
}

impl<K, V> CachedObjectRegistry<K, V>
where
    K: JayHash,
{
    pub fn with_cache(size: usize) -> Self {
        Self::new(ObjectRegistryRandomCache::new(size))
    }
}

impl<K, V, H> ObjectRegistry<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    fn new(cache: H) -> Self {
        Self {
            inner: Rc::new(Inner {
                random_state: Default::default(),
                mut_: UnsafeCell::new(Mut {
                    map: Default::default(),
                    cache,
                }),
            }),
        }
    }

    pub fn insert(&self, key: K, value: V) -> Rc<RegisteredObject<K, V, H>> {
        let inner = &self.inner;
        let hash = inner.random_state.hash_one(&key);
        let rc = Rc::new(RegisteredObject {
            inner: inner.clone(),
            serial: Default::default(),
            hash,
            key,
            value,
        });
        let _old = {
            let mut_ = unsafe { inner.mut_.get().deref_mut() };
            mut_.map
                .entry(hash, is_match(&rc.key), |(h, _)| *h)
                .insert((hash, Rc::downgrade(&rc)));
            mut_.cache.insert(&rc)
        };
        rc
    }

    pub fn get<Q>(&self, key: &Q) -> Option<Rc<RegisteredObject<K, V, H>>>
    where
        Q: JayHash,
        K: Borrow<Q>,
    {
        let inner = &self.inner;
        let hash = inner.random_state.hash_one(key);
        let mut_ = unsafe { inner.mut_.get().deref() };
        mut_.map
            .find(hash, is_match(key))
            .and_then(|(_, w)| w.upgrade())
    }

    pub fn clear(&self) {
        let _cache = {
            let mut_ = unsafe { self.inner.mut_.get().deref_mut() };
            mut_.map.clear();
            mut_.cache.clear()
        };
    }
}

impl<K, V, H> RegisteredObject<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    pub fn mark_used(&self) {
        let mut_ = unsafe { self.inner.mut_.get().deref_mut() };
        self.serial.set(mut_.cache.serial());
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn key(&self) -> &K {
        &self.key
    }
}

impl<K, V, H> Deref for RegisteredObject<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<K, V, H> Debug for RegisteredObject<K, V, H>
where
    K: JayHash + Debug,
    V: Debug,
    H: ObjectRegistryCache<K, V>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entry(&self.key, &self.value).finish()
    }
}

impl<K, V, H> Drop for ObjectRegistry<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    fn drop(&mut self) {
        self.clear();
    }
}

impl<K, V, H> Drop for RegisteredObject<K, V, H>
where
    K: JayHash,
    H: ObjectRegistryCache<K, V>,
{
    fn drop(&mut self) {
        let is_match = |(_, v): &(_, Weak<_>)| v.as_ptr() == ptr::from_ref(self);
        let mut_ = unsafe { self.inner.mut_.get().deref_mut() };
        if let Ok(e) = mut_.map.find_entry(self.hash, is_match) {
            e.remove();
        }
    }
}

fn is_match<Q, K, V, H>(key: &Q) -> impl Fn(&(u64, Weak<RegisteredObject<K, V, H>>)) -> bool
where
    Q: JayHash,
    K: JayHash + Borrow<Q>,
    H: ObjectRegistryCache<K, V>,
{
    move |(_, k)| k.upgrade().map(|k| k.key.borrow() == key).unwrap_or(false)
}
