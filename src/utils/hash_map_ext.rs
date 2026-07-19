use hashbrown::Equivalent;
use hashbrown::HashMap;
use hashbrown::HashSet;
use std::hash::BuildHasher;
use std::hash::Hash;

pub trait HashMapExt {
    type K;
    type V;

    fn drain_values(&mut self) -> impl Iterator<Item = Self::V>;

    fn not_contains_key<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<Self::K> + ?Sized;

    fn is_not_empty(&self) -> bool;
}

impl<K, V, S> HashMapExt for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type K = K;
    type V = V;

    fn drain_values(&mut self) -> impl Iterator<Item = Self::V> {
        self.drain().map(|(_, v)| v)
    }

    fn not_contains_key<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<Self::K> + ?Sized,
    {
        !self.contains_key(k)
    }

    fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }
}

pub trait HashSetExt {
    type T;

    fn not_contains<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<Self::T> + ?Sized;
}

impl<T, S> HashSetExt for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type T = T;

    fn not_contains<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<Self::T> + ?Sized,
    {
        !self.contains(k)
    }
}
