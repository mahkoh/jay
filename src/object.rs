use crate::client::Client;
use crate::client::ClientError;
use crate::utils::buffd::MsgParser;
use crate::wire::WlDisplayId;
use jay_proc::jay_hash;
use std::any::Any;
use std::cmp::Ordering;
use std::fmt::Display;
use std::fmt::Formatter;
use std::rc::Rc;

pub const WL_DISPLAY_ID: WlDisplayId = WlDisplayId::from_raw(1);

#[jay_hash]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq)]
pub struct ObjectId(u32);

impl ObjectId {
    #[expect(dead_code)]
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

pub trait ObjectBase: Any {
    fn id(&self) -> ObjectId;
    fn version(&self) -> Version;
    fn handle_request(
        self: Rc<Self>,
        client: &Client,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ClientError>;
    fn interface(&self) -> Interface;
}

pub trait Object: ObjectBase + 'static {
    fn break_loops(self: Rc<Self>) {}
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
