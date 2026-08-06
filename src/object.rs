use crate::client::Client;
use crate::client::ClientError;
use crate::utils::buffd::MsgParser;
use crate::utils::str_table::StrAccess;
use crate::wire::ObjectId;
use crate::wire::WlDisplayId;
use std::any::Any;
use std::cmp::Ordering;
use std::rc::Rc;

pub const WL_DISPLAY_ID: WlDisplayId = WlDisplayId::from_raw(1);

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
pub struct Interface(pub StrAccess);

impl Interface {
    pub fn name(self) -> &'static str {
        self.0.get()
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
