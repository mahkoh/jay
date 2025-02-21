use {
    crate::{
        ei::{
            EiContext,
            ei_client::{EiClient, EiClientError},
        },
        utils::buffd::EiMsgParser,
        wire_ei::EiHandshakeId,
    },
    std::{
        cmp::Ordering,
        fmt::{Display, Formatter, LowerHex},
        rc::Rc,
    },
};

pub const EI_HANDSHAKE_ID: EiHandshakeId = EiHandshakeId::from_raw(0);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct EiObjectId(u64);

impl EiObjectId {
    #[expect(dead_code)]
    pub const NONE: Self = EiObjectId(0);

    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl Display for EiObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl LowerHex for EiObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

pub trait EiObjectBase {
    fn id(&self) -> EiObjectId;
    fn version(&self) -> EiVersion;
    fn client(&self) -> &EiClient;
    fn handle_request(
        self: Rc<Self>,
        client: &EiClient,
        request: u32,
        parser: EiMsgParser<'_, '_>,
    ) -> Result<(), EiClientError>;
    fn interface(&self) -> EiInterface;
}

pub trait EiObject: EiObjectBase + 'static {
    fn break_loops(&self) {}

    fn context(&self) -> EiContext {
        self.client().context.get()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct EiInterface(pub &'static str);

impl EiInterface {
    pub fn name(self) -> &'static str {
        self.0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EiVersion(pub u32);

impl EiVersion {
    // pub const ALL: EiVersion = EiVersion(0);
}

impl PartialEq<u32> for EiVersion {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u32> for EiVersion {
    fn partial_cmp(&self, other: &u32) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
