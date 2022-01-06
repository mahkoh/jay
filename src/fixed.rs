use std::fmt::{Debug, Formatter};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Fixed(pub i32);

impl Fixed {
    pub fn from_1616(i: i32) -> Self {
        Self(i >> 8)
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

impl From<Fixed> for i32 {
    fn from(f: Fixed) -> Self {
        f.0 >> 8
    }
}

impl Debug for Fixed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&f64::from(*self), f)
    }
}
