use {
    crate::{phf, phf::PhfHash},
    std::{marker::PhantomData, ops::Index},
};

pub(crate) struct PhfMap<K, V>
where
    K: ?Sized,
    V: 'static,
{
    pub(crate) key: u64,
    pub(crate) disps: &'static [(u32, u32)],
    pub(crate) map: &'static [V],
    pub(crate) _phantom: PhantomData<fn(&K) -> V>,
}

impl<K, V> Index<&'_ K> for PhfMap<K, V>
where
    K: ?Sized + PhfHash,
    V: 'static,
{
    type Output = V;

    #[inline]
    fn index(&self, index: &'_ K) -> &Self::Output {
        let hash = phf::hash(index, self.key);
        let idx = phf::get_unwrapped_index(&hash, self.disps) % self.map.len();
        &self.map[idx]
    }
}
