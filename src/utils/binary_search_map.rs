use {
    crate::utils::ptr_ext::{MutPtrExt, PtrExt},
    smallvec::SmallVec,
    std::{
        fmt::{Debug, Formatter},
        mem,
    },
};

pub struct BinarySearchMap<K, V, const N: usize> {
    m: SmallVec<[(K, V); N]>,
}

impl<K: Debug, V: Debug, const N: usize> Debug for BinarySearchMap<K, V, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.m.iter().map(|e| (&e.0, &e.1)))
            .finish()
    }
}

impl<K, V, const N: usize> Default for BinarySearchMap<K, V, N> {
    fn default() -> Self {
        Self {
            m: Default::default(),
        }
    }
}

impl<K, V, const N: usize> BinarySearchMap<K, V, N> {
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

    fn pos(&self, k: &K) -> Result<usize, usize>
    where
        K: Ord + Eq,
    {
        self.m.binary_search_by(|(c, _)| c.cmp(k))
    }

    pub fn contains(&self, k: &K) -> bool
    where
        K: Ord + Eq,
    {
        self.pos(k).is_ok()
    }

    pub fn not_contains(&self, k: &K) -> bool
    where
        K: Ord + Eq,
    {
        !self.contains(k)
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V>
    where
        K: Ord + Eq,
    {
        match self.pos(&k) {
            Ok(p) => Some(mem::replace(&mut self.m[p], (k, v)).1),
            Err(p) => {
                self.m.insert(p, (k, v));
                None
            }
        }
    }

    pub fn get(&self, k: &K) -> Option<&V>
    where
        K: Ord + Eq,
    {
        self.pos(k).ok().map(|p| &self.m[p].1)
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V>
    where
        K: Ord + Eq,
    {
        self.pos(k).ok().map(|p| &mut self.m[p].1)
    }

    pub fn get_or_default_mut(&mut self, k: K) -> &mut V
    where
        K: Ord + Eq,
        V: Default,
    {
        self.get_or_insert_with(k, || V::default())
    }

    pub fn get_or_insert_with<F>(&mut self, k: K, f: F) -> &mut V
    where
        K: Ord + Eq,
        F: FnOnce() -> V,
    {
        let p = match self.pos(&k) {
            Ok(p) => return &mut self.m[p].1,
            Err(p) => p,
        };
        self.m.insert(p, (k, f()));
        &mut self.m[p].1
    }

    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }

    pub fn remove(&mut self, k: &K) -> Option<V>
    where
        K: Ord + Eq,
    {
        if let Ok(p) = self.pos(k) {
            return Some(self.m.remove(p).1);
        }
        None
    }

    pub fn clear(&mut self) {
        let _v = mem::replace(&mut self.m, SmallVec::new());
    }

    pub fn take(&mut self) -> SmallVec<[(K, V); N]> {
        mem::take(&mut self.m)
    }

    pub fn iter<'a>(&'a self) -> BinarySearchMapIter<'a, K, V, N> {
        BinarySearchMapIter { pos: 0, map: self }
    }

    pub fn values<'a>(&'a self) -> impl Iterator<Item = &'a V> + 'a {
        self.iter().map(|(_, v)| v)
    }

    pub fn iter_mut<'a>(&'a mut self) -> BinarySearchMapMutIterMut<'a, K, V, N> {
        BinarySearchMapMutIterMut { pos: 0, map: self }
    }

    pub fn values_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut V> + 'a {
        self.iter_mut().map(|(_, v)| v)
    }

    pub fn remove_if<F: FnMut(&K, &V) -> bool>(&mut self, mut f: F) {
        let mut i = 0;
        while i < self.m.len() {
            let (k, v) = &self.m[i];
            if f(k, v) {
                self.m.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

impl<'a, K: Copy, V, const N: usize> IntoIterator for &'a BinarySearchMap<K, V, N> {
    type Item = (&'a K, &'a V);
    type IntoIter = BinarySearchMapIter<'a, K, V, N>;

    fn into_iter(self) -> Self::IntoIter {
        BinarySearchMapIter { pos: 0, map: self }
    }
}

impl<'a, K: Copy, V, const N: usize> IntoIterator for &'a mut BinarySearchMap<K, V, N> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = BinarySearchMapMutIterMut<'a, K, V, N>;

    fn into_iter(self) -> Self::IntoIter {
        BinarySearchMapMutIterMut { pos: 0, map: self }
    }
}

pub struct BinarySearchMapIter<'a, K, V, const N: usize> {
    pos: usize,
    map: &'a BinarySearchMap<K, V, N>,
}

impl<'a, K, V, const N: usize> Iterator for BinarySearchMapIter<'a, K, V, N> {
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

pub struct BinarySearchMapMutIterMut<'a, K, V, const N: usize> {
    pos: usize,
    map: &'a mut BinarySearchMap<K, V, N>,
}

impl<'a, K, V, const N: usize> Iterator for BinarySearchMapMutIterMut<'a, K, V, N> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.map.m.len() {
            return None;
        }
        let (k, v) = &mut self.map.m[self.pos];
        self.pos += 1;
        unsafe { Some(((k as *const K).deref(), (v as *mut V).deref_mut())) }
    }
}
