use {
    crate::{
        ifs::wl_output::OutputIdHash, rect::Rect, sm::sm_wire::WireRect, tree::WorkspaceType,
        utils::send_sync_rc::SendSyncRc,
    },
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
    #[error("Could not deserialize the V1 component")]
    DeserializeV1(#[source] bincode::Error),
    #[error("Could not deserialize the V2 component")]
    DeserializeV2(#[source] bincode::Error),
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
    if wire.v0.version >= 1 {
        wire.v1 = des!(DeserializeV1)?;
    }
    if wire.v0.version >= 2 {
        wire.v2 = des!(DeserializeV2)?;
    }
    Ok(wire.into())
}

pub fn serialize_toplevel(data: &mut Vec<u8>, tl: &SmToplevelIn) {
    let wire = WireToplevel::from(tl);
    data.clear();
    bincode_ops().serialize_into(data, &wire).unwrap();
}

#[derive(Default)]
pub struct SmToplevelIn {
    pub output: Option<OutputIdHash>,
    pub workspace: Option<SendSyncRc<String>>,
    pub workspace_ty: Option<WorkspaceType>,
    pub floating_pos: Option<Rect>,
    pub fullscreen: bool,
}

pub struct SmToplevelOut {
    pub output: Option<OutputIdHash>,
    pub workspace: Option<String>,
    pub workspace_ty: Option<WorkspaceType>,
    pub floating_pos: Option<Rect>,
    pub fullscreen: bool,
}

#[derive(Copy, Clone, Default, Serialize)]
struct WireToplevel<'a> {
    v0: WireToplevelV0<'a>,
    v1: WireToplevelV1<'a>,
    v2: WireToplevelV2,
}

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
struct WireToplevelV0<'a> {
    version: u32,
    _phantom: PhantomData<&'a ()>,
}

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
struct WireToplevelV1<'a> {
    output: Option<OutputIdHash>,
    #[serde(borrow)]
    workspace: Option<&'a str>,
    floating_pos: Option<WireRect>,
    fullscreen: bool,
}

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
struct WireToplevelV2 {
    workspace_ty: Option<u32>,
}

impl<'a> From<&'a SmToplevelIn> for WireToplevel<'a> {
    fn from(value: &'a SmToplevelIn) -> Self {
        Self {
            v0: WireToplevelV0 {
                version: 2,
                _phantom: Default::default(),
            },
            v1: value.into(),
            v2: value.into(),
        }
    }
}

impl<'a> From<&'a SmToplevelIn> for WireToplevelV1<'a> {
    fn from(value: &'a SmToplevelIn) -> Self {
        Self {
            output: value.output,
            workspace: value.workspace.as_deref().map(|v| &**v),
            floating_pos: value.floating_pos.map(Into::into),
            fullscreen: value.fullscreen,
        }
    }
}

impl From<&SmToplevelIn> for WireToplevelV2 {
    fn from(value: &SmToplevelIn) -> Self {
        Self {
            workspace_ty: value.workspace_ty.map(Into::into),
        }
    }
}

impl From<WireToplevel<'_>> for SmToplevelOut {
    fn from(value: WireToplevel<'_>) -> Self {
        Self {
            output: value.v1.output,
            workspace: value.v1.workspace.map(Into::into),
            workspace_ty: value.v2.workspace_ty.and_then(map_workspace_type),
            floating_pos: value.v1.floating_pos.map(Into::into),
            fullscreen: value.v1.fullscreen,
        }
    }
}

impl From<WorkspaceType> for u32 {
    fn from(value: WorkspaceType) -> Self {
        match value {
            WorkspaceType::Normal => 0,
            WorkspaceType::Overlay => 1,
        }
    }
}

fn map_workspace_type(v: u32) -> Option<WorkspaceType> {
    match v {
        0 => Some(WorkspaceType::Normal),
        1 => Some(WorkspaceType::Overlay),
        _ => None,
    }
}
