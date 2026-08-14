use crate::utils::copyhashmap::LockableRandomState;
use hashbrown::HashMap;
use hashbrown::HashSet;

pub type BHashSet<T> = HashSet<T, ahash::RandomState>;

pub type BHashMap<K, V> = HashMap<K, V, ahash::RandomState>;

impl LockableRandomState for ahash::RandomState {
    const LOCKED_STATE: Self = ahash::RandomState::with_seeds(0, 0, 0, 0);
}
