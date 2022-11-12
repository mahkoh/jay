use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter},
    ops::{Add, AddAssign, Sub, SubAssign},
};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const EPSILON: Self = Fixed(1);

    pub fn is_integer(self) -> bool {
        self.0 & 255 == 0
    }

    pub fn from_f64(f: f64) -> Self {
        Self((f * 256.0) as i32)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / 256.0
    }

    pub fn from_1616(i: i32) -> Self {
        Self(i >> 8)
    }

    pub fn to_int(self) -> i32 {
        self.0 >> 8
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

impl PartialEq<i32> for Fixed {
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other << 8
    }
}

impl PartialOrd<i32> for Fixed {
    fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
        self.0.partial_cmp(&(*other << 8))
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

impl Sub<i32> for Fixed {
    type Output = Self;

    fn sub(self, rhs: i32) -> Self::Output {
        Self(self.0 - (rhs << 8))
    }
}

impl Add<i32> for Fixed {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        Self(self.0 + (rhs << 8))
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
