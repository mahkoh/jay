use {
    ahash::AHashMap,
    smallvec::{Array, SmallVec},
};

#[allow(clippy::allow_attributes, dead_code)]
pub trait Reset: Default {
    fn reset(&mut self);
}

impl<T: Array> Reset for SmallVec<T> {
    fn reset(&mut self) {
        self.clear();
    }
}

impl<T> Reset for Option<T> {
    fn reset(&mut self) {
        *self = None;
    }
}

impl<T> Reset for Vec<T> {
    fn reset(&mut self) {
        self.clear();
    }
}

impl<K, V> Reset for AHashMap<K, V> {
    fn reset(&mut self) {
        self.clear();
    }
}

impl Reset for bool {
    fn reset(&mut self) {
        *self = false;
    }
}

impl Reset for i32 {
    fn reset(&mut self) {
        *self = 0;
    }
}

macro_rules! tuples {
    ($($id:ident,)*) => {
        impl<$($id,)*> Reset for ($($id,)*) where $($id: Reset,)* {
            #[expect(non_snake_case)]
            fn reset(&mut self) {
                let ($($id,)*) = self;
                $($id.reset();)*
            }
        }
    };
}

tuples!(I0,);
tuples!(I0, I1,);
