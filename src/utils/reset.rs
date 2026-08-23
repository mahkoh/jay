use crate::utils::bhash::BHashMap;
use arrayvec::ArrayString;
use smallvec::Array;
use smallvec::SmallVec;

#[allow(dead_code)]
pub trait Reset {
    fn reset(&mut self);
}

impl Reset for () {
    fn reset(&mut self) {
        // nothing
    }
}

impl Reset for itoa::Buffer {
    fn reset(&mut self) {
        // nothing
    }
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

impl Reset for String {
    fn reset(&mut self) {
        self.clear();
    }
}

impl<const N: usize> Reset for ArrayString<N> {
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
num!(usize);

macro_rules! tuples {
    ($($id:ident,)*) => {
        impl<$($id,)*> Reset for ($($id,)*) where $($id: Reset,)* {
            #[allow(non_snake_case)]
            fn reset(&mut self) {
                let ($($id,)*) = self;
                $($id.reset();)*
            }
        }
    };
}

tuples!(I0,);
tuples!(I0, I1,);
