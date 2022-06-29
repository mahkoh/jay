use {
    crate::pipewire::{
        pw_con::PwConData,
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
    pub con: Rc<PwConData>,
}

impl PwRegistry {
    fn handle_global(&self, mut p: PwParser<'_>) -> Result<(), PwRegistryError> {
        let s = p.read_struct()?;
        let mut p2 = s.fields;
        let id = p2.read_int()?;
        let permissions = p2.read_int()?;
        let ty = p2.read_string()?;
        let version = p2.read_int()?;
        let props = p2.read_dict_struct()?;
        log::info!("global: id={id}, permissions={permissions}, ty={ty}, version={version}");
        log::info!("props: {:#?}", props);
        Ok(())
    }

    fn handle_global_remove(&self, p: PwParser<'_>) -> Result<(), PwRegistryError> {
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
