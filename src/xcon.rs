use crate::async_engine::SpawnedFuture;
use crate::utils::bufio::{BufIo, BufIoError, BufIoMessage};
use crate::utils::oserror::OsError;
use crate::wire_xcon::{
    Extension, GetInputFocus, ListExtensions, QueryExtension, RenderQueryPictFormats, Setup,
    EXTENSIONS,
};
use crate::xcon::consts::RENDER_PICT_TYPE_DIRECT;
pub use crate::xcon::formatter::Formatter;
use crate::xcon::incoming::handle_incoming;
use crate::xcon::outgoing::handle_outgoing;
pub use crate::xcon::parser::Parser;
pub use crate::xcon::wire_type::{Message, Request, XEvent};
use crate::xcon::xauthority::{XAuthority, LOCAL, MIT_MAGIC_COOKIE};
use crate::{AsyncEngine, AsyncError, AsyncQueue, CloneCell, ErrorFmt, NumCell, Phase};
use ahash::AHashMap;
use bstr::{BString, ByteSlice};
use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::io::Write;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};
use std::{mem, ptr};
use thiserror::Error;
use uapi::{c, OwnedFd};

pub mod consts;
mod formatter;
mod incoming;
mod outgoing;
mod parser;
mod wire_type;
mod xauthority;

#[derive(Debug, Error)]
pub enum XconError {
    #[error("Unexpected EOF")]
    UnexpectedEof,
    #[error("Buffer slice is not properly aligned")]
    UnalignedSlice,
    #[error("Neither XAUTHORITY nor HOME is set")]
    HomeNotSet,
    #[error("Could not read Xauthority file")]
    ReadXAuthority(#[source] std::io::Error),
    #[error("Display field in Xauthority could not be parsed")]
    InvalidAuthorityDisplay,
    #[error("The DISPLAY is not set")]
    DisplayNotSet,
    #[error("DISPLAY contains an invalid value")]
    InvalidDisplayFormat,
    #[error("Could not create a unix socket")]
    CreateSocket(#[source] OsError),
    #[error("Could not connect to Xserver")]
    ConnectSocket(#[source] OsError),
    #[error("Could not retrive the hostname")]
    Hostname(#[source] OsError),
    #[error("Server did not send enough fds")]
    NotEnoughFds,
    #[error("Server rejected our connection attempt: {0}")]
    Connect(BString),
    #[error("Server requires additional authentication: {0}")]
    Authenticate(BString),
    #[error(transparent)]
    AsyncError(#[from] AsyncError),
    #[error(transparent)]
    BufIoError(#[from] BufIoError),
    #[error("The server did not send a reply to a request")]
    MissingReply,
    #[error("The server did not send fds with a reply")]
    MissingFds,
    #[error("The server sent a message with an excessive size")]
    ExcessiveMessageSize,
    #[error(transparent)]
    XconError(Rc<XconError>),
    #[error("The server does not support the `{0}` extension")]
    ExtensionUnavailable(&'static str),
    #[error("The server returned error {0}")]
    CoreError(u8),
    #[error("The extension `{}` returned error {1}", .0.name())]
    ExtensionError(Extension, u8),
    #[error("The connection to the server has already been closed")]
    Dead,
    #[error("Could not query the `{0}` extension")]
    QueryExtension(BString, #[source] Box<XconError>),
    #[error("All available xids have been used")]
    XidExhausted,
    #[error("Enum contains an unknown variant")]
    UnknownEnumVariant,
    #[error("Could not query the render pict formats")]
    QueryPictFormats(#[source] Box<XconError>),
    #[error("The server does not support the picture format for cursors")]
    CursorFormatNotSupported,
}

#[derive(Debug)]
struct ExtensionIdRange {
    name: BString,
    extension: Option<Extension>,
    first: u8,
}

#[derive(Default, Debug)]
struct ExtensionData {
    opcodes: [Option<u8>; EXTENSIONS.len()],
    ext_by_opcode: AHashMap<u8, Extension>,
    events: Vec<ExtensionIdRange>,
    errors: Vec<ExtensionIdRange>,
}

pub struct Xcon {
    data: Rc<XconData>,
    outgoing: Cell<Option<SpawnedFuture<()>>>,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    setup: Reply<Setup<'static>>,
    extensions: Rc<ExtensionData>,

    xid_next: Cell<u32>,
    xid_inc: u32,
    xid_max: u32,
}

struct XconData {
    bufio: Rc<BufIo>,
    next_serial: NumCell<u64>,
    last_recv_serial: Cell<u64>,
    reply_handlers: RefCell<VecDeque<Box<dyn ReplyHandler>>>,
    dead: Cell<bool>,
    need_sync: Cell<bool>,
    extensions: CloneCell<Option<Rc<ExtensionData>>>,
    xorg: CloneCell<Weak<Xcon>>,
    events: AsyncQueue<Event>,
}

pub struct Reply<T: Message<'static>> {
    bufio: Rc<BufIo>,
    buf: Vec<u8>,
    t: T::Generic<'static>,
}

pub struct Event {
    bufio: Rc<BufIo>,
    ext: Option<Extension>,
    code: u16,
    buf: Vec<u8>,
}

impl Deref for Event {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl Event {
    pub fn ext(&self) -> Option<Extension> {
        self.ext
    }

    pub fn code(&self) -> u16 {
        self.code
    }

    pub fn parse<'a, M: Message<'a>>(&'a self) -> Result<M, XconError> {
        let mut parser = Parser::new(&self.buf, vec![]);
        M::deserialize(&mut parser)
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        self.bufio.add_buf(mem::take(&mut self.buf));
    }
}

impl<T: Message<'static>> Reply<T> {
    pub fn get<'a>(&'a self) -> &'a T::Generic<'a> {
        unsafe { mem::transmute(&self.t) }
    }
}

impl<T: Message<'static>> Drop for Reply<T> {
    fn drop(&mut self) {
        if self.buf.capacity() > 0 {
            self.bufio.add_buf(mem::take(&mut self.buf));
        }
    }
}

unsafe trait ReplyHandler {
    fn has_fds(&self) -> bool;
    fn serial(&self) -> u64;
    fn handle_result(
        self: Box<Self>,
        bufio: &Rc<BufIo>,
        parser: &mut Parser<'static>,
        buf: Vec<u8>,
    ) -> Result<(), XconError>;
    fn handle_noreply(self: Box<Self>, bufio: &Rc<BufIo>) -> Result<(), XconError>;
    fn handle_error(self: Box<Self>, error: XconError);
}

struct AsyncReplyHandler<T: Message<'static>> {
    serial: u64,
    slot: Weak<AsyncReplySlot<T>>,
}

impl<T: Message<'static>> AsyncReplyHandler<T> {
    fn done(self: Box<Self>, res: Result<Reply<T>, XconError>) {
        if let Some(slot) = self.slot.upgrade() {
            slot.data.set(Some(res));
            if let Some(waker) = slot.waker.take() {
                waker.wake();
            }
        }
    }
}

unsafe impl<T: Message<'static>> ReplyHandler for AsyncReplyHandler<T> {
    fn has_fds(&self) -> bool {
        T::HAS_FDS
    }

    fn serial(&self) -> u64 {
        self.serial
    }

    fn handle_result(
        self: Box<Self>,
        bufio: &Rc<BufIo>,
        parser: &mut Parser<'static>,
        buf: Vec<u8>,
    ) -> Result<(), XconError> {
        let msg = <T::Generic<'static> as Message<'static>>::deserialize(parser);
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                let e = Rc::new(e);
                self.done(Err(XconError::XconError(e.clone())));
                return Err(XconError::XconError(e));
            }
        };
        let reply = Reply {
            bufio: bufio.clone(),
            buf,
            t: msg,
        };
        self.done(Ok(reply));
        Ok(())
    }

    fn handle_noreply(self: Box<Self>, bufio: &Rc<BufIo>) -> Result<(), XconError> {
        if TypeId::of::<T::Generic<'static>>() == TypeId::of::<()>() {
            let reply = Reply {
                bufio: bufio.clone(),
                buf: vec![],
                t: unsafe { ptr::read(&() as *const () as *const T::Generic<'static>) },
            };
            self.done(Ok(reply));
            Ok(())
        } else {
            self.done(Err(XconError::MissingReply));
            Err(XconError::MissingReply)
        }
    }

    fn handle_error(self: Box<Self>, error: XconError) {
        self.done(Err(error))
    }
}

struct AsyncReplySlot<T: Message<'static>> {
    data: Cell<Option<Result<Reply<T>, XconError>>>,
    waker: Cell<Option<Waker>>,
}

pub struct AsyncReply<T: Message<'static>> {
    slot: Rc<AsyncReplySlot<T>>,
    xorg: Rc<XconData>,
}

impl<T: Message<'static>> Future for AsyncReply<T> {
    type Output = Result<Reply<T>, XconError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(d) = self.slot.data.take() {
            Poll::Ready(d)
        } else {
            self.slot.waker.set(Some(cx.waker().clone()));
            self.xorg.send_sync();
            Poll::Pending
        }
    }
}

impl Xcon {
    pub fn setup(&self) -> &Setup {
        self.setup.get()
    }

    pub async fn event(&self) -> Event {
        self.data.events.pop().await
    }

    pub fn generate_id(&self) -> Result<u32, XconError> {
        if self.xid_next.get() == self.xid_max {
            return Err(XconError::XidExhausted);
        }
        let id = self.xid_next.get();
        self.xid_next.set(id + self.xid_inc);
        Ok(id)
    }

    pub async fn connect(eng: Rc<AsyncEngine>) -> Result<Rc<Self>, XconError> {
        let authority = match XAuthority::load() {
            Ok(a) => a,
            Err(e) => {
                log::warn!(
                    "Could not parse Xauthority file. Proceeding without authorization: {}",
                    ErrorFmt(e)
                );
                vec![]
            }
        };
        let display = parse_display()?;
        let mut addr = c::sockaddr_un {
            sun_family: c::AF_UNIX as _,
            ..uapi::pod_zeroed()
        };
        {
            let mut path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
            let _ = write!(path, "/tmp/.X11-unix/X{}", display);
        }
        let fd = match uapi::socket(
            c::AF_UNIX,
            c::SOCK_STREAM | c::SOCK_CLOEXEC | c::SOCK_NONBLOCK,
            0,
        ) {
            Ok(fd) => Rc::new(fd),
            Err(e) => return Err(XconError::CreateSocket(e.into())),
        };
        if let Err(e) = uapi::connect(fd.raw(), &addr) {
            return Err(XconError::ConnectSocket(e.into()));
        }
        let mut hnbuf = [MaybeUninit::<u8>::uninit(); 256];
        let hn = match uapi::gethostname(&mut hnbuf[..]) {
            Ok(hn) => hn.to_bytes(),
            Err(e) => return Err(XconError::Hostname(e.into())),
        };
        let (auth_method, auth_value) = 'auth: {
            for auth in &authority {
                if auth.display == display
                    && auth.family == LOCAL
                    && auth.host == hn
                    && auth.method == MIT_MAGIC_COOKIE
                {
                    break 'auth (&auth.method[..], &auth.value[..]);
                }
            }
            (&[], &[])
        };
        Self::connect_to_fd(&eng, &fd, auth_method, auth_value).await
    }

    pub async fn connect_to_fd(
        eng: &Rc<AsyncEngine>,
        fd: &Rc<OwnedFd>,
        auth_method: &[u8],
        auth_value: &[u8],
    ) -> Result<Rc<Self>, XconError> {
        let fd = eng.fd(fd)?;
        let data = Rc::new(XconData {
            bufio: Rc::new(BufIo::new(fd)),
            next_serial: NumCell::new(1),
            last_recv_serial: Cell::new(0),
            reply_handlers: Default::default(),
            dead: Cell::new(false),
            need_sync: Cell::new(false),
            extensions: Default::default(),
            xorg: CloneCell::new(Weak::new()),
            events: Default::default(),
        });
        let outgoing = eng.spawn2(Phase::PostLayout, handle_outgoing(data.clone()));
        let mut buf = data.bufio.buf();
        let mut fds = vec![];
        {
            let mut formatter = Formatter::new(&mut fds, &mut buf, 0);
            #[cfg(target_endian = "little")]
            const ENDIAN: u8 = b'l';
            #[cfg(target_endian = "big")]
            const ENDIAN: u8 = b'B';
            formatter.write_packed(&ENDIAN);
            formatter.pad(1);
            formatter.write_packed(&11u16);
            formatter.write_packed(&0u16);
            formatter.write_packed(&(auth_method.len() as u16));
            formatter.write_packed(&(auth_value.len() as u16));
            formatter.pad(2);
            formatter.write_packed(auth_method.as_bytes());
            formatter.align(4);
            formatter.write_packed(auth_value.as_bytes());
            formatter.align(4);
        }
        data.bufio.send(BufIoMessage { fds, buf });
        let mut incoming = data.bufio.incoming();
        let mut buf = data.bufio.buf();
        incoming.fill_msg_buf(8, &mut buf).await?;
        let len = u16::from_ne_bytes([buf[6], buf[7]]) as usize * 4;
        incoming.fill_msg_buf(len, &mut buf).await?;
        let mut parser = Parser::new(&buf, vec![]);
        let res: u8 = buf[0];
        if res == 0 {
            parser.pad(1)?;
            let reason_len: u8 = parser.read_pod()?;
            parser.pad(6)?;
            let reason = parser.read_string(reason_len as usize)?;
            return Err(XconError::Connect(reason.to_owned()));
        }
        if res == 2 {
            parser.pad(6)?;
            let reason_len: u16 = parser.read_pod()?;
            let reason = parser.read_string(reason_len as usize * 4)?;
            return Err(XconError::Authenticate(reason.to_owned()));
        }
        let setup = Setup::deserialize(&mut parser)?;
        let incoming = eng.spawn(handle_incoming(data.clone(), incoming));
        let slf = Rc::new(Self {
            extensions: data.fetch_extension_data().await?,
            outgoing: Cell::new(Some(outgoing)),
            incoming: Cell::new(Some(incoming)),
            xid_next: Cell::new(setup.resource_id_base),
            xid_inc: 1 << setup.resource_id_mask.trailing_zeros(),
            xid_max: setup.resource_id_mask | setup.resource_id_base,
            setup: Reply {
                bufio: data.bufio.clone(),
                t: unsafe { mem::transmute(setup) },
                buf,
            },
            data,
        });
        slf.data.xorg.set(Rc::downgrade(&slf));
        Ok(slf)
    }

    pub fn call<'a, T: Request<'a>>(self: &Rc<Self>, t: &T) -> AsyncReply<T::Reply> {
        self.data.call(t, &self.extensions)
    }

    pub async fn find_cursor_format(self: &Rc<Self>) -> Result<u32, XconError> {
        let res = match self.call(&RenderQueryPictFormats {}).await {
            Ok(r) => r,
            Err(e) => return Err(XconError::QueryPictFormats(Box::new(e))),
        };
        for format in res.get().formats.iter() {
            let valid = format.ty == RENDER_PICT_TYPE_DIRECT
                && format.depth == 32
                && format.direct.red_shift == 16
                && format.direct.red_mask == 0xff
                && format.direct.green_shift == 8
                && format.direct.green_mask == 0xff
                && format.direct.blue_shift == 0
                && format.direct.blue_mask == 0xff
                && format.direct.alpha_shift == 24
                && format.direct.alpha_mask == 0xff;
            if valid {
                return Ok(format.id);
            }
        }
        Err(XconError::CursorFormatNotSupported)
    }
}

impl XconData {
    fn kill(&self) {
        self.bufio.shutdown();
        self.dead.set(true);
        let handlers = mem::take(self.reply_handlers.borrow_mut().deref_mut());
        for handler in handlers {
            handler.handle_error(XconError::Dead);
        }
        if let Some(xorg) = self.xorg.get().upgrade() {
            xorg.outgoing.take();
            xorg.incoming.take();
        }
    }

    fn call<'a, T: Request<'a>>(
        self: &Rc<Self>,
        t: &T,
        extensions: &ExtensionData,
    ) -> AsyncReply<T::Reply> {
        if self.dead.get() {
            return AsyncReply {
                slot: Rc::new(AsyncReplySlot {
                    data: Cell::new(Some(Err(XconError::Dead))),
                    waker: Cell::new(None),
                }),
                xorg: self.clone(),
            };
        }
        let opcode = match T::EXTENSION {
            None => 0,
            Some(idx) => match extensions.opcodes[idx] {
                Some(o) => o,
                _ => {
                    return AsyncReply {
                        slot: Rc::new(AsyncReplySlot {
                            data: Cell::new(Some(Err(XconError::ExtensionUnavailable(
                                EXTENSIONS[idx].name(),
                            )))),
                            waker: Cell::new(None),
                        }),
                        xorg: self.clone(),
                    }
                }
            },
        };
        let mut fds = vec![];
        let mut buf = self.bufio.buf();
        let mut formatter = Formatter::new(&mut fds, &mut buf, opcode);
        t.serialize(&mut formatter);
        formatter.write_request_length();
        self.bufio.send(BufIoMessage { fds, buf });
        let slot = Rc::new(AsyncReplySlot {
            data: Cell::new(None),
            waker: Cell::new(None),
        });
        let handler = Box::new(AsyncReplyHandler {
            serial: self.next_serial.fetch_add(1),
            slot: Rc::downgrade(&slot),
        });
        self.reply_handlers.borrow_mut().push_back(handler);
        self.need_sync.set(T::IS_VOID);
        AsyncReply {
            slot,
            xorg: self.clone(),
        }
    }

    fn send_sync(&self) {
        if !self.need_sync.replace(false) {
            return;
        }
        let mut fds = vec![];
        let mut buf = self.bufio.buf();
        let mut formatter = Formatter::new(&mut fds, &mut buf, 0);
        GetInputFocus {}.serialize(&mut formatter);
        formatter.write_request_length();
        self.bufio.send(BufIoMessage { fds, buf });
        self.next_serial.fetch_add(1);
    }

    async fn fetch_extension_data(self: &Rc<Self>) -> Result<Rc<ExtensionData>, XconError> {
        let mut ext_by_name = AHashMap::new();
        for e in EXTENSIONS.iter().copied() {
            ext_by_name.insert(e.name().as_bytes().as_bstr(), e);
        }
        let mut ed = ExtensionData::default();
        let extensions = self.call(&ListExtensions {}, &ed).await?;
        let mut pending = vec![];
        for name in extensions.get().names.iter() {
            pending.push((name.val, self.call(&QueryExtension { name: name.val }, &ed)));
        }
        for (name, data) in pending {
            let data = match data.await {
                Ok(d) => d,
                Err(e) => return Err(XconError::QueryExtension(name.to_owned(), Box::new(e))),
            };
            let data = data.get();
            if data.present != 0 {
                let e = ext_by_name.get(name).copied();
                if data.first_event > 0 {
                    ed.events.push(ExtensionIdRange {
                        name: name.to_owned(),
                        extension: e,
                        first: data.first_event,
                    });
                }
                if data.first_error > 0 {
                    ed.errors.push(ExtensionIdRange {
                        name: name.to_owned(),
                        extension: e,
                        first: data.first_error,
                    });
                }
                if let Some(e) = e {
                    ed.opcodes[e as usize] = Some(data.major_opcode);
                    ed.ext_by_opcode.insert(data.major_opcode, e);
                }
            }
        }
        ed.events.sort_by_key(|e| e.first);
        ed.errors.sort_by_key(|e| e.first);
        let ed = Rc::new(ed);
        self.extensions.set(Some(ed.clone()));
        Ok(ed)
    }
}

fn parse_display() -> Result<u32, XconError> {
    let display = match std::env::var("DISPLAY") {
        Ok(d) => d,
        _ => return Err(XconError::DisplayNotSet),
    };
    let num = match display.strip_prefix(":") {
        Some(p) => p,
        _ => return Err(XconError::InvalidDisplayFormat),
    };
    let num = match num.parse() {
        Ok(v) => v,
        _ => return Err(XconError::InvalidDisplayFormat),
    };
    Ok(num)
}
