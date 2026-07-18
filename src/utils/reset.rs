use {
    crate::utils::bhash::BHashMap,
    smallvec::{Array, SmallVec},
};

#[allow(clippy::allow_attributes, dead_code)]
pub trait Reset {
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

impl<K, V> Reset for BHashMap<K, V> {
    fn reset(&mut self) {
        self.clear();
    }
}

impl Reset for bool {
    fn reset(&mut self) {
        *self = false;
    }
}

macro_rules! num {
    ($ty:ty) => {
        impl Reset for $ty {
            fn reset(&mut self) {
                *self = 0;
            }
        }
    };
}

num!(i32);
num!(u32);
num!(u64);

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
