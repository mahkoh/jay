use crate::client::ClientError;
use crate::utils::buffd::MsgParser;
use crate::wire::WlDisplayId;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

pub const WL_DISPLAY_ID: WlDisplayId = WlDisplayId::from_raw(1);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(u32);

impl ObjectId {
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
    fn handle_request(
        self: Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ClientError>;
    fn interface(&self) -> Interface;
}

pub trait Object: ObjectBase + 'static {
    fn num_requests(&self) -> u32;
    fn break_loops(&self) {}
}

#[derive(Copy, Clone, Debug)]
pub struct Interface(pub &'static str);

impl Interface {
    pub fn name(self) -> &'static str {
        self.0
    }
}
