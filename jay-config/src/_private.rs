pub mod client;
pub mod ipc;
mod logging;
pub(crate) mod string_error;

use {
    crate::{client::ClientMatcher, video::Mode},
    bincode::Options,
    serde::{Deserialize, Serialize},
    std::marker::PhantomData,
};

pub const VERSION: u32 = 1;

#[repr(C)]
pub struct ConfigEntry {
    pub version: u32,
    pub init: unsafe extern "C" fn(
        srv_data: *const u8,
        srv_unref: unsafe extern "C" fn(data: *const u8),
        srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
        msg: *const u8,
        size: usize,
    ) -> *const u8,
    pub unref: unsafe extern "C" fn(data: *const u8),
    pub handle_msg: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
}

pub struct ConfigEntryGen<T> {
    _phantom: PhantomData<T>,
}

impl<T: Config> ConfigEntryGen<T> {}

pub fn bincode_ops() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .with_no_limit()
}

pub trait Config {
    extern "C" fn configure();
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WireMode {
    pub width: i32,
    pub height: i32,
    pub refresh_millihz: u32,
}

impl WireMode {
    pub fn to_mode(self) -> Mode {
        Mode {
            width: self.width,
            height: self.height,
            refresh_millihz: self.refresh_millihz,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct PollableId(pub u64);

pub const DEFAULT_SEAT_NAME: &str = "default";

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub enum GenericCriterionIpc<T> {
    Matcher(T),
    Not(T),
    List { list: Vec<T>, all: bool },
    Exactly { list: Vec<T>, num: usize },
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ClientCriterionIpc {
    Generic(GenericCriterionIpc<ClientMatcher>),
    String {
        string: String,
        field: ClientCriterionStringField,
        regex: bool,
    },
    Sandboxed,
    Uid(i32),
    Pid(i32),
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ClientCriterionStringField {
    SandboxEngine,
    SandboxAppId,
    SandboxInstanceId,
}
