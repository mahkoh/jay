use crate::git::Tree;
use anyhow::Result;

mod jay_config_ipc_enums;
mod jay_config_ipc_types;

pub trait Rule: Sync {
    fn hint(&self) -> &'static str;
    fn check(&self, base: &Tree, head: &Tree) -> Result<Vec<String>>;
}

pub fn rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(jay_config_ipc_enums::JayConfigIpcEnums),
        Box::new(jay_config_ipc_types::JayConfigIpcTypes),
    ]
}
