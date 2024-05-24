use {crate::utils::ptr_ext::PtrExt, std::cell::Cell};

pub trait CellExt {
    fn is_some(&self) -> bool;
    fn is_none(&self) -> bool;
}

impl<T> CellExt for Cell<Option<T>> {
    fn is_some(&self) -> bool {
        unsafe { self.as_ptr().deref().is_some() }
    }

    fn is_none(&self) -> bool {
        !self.is_some()
    }
}
