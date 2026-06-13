use std::cell::Cell;

pub trait TypeWrapper {
    type D<T: Default>: Default;
}

pub struct CellWrapper;

impl TypeWrapper for CellWrapper {
    type D<T: Default> = Cell<T>;
}

#[expect(dead_code)]
pub struct NoWrapper;

impl TypeWrapper for NoWrapper {
    type D<T: Default> = T;
}
