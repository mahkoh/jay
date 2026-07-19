use hashbrown::HashMap;
use hashbrown::HashSet;

pub type BHashSet<T> = HashSet<T, ahash::RandomState>;

pub type BHashMap<K, V> = HashMap<K, V, ahash::RandomState>;
