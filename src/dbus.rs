pub use types::*;
use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        dbus::{
            property::{Get, GetReply},
            types::{ObjectPath, Signature, Variant},
        },
        io_uring::{IoUring, IoUringError},
        utils::{
            buf::DynamicBuf,
            bufio::{BufIo, BufIoError},
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            numcell::NumCell,
            oserror::OsError,
            run_toplevel::RunToplevel,
            stack::Stack,
            vecstorage::VecStorage,
            xrd::{XRD, xrd},
        },
        wire_dbus::{
            org,
            org::freedesktop::dbus::properties::{GetAll, GetAllReply, PropertiesChanged},
        },
    },
    ahash::AHashMap,
    std::{
        borrow::{Borrow, Cow},
        cell::{Cell, RefCell},
        fmt::{Debug, Display},
        future::Future,
        marker::PhantomData,
        mem,
        ops::Deref,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    },
    thiserror::Error,
    uapi::OwnedFd,
};

mod auth;
mod dynamic_type;
mod formatter;
mod holder;
mod incoming;
mod outgoing;
mod parser;
mod property;
mod socket;
mod types;

#[derive(Debug)]
pub struct CallError {
    pub name: String,
    pub msg: Option<String>,
}

impl Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(msg) = &self.msg {
            write!(f, "{}: {}", self.name, msg)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

#[derive(Debug, Error)]
pub enum DbusError {
    #[error("Encountered an unknown type in a signature")]
    UnknownType,
    #[error("Function call reply does not contain a reply serial")]
    NoReplySerial,
    #[error("Signal message contains no interface or member or path")]
    MissingSignalHeaders,
    #[error("Method call message contains no interface or member or path")]
    MissingMethodCallHeaders,
    #[error("Error has no error name")]
    NoErrorName,
    #[error("The socket was killed")]
    Killed,
    #[error("{0}")]
    CallError(CallError),
    #[error("FD index is out of bounds")]
    OobFds,
    #[error("Variant has an invalid type")]
    InvalidVariantType,
    #[error("Could not create a socket")]
    Socket(#[source] OsError),
    #[error("Could not connect")]
    Connect(#[source] IoUringError),
    #[error("Could not write to the dbus socket")]
    WriteError(#[source] IoUringError),
    #[error("Could not read from the dbus socket")]
    ReadError(#[source] IoUringError),
    #[error("timeout")]
    IoUringError(#[source] Box<IoUringError>),
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
    #[error("The session bus address is not set")]
    SessionBusAddressNotSet,
    #[error("Server does not support FD passing")]
    UnixFd,
    #[error("Server message has a different endianess than ourselves")]
    InvalidEndianess,
    #[error("Server speaks an unexpected protocol version")]
    InvalidProtocol,
    #[error("Signature contains an invalid type")]
    InvalidSignatureType,
    #[error("The signal already has a handler")]
    AlreadyHandled,
    #[error(transparent)]
    BufIoError(#[from] BufIoError),
    #[error(transparent)]
    DbusError(Rc<DbusError>),
}
efrom!(DbusError, IoUringError);

pub struct Dbus {
    eng: Rc<AsyncEngine>,
    ring: Rc<IoUring>,
    system: Rc<DbusHolder>,
    session: Rc<DbusHolder>,
    user_path: Option<String>,
}

impl Dbus {
    pub fn new(eng: &Rc<AsyncEngine>, ring: &Rc<IoUring>, run_toplevel: &Rc<RunToplevel>) -> Self {
        let user_path = match xrd() {
            Some(path) => Some(format!("{}/bus", path)),
            _ => {
                log::warn!("{} is not set", XRD);
                None
            }
        };
        log::info!("dbus path = {:?}", user_path);
        Self {
            eng: eng.clone(),
            ring: ring.clone(),
            system: Rc::new(DbusHolder::new(run_toplevel)),
            session: Rc::new(DbusHolder::new(run_toplevel)),
            user_path,
        }
    }

    pub fn clear(&self) {
        self.system.clear();
        self.session.clear();
    }

    pub async fn system(&self) -> Result<Rc<DbusSocket>, DbusError> {
        self.system
            .get(
                &self.eng,
                &self.ring,
                "/var/run/dbus/system_bus_socket",
                "System bus",
            )
            .await
    }

    pub async fn session(&self) -> Result<Rc<DbusSocket>, DbusError> {
        let sba = match self.user_path.as_deref() {
            None => return Err(DbusError::SessionBusAddressNotSet),
            Some(sba) => sba,
        };
        self.session
            .get(&self.eng, &self.ring, sba, "Session bus")
            .await
    }
}

unsafe trait ReplyHandler {
    fn signature(&self) -> &str;
    fn handle_error(self: Box<Self>, socket: &Rc<DbusSocket>, error: DbusError);
    fn handle(
        self: Box<Self>,
        socket: &Rc<DbusSocket>,
        headers: &Headers,
        parser: &mut Parser,
        buf: Vec<u8>,
    ) -> Result<(), DbusError>;
}

pub struct DbusSocket {
    bus_name: &'static str,
    fd: Rc<OwnedFd>,
    ring: Rc<IoUring>,
    in_bufs: Stack<Vec<u8>>,
    bufio: Rc<BufIo>,
    eng: Rc<AsyncEngine>,
    next_serial: NumCell<u32>,
    unique_name: CloneCell<Rc<String>>,
    reply_handlers: CopyHashMap<u32, Box<dyn ReplyHandler>>,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    outgoing_: Cell<Option<SpawnedFuture<()>>>,
    auth: Cell<Option<SpawnedFuture<()>>>,
    dead: Cell<bool>,
    headers: RefCell<VecStorage<(u8, Variant<'static>)>>,
    run_toplevel: Rc<RunToplevel>,
    signal_handlers: RefCell<AHashMap<(&'static str, &'static str), InterfaceSignalHandlers>>,
    objects: CopyHashMap<Cow<'static, str>, Rc<DbusObjectData>>,
}

#[derive(Hash, Eq, PartialEq)]
struct MemberHandlerOwnedKey {
    key: MemberHandlerKey<'static>,
}

#[derive(Hash, Eq, PartialEq)]
struct MemberHandlerKey<'a> {
    interface: &'a str,
    member: &'a str,
}

impl<'a> Borrow<MemberHandlerKey<'a>> for MemberHandlerOwnedKey {
    fn borrow(&self) -> &MemberHandlerKey<'a> {
        &self.key
    }
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

const MSG_METHOD_CALL: u8 = 1;
const MSG_METHOD_RETURN: u8 = 2;
const MSG_ERROR: u8 = 3;
const MSG_SIGNAL: u8 = 4;

const NO_REPLY_EXPECTED: u8 = 0x1;
#[expect(dead_code)]
const NO_AUTO_START: u8 = 0x2;
#[expect(dead_code)]
const ALLOW_INTERACTIVE_AUTHORIZATION: u8 = 0x4;

#[expect(dead_code)]
pub const DBUS_NAME_FLAG_ALLOW_REPLACEMENT: u32 = 0x1;
#[expect(dead_code)]
pub const DBUS_NAME_FLAG_REPLACE_EXISTING: u32 = 0x2;
pub const DBUS_NAME_FLAG_DO_NOT_QUEUE: u32 = 0x4;

pub const DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER: u32 = 1;
#[expect(dead_code)]
pub const DBUS_REQUEST_NAME_REPLY_IN_QUEUE: u32 = 2;
#[expect(dead_code)]
pub const DBUS_REQUEST_NAME_REPLY_EXISTS: u32 = 3;
#[expect(dead_code)]
pub const DBUS_REQUEST_NAME_REPLY_ALREADY_OWNER: u32 = 4;

pub const BUS_DEST: &str = "org.freedesktop.DBus";
pub const BUS_PATH: &str = "/org/freedesktop/DBus";

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
    run_toplevel: Rc<RunToplevel>,
}

impl DbusHolder {
    pub fn new(run_toplevel: &Rc<RunToplevel>) -> Self {
        Self {
            socket: Default::default(),
            run_toplevel: run_toplevel.clone(),
        }
    }

    pub fn clear(&self) {
        if let Some(socket) = self.socket.take() {
            socket.clear();
        }
    }
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
    buf: &'a mut DynamicBuf,
}

pub unsafe trait Message<'a>: Sized + 'a {
    const SIGNATURE: &'static str;
    const INTERFACE: &'static str;
    const MEMBER: &'static str;
    type Generic<'b>: Message<'b>;

    fn marshal(&self, w: &mut Formatter);
    fn unmarshal(p: &mut Parser<'a>) -> Result<Self, DbusError>;
    fn num_fds(&self) -> u32;
}

pub struct ErrorMessage<'a> {
    pub msg: Cow<'a, str>,
}

unsafe impl<'a> Message<'a> for ErrorMessage<'a> {
    const SIGNATURE: &'static str = "s";
    const INTERFACE: &'static str = "";
    const MEMBER: &'static str = "";
    type Generic<'b> = ErrorMessage<'b>;

    fn marshal(&self, w: &mut Formatter) {
        self.msg.marshal(w)
    }

    fn unmarshal(p: &mut Parser<'a>) -> Result<Self, DbusError> {
        Ok(Self {
            msg: p.unmarshal()?,
        })
    }

    fn num_fds(&self) -> u32 {
        0
    }
}

pub trait Property {
    const INTERFACE: &'static str;
    const PROPERTY: &'static str;
    type Type: DbusType<'static>;
}

pub trait Signal<'a>: Message<'a> {}

pub trait MethodCall<'a>: Message<'a> {
    type Reply: Message<'static>;
}

pub unsafe trait DbusType<'a>: Clone + 'a {
    const ALIGNMENT: usize;
    const IS_POD: bool;
    type Generic<'b>: DbusType<'b> + 'b;

    fn consume_signature(s: &mut &[u8]) -> Result<(), DbusError>;
    fn write_signature(w: &mut Vec<u8>);
    fn marshal(&self, fmt: &mut Formatter);
    fn unmarshal(parser: &mut Parser<'a>) -> Result<Self, DbusError>;

    fn num_fds(&self) -> u32 {
        0
    }
}

pub struct Reply<T: Message<'static>> {
    socket: Rc<DbusSocket>,
    buf: Vec<u8>,
    t: T::Generic<'static>,
}

pub struct PropertyValue<T: Property> {
    reply: Reply<GetReply<'static, T::Type>>,
}

impl<T: Property> Debug for PropertyValue<T>
where
    for<'a> <T::Type as DbusType<'static>>::Generic<'a>: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: Property> PropertyValue<T> {
    pub fn get<'a>(&'a self) -> &'a <T::Type as DbusType<'static>>::Generic<'a> {
        &self.reply.get().value
    }
}

impl<T: Message<'static>> Debug for Reply<T>
where
    for<'a> T::Generic<'a>: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: Message<'static>> Reply<T> {
    pub fn get<'a>(&'a self) -> &'a T::Generic<'a> {
        unsafe { mem::transmute(&self.t) }
    }
}

impl<T: Message<'static>> Drop for Reply<T> {
    fn drop(&mut self) {
        self.socket.in_bufs.push(mem::take(&mut self.buf));
    }
}

struct AsyncReplySlot<T: Message<'static>> {
    data: Cell<Option<Result<Reply<T>, DbusError>>>,
    waker: Cell<Option<Waker>>,
}

pub struct AsyncReply<T: Message<'static>> {
    socket: Rc<DbusSocket>,
    serial: u32,
    slot: Rc<AsyncReplySlot<T>>,
}

#[pin_project::pin_project]
pub struct AsyncProperty<T: Property> {
    #[pin]
    reply: AsyncReply<GetReply<'static, T::Type>>,
}

impl<T: Message<'static>> Drop for AsyncReply<T> {
    fn drop(&mut self) {
        self.socket.reply_handlers.remove(&self.serial);
    }
}

impl<T: Message<'static>> Future for AsyncReply<T> {
    type Output = Result<Reply<T>, DbusError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(d) = self.slot.data.take() {
            Poll::Ready(d)
        } else {
            self.slot.waker.set(Some(cx.waker().clone()));
            Poll::Pending
        }
    }
}

impl<T: Property> Future for AsyncProperty<T> {
    type Output = Result<PropertyValue<T>, DbusError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        AsyncProperty::project(self)
            .reply
            .poll(cx)
            .map(|r| r.map(|v| PropertyValue { reply: v }))
    }
}

struct SignalHandlerData<T, F> {
    path: Option<String>,
    rule: String,
    handler: F,
    _phantom: PhantomData<T>,
}

trait SignalHandlerApi {
    fn interface(&self) -> &'static str;
    fn member(&self) -> &'static str;
    fn signature(&self) -> &'static str;
    fn path(&self) -> Option<&str>;
    fn rule(&self) -> &str;
    fn handle(&self, parser: &mut Parser) -> Result<(), DbusError>;
}

impl<T, F> SignalHandlerApi for SignalHandlerData<T, F>
where
    T: Signal<'static>,
    F: for<'a> Fn(T::Generic<'a>),
{
    fn interface(&self) -> &'static str {
        T::INTERFACE
    }

    fn member(&self) -> &'static str {
        T::MEMBER
    }

    fn signature(&self) -> &'static str {
        T::SIGNATURE
    }

    fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    fn rule(&self) -> &str {
        &self.rule
    }

    fn handle<'a>(&self, parser: &mut Parser<'a>) -> Result<(), DbusError> {
        (self.handler)(T::Generic::<'a>::unmarshal(parser)?);
        Ok(())
    }
}

#[must_use]
pub struct SignalHandler {
    socket: Rc<DbusSocket>,
    data: Rc<dyn SignalHandlerApi>,
}

impl Drop for SignalHandler {
    fn drop(&mut self) {
        self.socket.remove_signal_handler(&*self.data);
    }
}

struct InterfaceSignalHandlers {
    unconditional: Option<Rc<dyn SignalHandlerApi>>,
    conditional: AHashMap<String, Rc<dyn SignalHandlerApi>>,
}

struct DbusObjectData {
    path: Cow<'static, str>,
    methods: CopyHashMap<MemberHandlerOwnedKey, Rc<dyn MethodHandlerApi>>,
    properties: CopyHashMap<MemberHandlerOwnedKey, Rc<dyn PropertyHandlerApi>>,
}

pub struct DbusObject {
    socket: Rc<DbusSocket>,
    data: Rc<DbusObjectData>,
}

impl Drop for DbusObject {
    fn drop(&mut self) {
        self.socket.objects.remove(&self.data.path);
    }
}

impl DbusObject {
    pub fn add_method<T, F>(&self, handler: F)
    where
        T: MethodCall<'static>,
        F: for<'a> Fn(T::Generic<'a>, PendingReply<T::Reply>) + 'static,
    {
        let rhd = Rc::new(MethodHandlerData {
            handler,
            _phantom: Default::default(),
        });
        let key = MemberHandlerOwnedKey {
            key: MemberHandlerKey {
                interface: T::INTERFACE,
                member: T::MEMBER,
            },
        };
        self.data.methods.set(key, rhd);
    }

    pub fn set_property<T>(&self, value: Variant<'static>)
    where
        T: Property + 'static,
    {
        self.emit_signal(&PropertiesChanged {
            interface_name: T::INTERFACE.into(),
            changed_properties: Cow::Borrowed(&[DictEntry {
                key: T::PROPERTY.into(),
                value: value.borrow(),
            }]),
            invalidated_properties: Default::default(),
        });
        let phd = Rc::new(PropertyHandlerData::<T> {
            data: value,
            _phantom: Default::default(),
        });
        let key = MemberHandlerOwnedKey {
            key: MemberHandlerKey {
                interface: T::INTERFACE,
                member: T::PROPERTY,
            },
        };
        self.data.properties.set(key, phd);
    }

    pub fn emit_signal<'a, T: Signal<'a>>(&self, signal: &T) {
        self.socket.emit_signal(&self.data.path, signal);
    }

    pub fn path(&self) -> &str {
        &self.data.path
    }
}

trait PropertyHandlerApi {
    fn interface(&self) -> &'static str;
    fn member(&self) -> &'static str;
    fn value<'a>(&'a self) -> Variant<'a>;
}

struct PropertyHandlerData<T> {
    data: Variant<'static>,
    _phantom: PhantomData<T>,
}

impl<T> PropertyHandlerApi for PropertyHandlerData<T>
where
    T: Property,
{
    fn interface(&self) -> &'static str {
        T::INTERFACE
    }

    fn member(&self) -> &'static str {
        T::PROPERTY
    }

    fn value<'a>(&'a self) -> Variant<'a> {
        self.data.borrow()
    }
}

pub struct PendingReply<T> {
    reply_expected: bool,
    socket: Rc<DbusSocket>,
    destination: String,
    serial: u32,
    _phantom: PhantomData<T>,
}

impl<T> PendingReply<T> {
    #[expect(dead_code)]
    pub fn reply_expected(&self) -> bool {
        self.reply_expected
    }

    pub fn err(&self, msg: &str) {
        if self.reply_expected {
            self.socket.send_error(&self.destination, self.serial, msg);
        }
    }
}

impl<T> PendingReply<T>
where
    T: Message<'static>,
{
    pub fn ok<'a>(&self, msg: &T::Generic<'a>) {
        if self.reply_expected {
            self.socket.send_reply(&self.destination, self.serial, msg);
        }
    }

    #[expect(dead_code)]
    pub fn complete<'a>(&self, res: Result<&T::Generic<'a>, &str>) {
        match res {
            Ok(m) => self.ok(m),
            Err(e) => self.err(e),
        }
    }
}

trait MethodHandlerApi {
    fn signature(&self) -> &'static str;
    fn handle(
        &self,
        object: &DbusObjectData,
        socket: &Rc<DbusSocket>,
        dest: &str,
        serial: u32,
        reply_expected: bool,
        parser: &mut Parser,
    ) -> Result<(), DbusError>;
}

struct MethodHandlerData<T, F> {
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> MethodHandlerApi for MethodHandlerData<T, F>
where
    T: MethodCall<'static>,
    F: for<'a> Fn(T::Generic<'a>, PendingReply<T::Reply>) + 'static,
{
    fn signature(&self) -> &'static str {
        T::SIGNATURE
    }

    fn handle<'a>(
        &self,
        _object: &DbusObjectData,
        socket: &Rc<DbusSocket>,
        dest: &str,
        serial: u32,
        reply_expected: bool,
        parser: &mut Parser<'a>,
    ) -> Result<(), DbusError> {
        let msg = T::Generic::<'a>::unmarshal(parser)?;
        let pr = PendingReply {
            reply_expected,
            socket: socket.clone(),
            destination: dest.to_string(),
            serial,
            _phantom: Default::default(),
        };
        (self.handler)(msg, pr);
        Ok(())
    }
}

struct PropertyGetHandlerProxy;

impl MethodHandlerApi for PropertyGetHandlerProxy {
    fn signature(&self) -> &'static str {
        Get::<u32>::SIGNATURE
    }

    fn handle<'a>(
        &self,
        object: &DbusObjectData,
        socket: &Rc<DbusSocket>,
        dest: &str,
        serial: u32,
        reply_expected: bool,
        parser: &mut Parser<'a>,
    ) -> Result<(), DbusError> {
        if !reply_expected {
            return Ok(());
        }
        let msg = org::freedesktop::dbus::properties::Get::unmarshal(parser)?;
        let key = MemberHandlerKey {
            interface: msg.interface_name.deref(),
            member: msg.property_name.deref(),
        };
        match object.properties.get(&key) {
            Some(h) => socket.send_reply(
                dest,
                serial,
                &org::freedesktop::dbus::properties::GetReply { value: h.value() },
            ),
            _ => socket.send_error(dest, serial, "Property does not exist"),
        };
        Ok(())
    }
}

struct PropertyGetAllHandlerProxy;

impl MethodHandlerApi for PropertyGetAllHandlerProxy {
    fn signature(&self) -> &'static str {
        GetAll::SIGNATURE
    }

    fn handle<'a>(
        &self,
        object: &DbusObjectData,
        socket: &Rc<DbusSocket>,
        dest: &str,
        serial: u32,
        reply_expected: bool,
        parser: &mut Parser<'a>,
    ) -> Result<(), DbusError> {
        if !reply_expected {
            return Ok(());
        }
        let msg = GetAll::unmarshal(parser)?;
        let all_props = object.properties.lock();
        let mut props = vec![];
        for property in all_props.values() {
            if property.interface() == msg.interface_name {
                props.push(DictEntry {
                    key: property.member().into(),
                    value: property.value(),
                });
            }
        }
        socket.send_reply(
            dest,
            serial,
            &GetAllReply {
                props: props.into(),
            },
        );
        Ok(())
    }
}

pub mod prelude {
    pub use {
        super::{
            DbusError, DbusType, Formatter, Message, MethodCall, Parser, Property, Signal,
            types::{Bool, DictEntry, ObjectPath, Variant},
        },
        std::{borrow::Cow, rc::Rc},
        uapi::OwnedFd,
    };
}
