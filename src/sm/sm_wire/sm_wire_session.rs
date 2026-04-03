use {
    crate::utils::send_sync_rc::SendSyncRc,
    bincode::{Deserializer, Options},
    jay_config::_private::bincode_ops,
    serde::{Deserialize, Serialize},
    std::time::SystemTime,
    thiserror::Error,
};

#[derive(Default)]
pub struct SmSessionIn {
    pub last_acquire: SmSessionInUseData,
}

#[derive(Default)]
pub struct SmSessionInUseData {
    pub exe: Option<SendSyncRc<String>>,
}

pub struct SmSessionOut {
    #[expect(dead_code)]
    pub first_acquire: SmSessionOutUseData,
    #[expect(dead_code)]
    pub last_acquire: SmSessionOutUseData,
}

pub struct SmSessionOutUseData {
    pub exe: Option<String>,
    pub time: SystemTime,
}

#[derive(Debug, Error)]
pub enum DeserializeSessionError {
    #[error("Could not deserialize the V0 component")]
    DeserializeV0(#[source] bincode::Error),
}

pub fn deserialize_session(data: &[u8]) -> Result<SmSessionOut, DeserializeSessionError> {
    let wire = deserialize_session_(data)?;
    Ok(wire.into())
}

fn deserialize_session_(data: &[u8]) -> Result<WireSession<'_>, DeserializeSessionError> {
    let mut des = Deserializer::from_slice(data, bincode_ops());
    macro_rules! des {
        ($err:ident) => {
            Deserialize::deserialize(&mut des).map_err(DeserializeSessionError::$err)
        };
    }
    let wire = WireSession {
        v0: des!(DeserializeV0)?,
    };
    Ok(wire)
}

pub fn serialize_session(data: &mut Vec<u8>, tl: &SmSessionIn) {
    let wire = WireSession::from(tl);
    serialize_session_(data, &wire);
}

pub fn patch_session(
    data: &mut Vec<u8>,
    old: &[u8],
    new: &SmSessionIn,
) -> Result<SmSessionOut, DeserializeSessionError> {
    let old = deserialize_session_(old)?;
    let mut wire = WireSession::from(new);
    wire.v0.first_acquire = old.v0.first_acquire;
    serialize_session_(data, &wire);
    Ok(wire.into())
}

fn serialize_session_(data: &mut Vec<u8>, wire: &WireSession<'_>) {
    data.clear();
    bincode_ops().serialize_into(data, wire).unwrap();
}

#[derive(Serialize, Deserialize)]
pub struct WireSession<'a> {
    #[serde(borrow)]
    v0: WireSessionV0<'a>,
}

#[derive(Serialize, Deserialize)]
struct WireSessionV0<'a> {
    version: u32,
    #[serde(borrow)]
    first_acquire: WireSessionUserV0<'a>,
    #[serde(borrow)]
    last_acquire: WireSessionUserV0<'a>,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
struct WireSessionUserV0<'a> {
    exe: Option<&'a str>,
    time: SystemTime,
}

impl<'a> From<&'a SmSessionIn> for WireSession<'a> {
    fn from(value: &'a SmSessionIn) -> Self {
        Self { v0: value.into() }
    }
}

impl<'a> From<&'a SmSessionIn> for WireSessionV0<'a> {
    fn from(value: &'a SmSessionIn) -> Self {
        let last_acquire = WireSessionUserV0::from(&value.last_acquire);
        Self {
            version: 0,
            first_acquire: last_acquire,
            last_acquire,
        }
    }
}

impl From<WireSession<'_>> for SmSessionOut {
    fn from(value: WireSession<'_>) -> Self {
        Self {
            first_acquire: value.v0.first_acquire.into(),
            last_acquire: value.v0.last_acquire.into(),
        }
    }
}

impl<'a> From<&'a SmSessionInUseData> for WireSessionUserV0<'a> {
    fn from(value: &'a SmSessionInUseData) -> Self {
        Self {
            exe: value.exe.as_deref().map(|v| &**v),
            time: SystemTime::now(),
        }
    }
}

impl<'a> From<&'a SmSessionOutUseData> for WireSessionUserV0<'a> {
    fn from(value: &'a SmSessionOutUseData) -> Self {
        Self {
            exe: value.exe.as_deref(),
            time: value.time,
        }
    }
}

impl From<WireSessionUserV0<'_>> for SmSessionOutUseData {
    fn from(value: WireSessionUserV0<'_>) -> Self {
        Self {
            exe: value.exe.map(|v| v.to_owned()),
            time: value.time,
        }
    }
}
