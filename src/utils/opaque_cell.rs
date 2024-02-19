use std::{
    cell::Cell,
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut},
};

#[derive(Default)]
pub struct OpaqueCell<T>(Cell<T>);

impl<T> Deref for OpaqueCell<T> {
    type Target = Cell<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for OpaqueCell<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Debug for OpaqueCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cell<{}> {{ ... }}", std::any::type_name::<T>())
    }
}
