use std::{
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Add, Div, Mul, Sub},
};

macro_rules! define {
    ($big:ident, $little:ty) => {
        #[derive(Copy, Clone)]
        #[repr(transparent)]
        pub struct $big(pub $little);

        impl Eq for $big {}

        impl PartialEq for $big {
            fn eq(&self, other: &Self) -> bool {
                self.0.to_bits() == other.0.to_bits()
            }
        }

        impl Hash for $big {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.0.to_bits().hash(state);
            }
        }

        impl Add<Self> for $big {
            type Output = Self;

            fn add(self, rhs: $big) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub<Self> for $big {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl Mul<Self> for $big {
            type Output = Self;

            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }

        impl Div<Self> for $big {
            type Output = Self;

            fn div(self, rhs: Self) -> Self::Output {
                Self(self.0 / rhs.0)
            }
        }

        impl Display for $big {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&self.0, f)
            }
        }

        impl Debug for $big {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(&self.0, f)
            }
        }
    };
}

define!(F64, f64);
define!(F32, f32);
