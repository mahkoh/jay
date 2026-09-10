use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::copyhashmap::LockableRandomState;
use crate::utils::numcell::NumCell;
use crate::utils::reset::Reset;
use crate::utils::smallmap::SmallMap;
use std::cell::Cell;
use std::cell::RefCell;
use std::hash::Hash;

#[allow(dead_code)]
pub trait ResetImmutable {
    fn reset_immutable(&self);
}

impl ResetImmutable for () {
    fn reset_immutable(&self) {
        // nothing
    }
}

impl<T> ResetImmutable for Cell<Option<T>> {
    fn reset_immutable(&self) {
        self.take();
    }
}

impl<T> ResetImmutable for NumCell<T>
where
    T: Default,
{
    fn reset_immutable(&self) {
        self.set(T::default());
    }
}

impl<T> ResetImmutable for CloneCell<Option<T>> {
    fn reset_immutable(&self) {
        self.take();
    }
}

impl<K, V, S> ResetImmutable for CopyHashMap<K, V, S>
where
    K: Eq + Hash,
    S: LockableRandomState,
{
    fn reset_immutable(&self) {
        self.clear();
    }
}

impl<K, V, const N: usize> ResetImmutable for SmallMap<K, V, N>
where
    K: Eq,
{
    fn reset_immutable(&self) {
        self.clear();
    }
}

impl<T> ResetImmutable for RefCell<T>
where
    T: Reset,
{
    fn reset_immutable(&self) {
        self.borrow_mut().reset();
    }
}
