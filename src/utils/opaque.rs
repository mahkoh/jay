use {
    crate::utils::array,
    arrayvec::ArrayString,
    rand::{RngExt, rng},
    serde::{Deserialize, Deserializer, Serialize, Serializer, de},
    std::{
        fmt::{Debug, Display, Formatter},
        mem,
        num::ParseIntError,
        str::FromStr,
    },
    thiserror::Error,
};

#[cfg(test)]
mod tests;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Opaque {
    v: [u64; 3],
}

pub fn opaque() -> Opaque {
    let mut rng = rng();
    Opaque {
        v: array::from_fn(|_| rng.random()),
    }
}

impl Opaque {
    pub fn to_string(self) -> ArrayString<OPAQUE_LEN> {
        use std::fmt::Write;
        let mut s = ArrayString::new();
        write!(s, "{}", self).unwrap();
        s
    }

    pub fn as_bytes(&self) -> &[u8] {
        uapi::as_bytes(&self.v)
    }

    pub fn from_bytes(bytes: [u8; size_of::<Opaque>()]) -> Self {
        unsafe { mem::transmute(bytes) }
    }
}

impl Display for Opaque {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.v[2].to_le())?;
        write!(f, "{:016x}", self.v[1].to_le())?;
        write!(f, "{:016x}", self.v[0].to_le())?;
        Ok(())
    }
}

impl Debug for Opaque {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Serialize for Opaque {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.to_string();
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for Opaque {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <&str>::deserialize(deserializer)?;
        Opaque::from_str(s).map_err(de::Error::custom)
    }
}

impl FromStr for Opaque {
    type Err = OpaqueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != OPAQUE_LEN {
            return Err(OpaqueError::InvalidStringLength);
        }
        if !s.is_char_boundary(OPAQUE_SEGMENT) {
            return Err(OpaqueError::NotAscii);
        }
        let (a, s) = s.split_at(OPAQUE_SEGMENT);
        if !s.is_char_boundary(OPAQUE_SEGMENT) {
            return Err(OpaqueError::NotAscii);
        }
        let (b, c) = s.split_at(OPAQUE_SEGMENT);
        let parse = |s: &str| {
            u64::from_str_radix(s, 16)
                .map(u64::from_le)
                .map_err(OpaqueError::Parse)
        };
        let v = [parse(c)?, parse(b)?, parse(a)?];
        Ok(Self { v })
    }
}

pub const OPAQUE_LEN: usize = 48;
const OPAQUE_SEGMENT: usize = OPAQUE_LEN / 3;

#[derive(Debug, Error)]
pub enum OpaqueError {
    #[error("The string is not exactly {OPAQUE_LEN} bytes long")]
    InvalidStringLength,
    #[error("The string is not ascii")]
    NotAscii,
    #[error("Could not parse the string as a hex number")]
    Parse(ParseIntError),
}
