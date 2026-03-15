use std::fmt::{Debug, Display, Formatter};

pub const SCALE_BASE: u32 = 120;
const BASE64: i64 = SCALE_BASE as i64;
pub const SCALE_BASEF: f64 = SCALE_BASE as f64;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Scale(u32);

impl Default for Scale {
    fn default() -> Self {
        Scale::from_int(1)
    }
}

impl Scale {
    pub const fn from_int(f: u32) -> Self {
        Self(f.saturating_mul(SCALE_BASE))
    }

    pub fn from_f64(f: f64) -> Self {
        Self((f * SCALE_BASEF).round() as u32)
    }

    pub fn from_f64_as_float(f: f64) -> Self {
        Self(((f * (SCALE_BASEF / 15.0)).round() as u32).saturating_mul(15))
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE_BASEF
    }

    pub fn round_up(self) -> u32 {
        self.0.saturating_add(SCALE_BASE - 1) / SCALE_BASE
    }

    pub const fn from_wl(wl: u32) -> Self {
        Self(wl)
    }

    pub fn to_wl(self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub fn pixel_size<const N: usize>(self, v: [i32; N]) -> [i32; N] {
        if self == Scale::default() {
            return v;
        }
        let scale = self.0 as i64;
        v.map(|v| ((v as i64 * scale + v.signum() as i64 * BASE64 / 2) / BASE64) as i32)
    }
}

impl PartialEq<u32> for Scale {
    fn eq(&self, other: &u32) -> bool {
        self.0 == other * SCALE_BASE
    }
}

impl Debug for Scale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.to_f64(), f)
    }
}

impl Display for Scale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.to_f64(), f)
    }
}
