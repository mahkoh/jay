use {
    crate::pipewire::{
        pw_con::PwCon,
        pw_object::{PwObject, PwObjectData},
        pw_parser::{PwParser, PwParserError},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub const PW_REGISTRY_VERSION: i32 = 3;

pw_opcodes! {
    PwRegistryEvents;

    Global = 0,
    GlobalRemove = 1,
}

pub struct PwRegistry {
    pub data: PwObjectData,
    pub con: Rc<PwCon>,
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
