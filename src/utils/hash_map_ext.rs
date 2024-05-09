use std::collections::HashMap;

pub trait HashMapExt {
    type V;

    fn drain_values(&mut self) -> impl Iterator<Item = Self::V>;
}

impl<K, V, S> HashMapExt for HashMap<K, V, S> {
    type V = V;

    fn drain_values(&mut self) -> impl Iterator<Item = Self::V> {
        self.drain().map(|(_, v)| v)
    }
}
