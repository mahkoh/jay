use crate::async_engine::{AsyncFd, SpawnedFuture};
use crate::dbus::types::{ObjectPath, Signature, Variant};
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::stack::Stack;
use crate::utils::vecstorage::VecStorage;
use crate::{AsyncEngine, AsyncError, AsyncQueue, CloneCell, NumCell};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::Waker;
use thiserror::Error;
use uapi::OwnedFd;

mod auth;
mod dynamic_type;
mod formatter;
mod holder;
mod incoming;
mod outgoing;
mod parser;
mod socket;
mod types;

#[derive(Debug, Error)]
pub enum DbusError {
    #[error("timeout")]
    Timeout,
    #[error("Encountered an unknown type in a signature")]
    UnknownType,
    #[error("BUS closed the connection")]
    Closed,
    #[error("FD index is out of bounds")]
    OobFds,
    #[error("Variant has an invalid type")]
    InvalidVariantType,
    #[error("Could not create a socket")]
    Socket(#[source] std::io::Error),
    #[error("Could not connect")]
    Connect(#[source] std::io::Error),
    #[error("Could not write to the dbus socket")]
    WriteError(#[source] std::io::Error),
    #[error("Could not read from the dbus socket")]
    ReadError(#[source] std::io::Error),
    #[error("timeout")]
    AsyncError(#[source] Box<AsyncError>),
    #[error("Server did not accept our authentication")]
    Auth,
    #[error("Array length is not a multiple of the element size")]
    PodArrayLength,
    #[error("Peer did not send enough fds")]
    TooFewFds,
    #[error("Variant signature is not a single type")]
    TrailingVariantSignature,
    #[error("Dict signature does not contain a terminating '}}'")]
    UnterminatedDict,
    #[error("Struct signature does not contain a terminating '}}'")]
    UnterminatedStruct,
    #[error("Dict signature contains trailing types")]
    DictTrailing,
    #[error("String does not contain valid UTF-8")]
    InvalidUtf8,
    #[error("Unexpected end of message")]
    UnexpectedEof,
    #[error("Boolean value was not 0 or 1")]
    InvalidBoolValue,
    #[error("Signature is empty")]
    EmptySignature,
    #[error("Server does not support FD passing")]
    UnixFd,
    #[error("Server message has a different endianess than ourselves")]
    InvalidEndianess,
    #[error("Server speaks an unexpected protocol version")]
    InvalidProtocol,
    #[error("Could not read from the socket")]
    ReadFailed(#[source] std::io::Error),
    #[error(transparent)]
    Rc(Rc<DbusError>),
}
efrom!(DbusError, AsyncError);

pub struct Dbus {
    eng: Rc<AsyncEngine>,
    system: Rc<DbusHolder>,
}

impl Dbus {
    pub fn new(eng: &Rc<AsyncEngine>) -> Self {
        Self {
            eng: eng.clone(),
            system: Default::default(),
        }
    }

    pub fn system(&self) -> Result<Rc<DbusSocket>, DbusError> {
        self.system
            .get(&self.eng, "/var/run/dbus/system_bus_socket")
    }
}

pub struct DbusSocket {
    fd: AsyncFd,
    eng: Rc<AsyncEngine>,
    next_serial: NumCell<u32>,
    bufs: Stack<Vec<u8>>,
    outgoing: AsyncQueue<DbusMessage>,
    waiters: CopyHashMap<u32, Waker>,
    replies: CopyHashMap<u32, Reply>,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    outgoing_: Cell<Option<SpawnedFuture<()>>>,
    auth: Cell<Option<SpawnedFuture<()>>>,
    dead: Cell<bool>,
    headers: RefCell<VecStorage<(u8, Variant<'static>)>>,
}

const TY_BYTE: u8 = b'y';
const TY_BOOLEAN: u8 = b'b';
const TY_INT16: u8 = b'n';
const TY_UINT16: u8 = b'q';
const TY_INT32: u8 = b'i';
const TY_UINT32: u8 = b'u';
const TY_INT64: u8 = b'x';
const TY_UINT64: u8 = b't';
const TY_DOUBLE: u8 = b'd';
const TY_STRING: u8 = b's';
const TY_OBJECT_PATH: u8 = b'o';
const TY_SIGNATURE: u8 = b'g';
const TY_ARRAY: u8 = b'a';
const TY_VARIANT: u8 = b'v';
const TY_UNIX_FD: u8 = b'h';

const HDR_PATH: u8 = 1;
const HDR_INTERFACE: u8 = 2;
const HDR_MEMBER: u8 = 3;
const HDR_ERROR_NAME: u8 = 4;
const HDR_REPLY_SERIAL: u8 = 5;
const HDR_DESTINATION: u8 = 6;
const HDR_SENDER: u8 = 7;
const HDR_SIGNATURE: u8 = 8;
const HDR_UNIX_FDS: u8 = 9;

#[derive(Default, Debug)]
struct Headers<'a> {
    path: Option<ObjectPath<'a>>,
    interface: Option<Cow<'a, str>>,
    member: Option<Cow<'a, str>>,
    error_name: Option<Cow<'a, str>>,
    reply_serial: Option<u32>,
    destination: Option<Cow<'a, str>>,
    sender: Option<Cow<'a, str>>,
    signature: Option<Signature<'a>>,
    unix_fds: Option<u32>,
}

struct DbusHolder {
    socket: CloneCell<Option<Rc<DbusSocket>>>,
}

impl Drop for DbusHolder {
    fn drop(&mut self) {
        if let Some(socket) = self.socket.take() {
            socket.auth.take();
            socket.outgoing_.take();
            socket.incoming.take();
        }
    }
}

impl Default for DbusHolder {
    fn default() -> Self {
        Self {
            socket: Default::default(),
        }
    }
}

struct DbusMessage {
    fds: Vec<Rc<OwnedFd>>,
    buf: Vec<u8>,
}

struct Reply {
    signature: String,
}

#[derive(Clone, Debug)]
pub enum DynamicType {
    U8,
    Bool,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F64,
    String,
    ObjectPath,
    Signature,
    Variant,
    Fd,
    Array(Box<DynamicType>),
    DictEntry(Box<DynamicType>, Box<DynamicType>),
    Struct(Vec<DynamicType>),
}

pub struct Parser<'a> {
    buf: &'a [u8],
    pos: usize,
    fds: &'a [Rc<OwnedFd>],
}

pub struct Formatter<'a> {
    fds: &'a mut Vec<Rc<OwnedFd>>,
    buf: &'a mut Vec<u8>,
}

pub trait Message<'a>: Sized {
    const SIGNATURE: &'static str;
    const INTERFACE: &'static str;
    const MEMBER: &'static str;

    fn marshal(&self, w: &mut Formatter);
    fn unmarshal(p: &mut Parser<'a>) -> Result<Self, DbusError>;
    fn num_fds(&self) -> u32;
}

pub trait MethodCall<'a>: Message<'a> {
    type Reply<'b>: Message<'b>;
}

pub unsafe trait DbusType<'a>: Clone {
    const ALIGNMENT: usize;
    const IS_POD: bool;

    fn write_signature(w: &mut Vec<u8>);
    fn marshal(&self, fmt: &mut Formatter);
    fn unmarshal(parser: &mut Parser<'a>) -> Result<Self, DbusError>;

    fn num_fds(&self) -> u32 {
        0
    }
}

pub mod prelude {
    pub use super::{
        types::{Bool, DictEntry, ObjectPath, Signature, Variant},
        DbusError, DbusType, Formatter, Message, MethodCall, Parser,
    };
    pub use std::borrow::Cow;
}
