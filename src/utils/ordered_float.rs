use std::{
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Add, Div, Mul, Sub},
};

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct F64(pub f64);

impl Eq for F64 {}

impl PartialEq for F64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Add<F64> for F64 {
    type Output = Self;

    fn add(self, rhs: F64) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<F64> for F64 {
    type Output = Self;

    fn sub(self, rhs: F64) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<F64> for F64 {
    type Output = Self;

    fn mul(self, rhs: F64) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Div<F64> for F64 {
    type Output = Self;

    fn div(self, rhs: F64) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Display for F64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for F64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
