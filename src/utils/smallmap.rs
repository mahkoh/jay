use {
    crate::utils::{
        clonecell::UnsafeCellCloneSafe,
        ptr_ext::{MutPtrExt, PtrExt},
    },
    smallvec::SmallVec,
    std::{
        cell::UnsafeCell,
        fmt::{Debug, Formatter},
        mem,
    },
};

pub struct SmallMap<K, V, const N: usize> {
    m: UnsafeCell<SmallMapMut<K, V, N>>,
}

impl<K: Debug, V: Debug, const N: usize> Debug for SmallMap<K, V, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unsafe { self.m.get().deref().fmt(f) }
    }
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
        Self {
            m: UnsafeCell::new(SmallMapMut::new_with(k, v)),
        }
    }

    pub fn new() -> Self {
        Self {
            m: UnsafeCell::new(SmallMapMut::new()),
        }
    }

    pub fn contains(&self, k: &K) -> bool {
        unsafe { self.m.get().deref().contains(k) }
    }

    pub fn len(&self) -> usize {
        unsafe { self.m.get().deref().len() }
    }

    pub fn insert(&self, k: K, v: V) -> Option<V> {
        unsafe { self.m.get().deref_mut().insert(k, v) }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.m.get().deref().is_empty() }
    }

    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    pub fn remove(&self, k: &K) -> Option<V> {
        unsafe { self.m.get().deref_mut().remove(k) }
    }

    pub fn clear(&self) {
        unsafe {
            self.m.get().deref_mut().clear();
        }
    }

    pub fn take(&self) -> SmallVec<[(K, V); N]> {
        unsafe { self.m.get().deref_mut().take() }
    }

    pub fn replace(&self, other: SmallVec<[(K, V); N]>) -> SmallVec<[(K, V); N]> {
        unsafe { mem::replace(&mut self.m.get().deref_mut().m, other) }
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
            let m = &self.m.get().deref().m;
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
            let v = &self.map.m.get().deref().m;
            if self.pos >= v.len() {
                return None;
            }
            let (k, v) = &v[self.pos];
            self.pos += 1;
            Some((*k, v.clone()))
        }
    }
}

pub struct SmallMapMut<K, V, const N: usize> {
    m: SmallVec<[(K, V); N]>,
}

impl<K: Debug, V: Debug, const N: usize> Debug for SmallMapMut<K, V, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.m.iter().map(|e| (&e.0, &e.1)))
            .finish()
    }
}

impl<K, V, const N: usize> Default for SmallMapMut<K, V, N> {
    fn default() -> Self {
        Self {
            m: Default::default(),
        }
    }
}

impl<K: Eq, V, const N: usize> SmallMapMut<K, V, N> {
    pub fn new_with(k: K, v: V) -> Self {
        let mut sv = SmallVec::new();
        sv.push((k, v));
        Self { m: sv }
    }

    pub fn new() -> Self {
        Self {
            m: SmallVec::new_const(),
        }
    }

    pub fn len(&self) -> usize {
        self.m.len()
    }

    pub fn contains(&self, k: &K) -> bool {
        for (ek, _) in &self.m {
            if ek == k {
                return true;
            }
        }
        false
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        for (ek, ev) in &mut self.m {
            if ek == &k {
                return Some(mem::replace(ev, v));
            }
        }
        self.m.push((k, v));
        None
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        for (ek, ev) in &self.m {
            if ek == k {
                return Some(ev);
            }
        }
        None
    }

    pub fn get_or_default_mut(&mut self, k: K) -> &mut V
    where
        V: Default,
    {
        for (ek, ev) in &mut self.m {
            if ek == &k {
                return unsafe { (ev as *mut V).deref_mut() };
            }
        }
        self.m.push((k, V::default()));
        &mut self.m.last_mut().unwrap().1
    }

    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        for (idx, (ek, _)) in self.m.iter_mut().enumerate() {
            if ek == k {
                return Some(self.m.swap_remove(idx).1);
            }
        }
        None
    }

    pub fn clear(&mut self) {
        let _v = mem::replace(&mut self.m, SmallVec::new());
    }

    pub fn take(&mut self) -> SmallVec<[(K, V); N]> {
        mem::take(&mut self.m)
    }

    pub fn pop(&mut self) -> Option<(K, V)> {
        self.m.pop()
    }

    pub fn iter<'a>(&'a self) -> SmallMapMutIter<'a, K, V, N> {
        SmallMapMutIter { pos: 0, map: self }
    }

    pub fn iter_mut<'a>(&'a mut self) -> SmallMapMutIterMut<'a, K, V, N> {
        SmallMapMutIterMut { pos: 0, map: self }
    }

    pub fn remove_if<F: FnMut(&K, &V) -> bool>(&mut self, mut f: F) {
        let mut i = 0;
        while i < self.m.len() {
            let (k, v) = &self.m[i];
            if f(k, v) {
                self.m.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}

impl<'a, K: Copy, V, const N: usize> IntoIterator for &'a SmallMapMut<K, V, N> {
    type Item = (&'a K, &'a V);
    type IntoIter = SmallMapMutIter<'a, K, V, N>;

    fn into_iter(self) -> Self::IntoIter {
        SmallMapMutIter { pos: 0, map: self }
    }
}

pub struct SmallMapMutIter<'a, K, V, const N: usize> {
    pos: usize,
    map: &'a SmallMapMut<K, V, N>,
}

impl<'a, K, V, const N: usize> Iterator for SmallMapMutIter<'a, K, V, N> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.map.m.len() {
            return None;
        }
        let (k, v) = &self.map.m[self.pos];
        self.pos += 1;
        Some((k, v))
    }
}

pub struct SmallMapMutIterMut<'a, K, V, const N: usize> {
    pos: usize,
    map: &'a mut SmallMapMut<K, V, N>,
}

impl<'a, K, V, const N: usize> Iterator for SmallMapMutIterMut<'a, K, V, N> {
    type Item = (&'a mut K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.map.m.len() {
            return None;
        }
        let (k, v) = &mut self.map.m[self.pos];
        self.pos += 1;
        unsafe { Some(((k as *mut K).deref_mut(), (v as *mut V).deref_mut())) }
    }
}
