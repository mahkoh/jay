use crate::object::EventHandlingError;
use crate::object::Interface;
use crate::object::Version;
use crate::utils::buffd::MsgParser;
use crate::wire::ObjectId;
use std::rc::Rc;

pub trait UsrObjectBase {
    fn id(&self) -> ObjectId;
    fn handle_event(
        self: Rc<Self>,
        event: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), EventHandlingError>;
    fn interface(&self) -> Interface;
    fn version(&self) -> Version;
}

pub trait UsrObject: UsrObjectBase + 'static {
    fn destroy(&self);

    fn break_loops(&self) {}
}
