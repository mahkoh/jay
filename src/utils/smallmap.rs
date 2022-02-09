use crate::utils::clonecell::UnsafeCellCloneSafe;
use crate::utils::ptr_ext::{MutPtrExt, PtrExt};
use smallvec::SmallVec;
use std::cell::UnsafeCell;
use std::mem;

pub struct SmallMap<K, V, const N: usize> {
    m: UnsafeCell<SmallVec<[(K, V); N]>>,
}

impl<K, V, const N: usize> Default for SmallMap<K, V, N> {
    fn default() -> Self {
        Self {
            m: Default::default(),
        }
    }
}

impl<K: Eq, V, const N: usize> SmallMap<K, V, N> {
    pub fn new_with(k: K, v: V) -> Self {
        let mut sv = SmallVec::new();
        sv.push((k, v));
        Self {
            m: UnsafeCell::new(sv),
        }
    }

    pub fn new() -> Self {
        Self {
            m: UnsafeCell::new(SmallVec::new_const()),
        }
    }

    pub fn len(&self) -> usize {
        unsafe { self.m.get().deref().len() }
    }

    pub fn insert(&self, k: K, v: V) -> Option<V> {
        unsafe {
            let m = self.m.get().deref_mut();
            for (ek, ev) in &mut *m {
                if ek == &k {
                    return Some(mem::replace(ev, v));
                }
            }
            m.push((k, v));
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.m.get().deref_mut().is_empty() }
    }

    pub fn remove(&self, k: &K) -> Option<V> {
        unsafe {
            let m = self.m.get().deref_mut();
            for (idx, (ek, _)) in m.iter_mut().enumerate() {
                if ek == k {
                    return Some(m.swap_remove(idx).1);
                }
            }
            None
        }
    }

    pub fn clear(&self) {
        unsafe {
            let _v = mem::replace(self.m.get().deref_mut(), SmallVec::new());
        }
    }

    pub fn take(&self) -> SmallVec<[(K, V); N]> {
        unsafe { mem::take(self.m.get().deref_mut()) }
    }

    pub fn pop(&self) -> Option<(K, V)> {
        unsafe { self.m.get().deref_mut().pop() }
    }

    pub fn iter<'a>(&'a self) -> SmallMapIter<'a, K, V, N> {
        SmallMapIter { pos: 0, map: self }
    }
}

impl<K: Eq, V: UnsafeCellCloneSafe, const N: usize> SmallMap<K, V, N> {
    pub fn get(&self, k: &K) -> Option<V> {
        unsafe {
            let m = self.m.get().deref();
            for (ek, ev) in m {
                if ek == k {
                    return Some(ev.clone());
                }
            }
            None
        }
    }
}

impl<'a, K: Copy, V: UnsafeCellCloneSafe, const N: usize> IntoIterator for &'a SmallMap<K, V, N> {
    type Item = (K, V);
    type IntoIter = SmallMapIter<'a, K, V, N>;

    fn into_iter(self) -> Self::IntoIter {
        SmallMapIter { pos: 0, map: self }
    }
}

pub struct SmallMapIter<'a, K, V, const N: usize> {
    pos: usize,
    map: &'a SmallMap<K, V, N>,
}

impl<'a, K: Copy, V: UnsafeCellCloneSafe, const N: usize> Iterator for SmallMapIter<'a, K, V, N> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let v = self.map.m.get().deref();
            if self.pos >= v.len() {
                return None;
            }
            let (k, v) = &v[self.pos];
            self.pos += 1;
            Some((*k, v.clone()))
        }
    }
}
