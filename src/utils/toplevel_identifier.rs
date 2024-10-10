use {
    crate::utils::opaque::{opaque, Opaque, OpaqueError, OPAQUE_LEN},
    arrayvec::ArrayString,
    std::{
        fmt::{Display, Formatter},
        str::FromStr,
    },
};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct ToplevelIdentifier(Opaque);

pub fn toplevel_identifier() -> ToplevelIdentifier {
    ToplevelIdentifier(opaque())
}

impl ToplevelIdentifier {
    pub fn to_string(self) -> ArrayString<OPAQUE_LEN> {
        self.0.to_string()
    }
}

impl Display for ToplevelIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ToplevelIdentifier {
    type Err = OpaqueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}
