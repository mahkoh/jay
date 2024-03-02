use std::cell::Cell;

pub trait CellExt {
    fn is_some(&self) -> bool;
    fn is_none(&self) -> bool;
}

impl<T: Copy> CellExt for Cell<Option<T>> {
    fn is_some(&self) -> bool {
        self.get().is_some()
    }

    fn is_none(&self) -> bool {
        self.get().is_none()
    }
}
