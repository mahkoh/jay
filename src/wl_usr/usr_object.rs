use crate::object::Interface;
use crate::object::ObjectId;
use crate::object::Version;
use crate::utils::buffd::MsgParser;
use crate::wl_usr::UsrCon;
use crate::wl_usr::UsrConError;
use std::rc::Rc;

pub trait UsrObjectBase {
    fn id(&self) -> ObjectId;
    fn handle_event(
        self: Rc<Self>,
        con: &UsrCon,
        event: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), UsrConError>;
    fn interface(&self) -> Interface;
    fn version(&self) -> Version;
}

pub trait UsrObject: UsrObjectBase + 'static {
    fn destroy(&self);

    fn break_loops(&self) {}
}
