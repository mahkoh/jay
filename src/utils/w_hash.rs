use crate::utils::copyhashmap::LockableRandomState;
use hashbrown::HashMap;
use hashbrown::HashSet;
use rand::random;
use rustc_hash::FxHasher;
use std::hash::BuildHasher;

#[derive(Copy, Clone, Debug)]
pub struct WBuildHasher(usize);

impl Default for WBuildHasher {
    fn default() -> Self {
        Self(random::<u64>() as usize)
    }
}

impl BuildHasher for WBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FxHasher::with_seed(self.0)
    }
}

impl LockableRandomState for WBuildHasher {
    const LOCKED_STATE: Self = WBuildHasher(0);
}

#[expect(unused)]
pub type WHashSet<T> = HashSet<T, WBuildHasher>;

#[expect(unused)]
pub type WHashMap<K, V> = HashMap<K, V, WBuildHasher>;
