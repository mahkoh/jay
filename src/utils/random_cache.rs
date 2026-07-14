use {
    crate::utils::{
        markers::JayHash,
        numcell::NumCell,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    hashbrown::HashTable,
    rand::{RngExt, rngs::SmallRng},
    std::{
        borrow::Borrow,
        cell::{Cell, UnsafeCell},
        fmt::{Debug, Formatter},
        mem,
        ops::Deref,
        ptr,
        rc::{Rc, Weak},
    },
};

#[cfg(test)]
mod tests;

pub struct RandomCache<K, V>
where
    K: JayHash,
{
    inner: Rc<Inner<K, V>>,
}

struct Inner<K, V>
where
    K: JayHash,
{
    serial: NumCell<u64>,
    random_state: ahash::RandomState,
    mut_: UnsafeCell<Mut<K, V>>,
}

struct Mut<K, V>
where
    K: JayHash,
{
    map: HashTable<(u64, Weak<RandomCached<K, V>>)>,
    heap: Heap<K, V>,
}

pub struct RandomCached<K, V>
where
    K: JayHash,
{
    inner: Rc<Inner<K, V>>,
    serial: Cell<u64>,
    hash: u64,
    key: K,
    value: V,
}

struct Heap<K, V>
where
    K: JayHash,
{
    map: Box<[Option<Rc<RandomCached<K, V>>>]>,
    rng: SmallRng,
}

impl<K, V> RandomCache<K, V>
where
    K: JayHash,
{
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new(size: usize) -> Self {
        Self {
            inner: Rc::new(Inner {
                serial: Default::default(),
                random_state: Default::default(),
                mut_: UnsafeCell::new(Mut {
                    map: Default::default(),
                    heap: Heap::new(size),
                }),
            }),
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn insert(&self, key: K, value: V) -> Rc<RandomCached<K, V>> {
        let inner = &self.inner;
        let hash = inner.random_state.hash_one(&key);
        let rc = Rc::new(RandomCached {
            inner: inner.clone(),
            serial: Cell::new(inner.serial.fetch_add(1)),
            hash,
            key,
            value,
        });
        let _old = {
            let mut_ = unsafe { inner.mut_.get().deref_mut() };
            mut_.map
                .entry(hash, is_match(&rc.key), |(h, _)| *h)
                .insert((hash, Rc::downgrade(&rc)));
            mut_.heap.insert(rc.clone())
        };
        rc
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn get<Q>(&self, key: &Q) -> Option<Rc<RandomCached<K, V>>>
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
        let _heap = {
            let mut_ = unsafe { self.inner.mut_.get().deref_mut() };
            mut_.map.clear();
            let mut heap = Heap::new(mut_.heap.map.len());
            mem::swap(&mut heap, &mut mut_.heap);
            heap
        };
    }
}

impl<K, V> RandomCached<K, V>
where
    K: JayHash,
{
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn mark_used(&self) {
        self.serial.set(self.inner.serial.fetch_add(1));
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn key(&self) -> &K {
        &self.key
    }
}

impl<K, V> Deref for RandomCached<K, V>
where
    K: JayHash,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<K, V> Debug for RandomCached<K, V>
where
    K: JayHash + Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entry(&self.key, &self.value).finish()
    }
}

impl<K, V> Heap<K, V>
where
    K: JayHash,
{
    fn new(size: usize) -> Self {
        let size = size.next_power_of_two().max(16);
        Self {
            map: (0..size).map(|_| None).collect(),
            rng: rand::make_rng(),
        }
    }

    fn insert(&mut self, value: Rc<RandomCached<K, V>>) -> Option<Rc<RandomCached<K, V>>> {
        let r1 = self.rng.random::<u64>() as usize;
        let r2 = self.rng.random::<u64>() as usize;
        let offsets = [r1, r2, r2.wrapping_add(1)];
        let entries = unsafe {
            let mask = self.map.len() - 1;
            let ptr = self.map.as_mut_ptr();
            offsets.map(|idx| ptr.add(idx & mask))
        };
        let mut min_entry = ptr::null_mut();
        let mut min_serial = u64::MAX;
        for entry in entries {
            match unsafe { &mut *entry } {
                e @ None => {
                    *e = Some(value);
                    return None;
                }
                Some(e) => {
                    let serial = e.serial.get();
                    if serial < min_serial {
                        min_entry = e;
                        min_serial = serial;
                    }
                }
            }
        }
        let old = unsafe { mem::replace(&mut *min_entry, value) };
        Some(old)
    }
}

impl<K, V> Drop for RandomCache<K, V>
where
    K: JayHash,
{
    fn drop(&mut self) {
        self.clear();
    }
}

impl<K, V> Drop for RandomCached<K, V>
where
    K: JayHash,
{
    fn drop(&mut self) {
        let is_match = |(_, v): &(_, Weak<_>)| v.as_ptr() == ptr::from_ref(self);
        let mut_ = unsafe { self.inner.mut_.get().deref_mut() };
        if let Ok(e) = mut_.map.find_entry(self.hash, is_match) {
            e.remove();
        }
    }
}

fn is_match<Q, K, V>(key: &Q) -> impl Fn(&(u64, Weak<RandomCached<K, V>>)) -> bool
where
    Q: JayHash,
    K: JayHash + Borrow<Q>,
{
    move |(_, k)| k.upgrade().map(|k| k.key.borrow() == key).unwrap_or(false)
}
