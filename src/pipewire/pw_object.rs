use {
    crate::{pipewire::pw_parser::PwParser, utils::numcell::NumCell},
    std::{cell::Cell, fmt::Debug, rc::Rc},
    thiserror::Error,
};

pub trait PwObjectBase {
    fn data(&self) -> &PwObjectData;
    fn interface(&self) -> &str;
    fn handle_msg(self: Rc<Self>, opcode: u8, parser: PwParser<'_>) -> Result<(), PwObjectError>;
    fn event_name(&self, opcode: u8) -> Option<&'static str>;
}

pub trait PwObject: PwObjectBase {
    fn bound_id(&self, id: u32) {
        let _ = id;
    }

    fn done(&self) {}
}

pub struct PwObjectData {
    pub id: u32,
    pub bound_id: Cell<Option<u32>>,
    pub sync_id: NumCell<u32>,
}

#[derive(Debug, Error)]
#[error("An error occurred in a `{interface}`")]
pub struct PwObjectError {
    pub interface: &'static str,
    #[source]
    pub source: PwObjectErrorType,
}

#[derive(Debug, Error)]
pub enum PwObjectErrorType {
    #[error("Unknown event {0}")]
    UnknownEvent(u8),
    #[error("An error occurred in event `{method}`")]
    EventError {
        method: &'static str,
        #[source]
        source: Box<dyn std::error::Error>,
    },
}

pub trait PwOpcode: Debug {
    fn id(&self) -> u8;
}
