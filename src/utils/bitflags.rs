pub trait BitflagsExt {
    fn contains(self, other: Self) -> bool;
    fn intersects(self, other: Self) -> bool;
}

macro_rules! num {
    ($ty:ident) => {
        impl BitflagsExt for $ty {
            fn contains(self, other: Self) -> bool {
                self & other == other
            }

            fn intersects(self, other: Self) -> bool {
                self & other != 0
            }
        }
    };
}

num!(u8);
num!(u16);
num!(u32);
num!(u64);
num!(i8);
num!(i16);
num!(i32);
num!(i64);
