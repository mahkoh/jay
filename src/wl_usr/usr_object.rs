use {
    crate::{
        object::{Interface, ObjectId},
        utils::buffd::MsgParser,
    },
    std::rc::Rc,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum UsrObjectErrorType {
    #[error("Could not process a `{event}` event")]
    EventError {
        event: &'static str,
        #[source]
        error: Box<dyn std::error::Error>,
    },
    #[error("Unknown event {event}")]
    UnknownEventError { event: u32 },
}

#[derive(Debug, Error)]
#[error("An error occurred in a `{}`", .interface.name())]
pub struct UsrObjectError {
    pub interface: Interface,
    #[source]
    pub ty: UsrObjectErrorType,
}

pub trait UsrObjectBase {
    fn id(&self) -> ObjectId;
    fn handle_event(
        self: Rc<Self>,
        event: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), UsrObjectError>;
    fn interface(&self) -> Interface;
}

pub trait UsrObject: UsrObjectBase + 'static {
    fn break_loops(&self) {}
}
