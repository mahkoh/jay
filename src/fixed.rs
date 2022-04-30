use std::{
    fmt::{Debug, Display, Formatter},
    ops::{Add, AddAssign, Sub, SubAssign},
};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Fixed(pub i32);

impl Fixed {
    pub fn from_f64(f: f64) -> Self {
        Self((f * 256.0) as i32)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / 256.0
    }

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

impl Debug for Fixed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.to_f64(), f)
    }
}

impl Display for Fixed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.to_f64(), f)
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
