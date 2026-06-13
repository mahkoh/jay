use std::cell::Cell;

pub trait TypeWrapper {
    type D<T>;
}

#[derive(Copy, Clone, Default)]
pub struct CellWrapper;

impl TypeWrapper for CellWrapper {
    type D<T> = Cell<T>;
}

#[derive(Copy, Clone, Default)]
pub struct NoWrapper;

impl TypeWrapper for NoWrapper {
    type D<T> = T;
}
