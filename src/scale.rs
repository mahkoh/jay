use std::fmt::{Debug, Display, Formatter};

const BASE: u32 = 120;
const BASE64: i64 = BASE as i64;
const BASEF: f64 = BASE as f64;

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
        Self(f.saturating_mul(BASE))
    }

    pub fn from_f64(f: f64) -> Self {
        Self((f * BASEF).round() as u32)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / BASEF
    }

    pub fn round_up(self) -> u32 {
        self.0.saturating_add(BASE - 1) / BASE
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
        v.map(|v| ((v as i64 * scale + BASE64 / 2) / BASE64) as i32)
    }
}

impl PartialEq<u32> for Scale {
    fn eq(&self, other: &u32) -> bool {
        self.0 == other * BASE
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
