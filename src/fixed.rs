use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Fixed(pub i32);

impl Fixed {
    pub fn from_1616(i: i32) -> Self {
        Self(i >> 8)
    }

    pub fn from_int(i: i32) -> Self {
        Self(i << 8)
    }

    pub fn round_down(self) -> i32 {
        self.0 >> 8
    }

    pub fn apply_fract(self, i: i32) -> Self {
        Self((i << 8) | (self.0 & 255))
    }
}

impl From<f64> for Fixed {
    fn from(v: f64) -> Self {
        Self((v * 256.0) as i32)
    }
}

impl From<Fixed> for f64 {
    fn from(v: Fixed) -> Self {
        v.0 as f64 / 256.0
    }
}

impl Debug for Fixed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&f64::from(*self), f)
    }
}

impl Display for Fixed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&f64::from(*self), f)
    }
}

impl Sub for Fixed {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Add for Fixed {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Fixed {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Fixed {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}
