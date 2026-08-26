use std::cell::Ref;

#[expect(unused)]
pub trait GetInner<T> {
    fn get_inner(&self) -> &T;
}

impl<T> GetInner<T> for T {
    fn get_inner(&self) -> &T {
        self
    }
}

impl<T> GetInner<T> for &'_ T {
    fn get_inner(&self) -> &T {
        self
    }
}

impl<T> GetInner<T> for Ref<'_, T> {
    fn get_inner(&self) -> &T {
        self
    }
}
