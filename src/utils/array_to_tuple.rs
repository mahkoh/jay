pub trait ArrayToTuple {
    type Tuple;

    fn to_tuple(self) -> Self::Tuple;
}

macro_rules! ignore {
    ($t:tt) => {
        T
    };
}

macro_rules! array_to_tuple {
    ($n:expr, $($field:ident,)*) => {
        impl<T> ArrayToTuple for [T; $n] {
            type Tuple = ($(ignore!($field),)*);

            fn to_tuple(self) -> Self::Tuple {
                let [$($field,)*] = self;
                #[allow(clippy::allow_attributes)]
                #[allow(clippy::unused_unit)]
                ($($field,)*)
            }
        }
    };
}

array_to_tuple!(2, t1, t2,);
