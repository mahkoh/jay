use {
    crate::utils::opaque::{opaque, Opaque, OpaqueError, OPAQUE_LEN},
    arrayvec::ArrayString,
    std::{
        fmt::{Display, Formatter},
        str::FromStr,
    },
};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct ActivationToken(Opaque);

pub fn activation_token() -> ActivationToken {
    ActivationToken(opaque())
}

impl ActivationToken {
    pub fn to_string(self) -> ArrayString<OPAQUE_LEN> {
        self.0.to_string()
    }
}

impl Display for ActivationToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ActivationToken {
    type Err = OpaqueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}
