use {
    bincode::{Deserializer, Options},
    jay_config::_private::bincode_ops,
    serde::{Deserialize, Serialize},
    std::marker::PhantomData,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum DeserializeToplevelError {
    #[error("Could not deserialize the V0 component")]
    DeserializeV0(#[source] bincode::Error),
}

pub fn deserialize_toplevel(data: &[u8]) -> Result<SmToplevelOut, DeserializeToplevelError> {
    let mut des = Deserializer::from_slice(data, bincode_ops());
    macro_rules! des {
        ($err:ident) => {
            Deserialize::deserialize(&mut des).map_err(DeserializeToplevelError::$err)
        };
    }
    let mut wire = WireToplevel::default();
    wire.v0 = des!(DeserializeV0)?;
    Ok(wire.into())
}

pub fn serialize_toplevel(data: &mut Vec<u8>, tl: &SmToplevelIn) {
    let wire = WireToplevel::from(tl);
    data.clear();
    bincode_ops().serialize_into(data, &wire).unwrap();
}

#[derive(Default)]
pub struct SmToplevelIn {}

pub struct SmToplevelOut {}

#[derive(Copy, Clone, Default, Serialize)]
struct WireToplevel<'a> {
    v0: WireToplevelV0<'a>,
}

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
struct WireToplevelV0<'a> {
    version: u32,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> From<&'a SmToplevelIn> for WireToplevel<'a> {
    fn from(_value: &'a SmToplevelIn) -> Self {
        Self {
            v0: WireToplevelV0 {
                version: 0,
                _phantom: Default::default(),
            },
        }
    }
}

impl From<WireToplevel<'_>> for SmToplevelOut {
    fn from(_value: WireToplevel<'_>) -> Self {
        Self {}
    }
}
