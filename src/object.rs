use {
    crate::{client::ClientError, utils::buffd::MsgParser, wire::WlDisplayId},
    std::{
        any::Any,
        cmp::Ordering,
        fmt::{Display, Formatter},
        rc::Rc,
    },
};

pub const WL_DISPLAY_ID: WlDisplayId = WlDisplayId::from_raw(1);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(u32);

impl ObjectId {
    #[allow(dead_code)]
    pub const NONE: Self = ObjectId(0);

    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl Display for ObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub trait ObjectBase {
    fn id(&self) -> ObjectId;
    fn into_any(self: Rc<Self>) -> Rc<dyn Any>;
    fn handle_request(
        self: Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ClientError>;
    fn interface(&self) -> Interface;
}

pub trait Object: ObjectBase + 'static {
    fn break_loops(&self) {}
}

#[derive(Copy, Clone, Debug)]
pub struct Interface(pub &'static str);

impl Interface {
    pub fn name(self) -> &'static str {
        self.0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version(pub u32);

impl Version {
    pub const ALL: Version = Version(0);
}

impl PartialEq<u32> for Version {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u32> for Version {
    fn partial_cmp(&self, other: &u32) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}
