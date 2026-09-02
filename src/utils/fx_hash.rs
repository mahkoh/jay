use crate::utils::copyhashmap::LockableRandomState;
use hashbrown::HashMap;
use hashbrown::HashSet;
use rustc_hash::FxHasher;
use std::hash::BuildHasher;

#[derive(Copy, Clone, Debug, Default)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FxHasher::default()
    }
}

impl LockableRandomState for FxBuildHasher {
    const LOCKED_STATE: Self = FxBuildHasher;
}

#[expect(unused)]
pub type FHashSet<T> = HashSet<T, FxBuildHasher>;

pub type FHashMap<K, V> = HashMap<K, V, FxBuildHasher>;
