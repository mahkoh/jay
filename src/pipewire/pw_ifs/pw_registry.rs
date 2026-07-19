use crate::pipewire::pw_con::PwCon;
use crate::pipewire::pw_object::PwObject;
use crate::pipewire::pw_object::PwObjectData;
use crate::pipewire::pw_parser::PwParser;
use crate::pipewire::pw_parser::PwParserError;
use std::rc::Rc;
use thiserror::Error;

pub const PW_REGISTRY_VERSION: i32 = 3;

pw_opcodes! {
    PwRegistryEvents;

    Global = 0,
    GlobalRemove = 1,
}

pub struct PwRegistry {
    pub data: PwObjectData,
    pub _con: Rc<PwCon>,
}

impl PwRegistry {
    fn handle_global(&self, _p: PwParser<'_>) -> Result<(), PwRegistryError> {
        Ok(())
    }

    fn handle_global_remove(&self, _p: PwParser<'_>) -> Result<(), PwRegistryError> {
        Ok(())
    }
}

pw_object_base! {
    PwRegistry, "registry", PwRegistryEvents;

    Global => handle_global,
    GlobalRemove => handle_global_remove,
}

impl PwObject for PwRegistry {}

#[derive(Debug, Error)]
pub enum PwRegistryError {
    #[error(transparent)]
    PwParserError(#[from] PwParserError),
}
