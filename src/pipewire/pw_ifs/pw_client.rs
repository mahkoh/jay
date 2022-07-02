use {
    crate::pipewire::{
        pw_con::PwConData,
        pw_object::{PwObject, PwObjectData},
        pw_parser::{PwParser, PwParserError},
    },
    std::rc::Rc,
    thiserror::Error,
};

pw_opcodes! {
    PwClientMethods;

    Error = 1,
    UpdateProperties = 2,
    GetPermissions = 3,
    UpdatePermissions = 4,
}

pw_opcodes! {
    PwClientEvents;

    Info = 0,
    Permissions = 1,
}

pub struct PwClient {
    pub data: PwObjectData,
    pub con: Rc<PwConData>,
}

impl PwClient {
    fn handle_info(&self, mut p: PwParser<'_>) -> Result<(), PwClientError> {
        let s1 = p.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_int()?;
        let change_mask = p2.read_long()?;
        let props = p2.read_dict_struct()?;
        log::info!("info: id={id}, change_mask={change_mask}");
        log::info!("props: {:#?}", props);
        Ok(())
    }

    fn handle_permissions(&self, _p: PwParser<'_>) -> Result<(), PwClientError> {
        Ok(())
    }
}

pw_object_base! {
    PwClient, "client", PwClientEvents;

    Info => handle_info,
    Permissions => handle_permissions,
}

impl PwObject for PwClient {}

#[derive(Debug, Error)]
pub enum PwClientError {
    #[error(transparent)]
    PwParserError(#[from] PwParserError),
}
