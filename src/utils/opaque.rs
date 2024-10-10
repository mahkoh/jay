use {
    arrayvec::ArrayString,
    rand::{thread_rng, Rng},
    serde::{de, Deserialize, Deserializer, Serialize, Serializer},
    std::{
        fmt::{Debug, Display, Formatter},
        num::ParseIntError,
        str::FromStr,
    },
    thiserror::Error,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Opaque {
    lo: u64,
    hi: u64,
}

pub fn opaque() -> Opaque {
    let mut rng = thread_rng();
    Opaque {
        lo: rng.gen(),
        hi: rng.gen(),
    }
}

impl Opaque {
    pub fn to_string(self) -> ArrayString<OPAQUE_LEN> {
        use std::fmt::Write;
        let mut s = ArrayString::new();
        write!(s, "{}", self).unwrap();
        s
    }
}

impl Display for Opaque {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.hi)?;
        write!(f, "{:016x}", self.lo)?;
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
            return Err(OpaqueError::InvalidLength);
        }
        if !s.is_char_boundary(OPAQUE_LEN / 2) {
            return Err(OpaqueError::NotAscii);
        }
        let (hi, lo) = s.split_at(OPAQUE_LEN / 2);
        let hi = u64::from_str_radix(hi, 16).map_err(OpaqueError::Parse)?;
        let lo = u64::from_str_radix(lo, 16).map_err(OpaqueError::Parse)?;
        Ok(Self { lo, hi })
    }
}

pub const OPAQUE_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum OpaqueError {
    #[error("The string is not exactly 32 bytes long")]
    InvalidLength,
    #[error("The string is not ascii")]
    NotAscii,
    #[error("Could not parse the string as a hex number")]
    Parse(ParseIntError),
}
