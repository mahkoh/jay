pub use crate::xcon::{
    formatter::Formatter,
    parser::Parser,
    wire_type::{Message, Request, XEvent},
};
use {
    crate::{
        async_engine::{Phase, SpawnedFuture},
        compositor::DISPLAY,
        io_uring::IoUringError,
        state::State,
        utils::{
            buf::DynamicBuf,
            bufio::{BufIo, BufIoError, BufIoMessage},
            clonecell::CloneCell,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            oserror::OsError,
            queue::AsyncQueue,
            stack::Stack,
            vec_ext::VecExt,
        },
        wire_xcon::{
            CreateGC, CreatePixmap, Extension, FreeGC, FreePixmap, GetInputFocus, GetProperty,
            ListExtensions, PutImage, QueryExtension, RenderCreateCursor, RenderCreatePicture,
            RenderQueryPictFormats, Setup, EXTENSIONS,
        },
        xcon::{
            consts::{IMAGE_FORMAT_Z_PIXMAP, RENDER_PICT_TYPE_DIRECT},
            incoming::handle_incoming,
            outgoing::handle_outgoing,
            wire_type::SendEvent,
            xauthority::{XAuthority, LOCAL, MIT_MAGIC_COOKIE},
        },
    },
    ahash::AHashMap,
    bstr::{BString, ByteSlice},
    std::{
        any::TypeId,
        cell::{Cell, RefCell},
        collections::VecDeque,
        fmt::Debug,
        future::Future,
        io::Write,
        mem::{self, MaybeUninit},
        ops::{Deref, DerefMut},
        pin::Pin,
        ptr,
        rc::{Rc, Weak},
        task::{Context, Poll, Waker},
    },
    thiserror::Error,
    uapi::{c, OwnedFd},
};

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
    ConnectSocket(#[source] IoUringError),
    #[error("Could not retrive the hostname")]
    Hostname(#[source] OsError),
    #[error("Server did not send enough fds")]
    NotEnoughFds,
    #[error("Server rejected our connection attempt: {0}")]
    Connect(BString),
    #[error("Server requires additional authentication: {0}")]
    Authenticate(BString),
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
    #[error("Could not create a pixmap")]
    CreatePixmap(#[source] Box<XconError>),
    #[error("Could not create a graphics context")]
    CreateGc(#[source] Box<XconError>),
    #[error("Could not upload an image")]
    PutImage(#[source] Box<XconError>),
    #[error("Could not create a picture")]
    CreatePicture(#[source] Box<XconError>),
    #[error("Could not create a cursor")]
    CreateCursor(#[source] Box<XconError>),
    #[error("Property has an invalid type")]
    InvalidPropertyType,
    #[error("Property has an invalid format. Expected: {0}; Actual: {1}")]
    InvalidPropertyFormat(u8, u8),
    #[error("Length of the property data is not a multiple of its format")]
    IrregularPropertyLength,
    #[error("The property is not set")]
    PropertyUnavailable,
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
    first_event: [Option<u8>; EXTENSIONS.len()],
    ext_by_opcode: AHashMap<u8, Extension>,
    events: Vec<ExtensionIdRange>,
    errors: Vec<ExtensionIdRange>,
}

pub struct Xcon {
    data: Rc<XconData>,
    outgoing: Cell<Option<SpawnedFuture<()>>>,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    root_window: u32,
    extensions: Rc<ExtensionData>,

    xid_next: Cell<u32>,
    xid_inc: u32,
    xid_max: u32,
}

impl Drop for Xcon {
    fn drop(&mut self) {
        self.data.kill();
    }
}

struct XconData {
    bufio: Rc<BufIo>,
    in_bufs: Stack<Vec<u8>>,
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
    socket: Rc<XconData>,
    buf: Vec<u8>,
    t: T::Generic<'static>,
}

impl<T: Message<'static>> Debug for Reply<T>
where
    T::Generic<'static>: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.t.fmt(f)
    }
}

pub struct Event {
    socket: Rc<XconData>,
    ext: Option<Extension>,
    code: u16,
    buf: Vec<u8>,
    serial: u64,
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

    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn parse<'a, M: Message<'a>>(&'a self) -> Result<M, XconError> {
        let mut parser = Parser::new(&self.buf, vec![]);
        let res = M::deserialize(&mut parser);
        if let Ok(res) = &res {
            log::trace!("event {:?}", res);
        }
        res
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        self.socket.in_bufs.push(mem::take(&mut self.buf));
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
            self.socket.in_bufs.push(mem::take(&mut self.buf));
        }
    }
}

unsafe trait ReplyHandler {
    fn has_fds(&self) -> bool;
    fn serial(&self) -> u64;
    fn handle_result(
        self: Box<Self>,
        socket: &Rc<XconData>,
        parser: &mut Parser<'static>,
        buf: Vec<u8>,
    ) -> Result<(), XconError>;
    fn handle_noreply(self: Box<Self>, bufio: &Rc<XconData>) -> Result<(), XconError>;
    fn handle_error(self: Box<Self>, error: XconError);
}

struct AsyncReplyHandler<T: Message<'static>> {
    serial: u64,
    slot: Weak<AsyncReplySlot<T>>,
}

impl<T: Message<'static>> AsyncReplyHandler<T> {
    fn done(self, res: Result<Reply<T>, XconError>) {
        if let Some(slot) = self.slot.upgrade() {
            slot.data.set(Some(res));
            if let Some(waker) = slot.waker.take() {
                waker.wake();
            }
        } else if let Err(e) = res {
            log::error!(
                "Received an error whose handler has already been dropped: {}",
                ErrorFmt(e)
            );
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
        socket: &Rc<XconData>,
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
        log::trace!("result {:?}", msg);
        let reply = Reply {
            socket: socket.clone(),
            buf,
            t: msg,
        };
        self.done(Ok(reply));
        Ok(())
    }

    fn handle_noreply(self: Box<Self>, socket: &Rc<XconData>) -> Result<(), XconError> {
        if TypeId::of::<T::Generic<'static>>() == TypeId::of::<()>() {
            let reply = Reply {
                socket: socket.clone(),
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
    pub fn root_window(&self) -> u32 {
        self.root_window
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

    pub async fn connect(state: &Rc<State>) -> Result<Rc<Self>, XconError> {
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
        let fd = match uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0) {
            Ok(fd) => Rc::new(fd),
            Err(e) => return Err(XconError::CreateSocket(e.into())),
        };
        if let Err(e) = state.ring.connect(&fd, &addr).await {
            return Err(XconError::ConnectSocket(e));
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
        Self::connect_to_fd(state, &fd, auth_method, auth_value).await
    }

    pub async fn connect_to_fd(
        state: &Rc<State>,
        fd: &Rc<OwnedFd>,
        auth_method: &[u8],
        auth_value: &[u8],
    ) -> Result<Rc<Self>, XconError> {
        let data = Rc::new(XconData {
            bufio: Rc::new(BufIo::new(fd, &state.ring)),
            in_bufs: Default::default(),
            next_serial: NumCell::new(1),
            last_recv_serial: Cell::new(0),
            reply_handlers: Default::default(),
            dead: Cell::new(false),
            need_sync: Cell::new(false),
            extensions: Default::default(),
            xorg: CloneCell::new(Weak::new()),
            events: Default::default(),
        });
        let outgoing = state
            .eng
            .spawn2(Phase::PostLayout, handle_outgoing(data.clone()));
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
        data.bufio.send(BufIoMessage {
            fds,
            buf: buf.unwrap(),
        });
        let mut incoming = data.bufio.incoming();
        let mut buf = data.in_bufs.pop().unwrap_or_default();
        buf.clear();
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
        let incoming = state.eng.spawn(handle_incoming(data.clone(), incoming));
        let slf = Rc::new(Self {
            extensions: data.fetch_extension_data().await?,
            outgoing: Cell::new(Some(outgoing)),
            incoming: Cell::new(Some(incoming)),
            xid_next: Cell::new(setup.resource_id_base),
            xid_inc: 1 << setup.resource_id_mask.trailing_zeros(),
            xid_max: setup.resource_id_mask | setup.resource_id_base,
            root_window: setup.screens[0].root,
            data,
        });
        slf.data.xorg.set(Rc::downgrade(&slf));
        Ok(slf)
    }

    pub fn call<'a, T: Request<'a>>(self: &Rc<Self>, t: &T) -> AsyncReply<T::Reply> {
        log::trace!("send {:?}", t);
        self.data.call_with_serial(t, &self.extensions).0
    }

    pub fn call_with_serial<'a, T: Request<'a>>(
        self: &Rc<Self>,
        t: &T,
    ) -> (AsyncReply<T::Reply>, u64) {
        log::trace!("send {:?}", t);
        self.data.call_with_serial(t, &self.extensions)
    }

    pub fn send_event<'a, T: XEvent<'a>>(
        self: &Rc<Self>,
        propagate: bool,
        destination: u32,
        event_mask: u32,
        t: &T,
    ) -> AsyncReply<()> {
        log::trace!("send {:?}", t);
        self.data
            .send_event(t, &self.extensions, propagate, destination, event_mask)
    }

    pub async fn get_property3<T: PropertyType>(
        self: &Rc<Self>,
        window: u32,
        property: u32,
        ty: u32,
        delete: bool,
        buf: &mut Vec<T>,
    ) -> Result<u32, XconError> {
        let len = buf.len();
        match self.get_property2(window, property, ty, delete, buf).await {
            Ok(n) => Ok(n),
            Err(e) => {
                buf.truncate(len);
                Err(e)
            }
        }
    }

    pub async fn get_property<T: PropertyType>(
        self: &Rc<Self>,
        window: u32,
        property: u32,
        ty: u32,
        buf: &mut Vec<T>,
    ) -> Result<u32, XconError> {
        let len = buf.len();
        match self.get_property2(window, property, ty, false, buf).await {
            Ok(n) => Ok(n),
            Err(e) => {
                buf.truncate(len);
                Err(e)
            }
        }
    }

    async fn get_property2<T: PropertyType>(
        self: &Rc<Self>,
        window: u32,
        property: u32,
        ty: u32,
        delete: bool,
        buf: &mut Vec<T>,
    ) -> Result<u32, XconError> {
        let mut gp = GetProperty {
            delete: delete as _,
            window,
            property,
            ty,
            long_offset: 0,
            long_length: 128,
        };
        let format = mem::size_of::<T>() as u8 * 8;
        loop {
            let res = self.call(&gp).await?;
            let res = res.get();
            if res.format == 0 {
                return Err(XconError::PropertyUnavailable);
            }
            if gp.ty != 0 && gp.ty != res.ty {
                return Err(XconError::InvalidPropertyType);
            }
            gp.ty = res.ty;
            if res.format != format {
                return Err(XconError::InvalidPropertyFormat(format, res.format));
            }
            if res.data.len() % mem::size_of::<T>() != 0 {
                return Err(XconError::IrregularPropertyLength);
            }
            let len = res.data.len() / mem::size_of::<T>();
            buf.reserve(len);
            let (_, uninit) = buf.split_at_spare_mut_bytes_ext();
            uninit[..res.data.len()].copy_from_slice(uapi::as_maybe_uninit_bytes(res.data));
            unsafe {
                buf.set_len(buf.len() + len);
            }
            if res.bytes_after == 0 {
                return Ok(res.ty);
            }
            gp.long_offset += gp.long_length;
        }
    }

    pub async fn create_cursor(
        self: &Rc<Self>,
        pixels: &[Cell<u8>],
        width: i32,
        height: i32,
        xhot: i32,
        yhot: i32,
    ) -> Result<u32, XconError> {
        let cursor_format = 'cursor_format: {
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
                    break 'cursor_format format.id;
                }
            }
            return Err(XconError::CursorFormatNotSupported);
        };
        let pixmap = self.generate_id()?;
        let gc = self.generate_id()?;
        let picture = self.generate_id()?;
        let cursor = self.generate_id()?;
        let create_pixmap = self.call(&CreatePixmap {
            depth: 32,
            pid: pixmap,
            drawable: self.root_window,
            width: width as _,
            height: height as _,
        });
        let create_gc = self.call(&CreateGC {
            cid: gc,
            drawable: pixmap,
            values: Default::default(),
        });
        let put_image = self.call(&PutImage {
            format: IMAGE_FORMAT_Z_PIXMAP,
            drawable: pixmap,
            gc,
            width: width as _,
            height: height as _,
            dst_x: 0,
            dst_y: 0,
            left_pad: 0,
            depth: 32,
            data: unsafe { mem::transmute(pixels) },
        });
        self.call(&FreeGC { gc });
        let create_picture = self.call(&RenderCreatePicture {
            pid: picture,
            drawable: pixmap,
            format: cursor_format,
            values: Default::default(),
        });
        self.call(&FreePixmap { pixmap });
        let create_cursor = self.call(&RenderCreateCursor {
            cid: cursor,
            source: picture,
            x: xhot as _,
            y: yhot as _,
        });
        if let Err(e) = create_pixmap.await {
            return Err(XconError::CreatePixmap(Box::new(e)));
        }
        if let Err(e) = create_gc.await {
            return Err(XconError::CreateGc(Box::new(e)));
        }
        if let Err(e) = put_image.await {
            return Err(XconError::PutImage(Box::new(e)));
        }
        if let Err(e) = create_picture.await {
            return Err(XconError::CreatePicture(Box::new(e)));
        }
        if let Err(e) = create_cursor.await {
            return Err(XconError::CreateCursor(Box::new(e)));
        }
        Ok(cursor)
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

    #[cold]
    fn dead<T: Message<'static>>(self: &Rc<Self>) -> AsyncReply<T> {
        AsyncReply {
            slot: Rc::new(AsyncReplySlot {
                data: Cell::new(Some(Err(XconError::Dead))),
                waker: Cell::new(None),
            }),
            xorg: self.clone(),
        }
    }

    #[cold]
    fn ext_unavailable<T: Message<'static>>(self: &Rc<Self>, idx: usize) -> AsyncReply<T> {
        AsyncReply {
            slot: Rc::new(AsyncReplySlot {
                data: Cell::new(Some(Err(XconError::ExtensionUnavailable(
                    EXTENSIONS[idx].name(),
                )))),
                waker: Cell::new(None),
            }),
            xorg: self.clone(),
        }
    }

    fn send_event<'a, T: XEvent<'a>>(
        self: &Rc<Self>,
        t: &T,
        extensions: &ExtensionData,
        propagate: bool,
        destination: u32,
        event_mask: u32,
    ) -> AsyncReply<()> {
        if self.dead.get() {
            return self.dead();
        }
        let first_event = match T::EXTENSION {
            None => 0,
            Some(idx) => match extensions.first_event[idx] {
                Some(o) => o,
                _ => return self.ext_unavailable(idx),
            },
        };
        let mut fds = vec![];
        let mut buf = self.bufio.buf();
        let mut formatter = Formatter::new(&mut fds, &mut buf, 0);
        let se = SendEvent {
            propagate: propagate as u8,
            destination,
            event_mask,
        };
        se.serialize(&mut formatter);
        t.serialize(&mut formatter);
        formatter.pad_to(44);
        formatter.write_request_length();
        buf[12] = first_event + T::OPCODE as u8;
        self.need_sync.set(true);
        self.send(fds, buf).0
    }

    fn call<'a, T: Request<'a>>(
        self: &Rc<Self>,
        t: &T,
        extensions: &ExtensionData,
    ) -> AsyncReply<T::Reply> {
        self.call_with_serial(t, extensions).0
    }

    fn call_with_serial<'a, T: Request<'a>>(
        self: &Rc<Self>,
        t: &T,
        extensions: &ExtensionData,
    ) -> (AsyncReply<T::Reply>, u64) {
        if self.dead.get() {
            return (self.dead(), 0);
        }
        let opcode = match T::EXTENSION {
            None => 0,
            Some(idx) => match extensions.opcodes[idx] {
                Some(o) => o,
                _ => return (self.ext_unavailable(idx), 0),
            },
        };
        let mut fds = vec![];
        let mut buf = self.bufio.buf();
        let mut formatter = Formatter::new(&mut fds, &mut buf, opcode);
        t.serialize(&mut formatter);
        formatter.align(4);
        formatter.write_request_length();
        self.need_sync.set(T::IS_VOID);
        self.send(fds, buf)
    }

    fn send<T: Message<'static>>(
        self: &Rc<Self>,
        fds: Vec<Rc<OwnedFd>>,
        buf: DynamicBuf,
    ) -> (AsyncReply<T>, u64) {
        self.bufio.send(BufIoMessage {
            fds,
            buf: buf.unwrap(),
        });
        let slot = Rc::new(AsyncReplySlot {
            data: Cell::new(None),
            waker: Cell::new(None),
        });
        let serial = self.next_serial.fetch_add(1);
        let handler = Box::new(AsyncReplyHandler {
            serial,
            slot: Rc::downgrade(&slot),
        });
        self.reply_handlers.borrow_mut().push_back(handler);
        let rep = AsyncReply {
            slot,
            xorg: self.clone(),
        };
        (rep, serial)
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
        self.bufio.send(BufIoMessage {
            fds,
            buf: buf.unwrap(),
        });
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
                    ed.first_event[e as usize] = Some(data.first_event);
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
    let display = match std::env::var(DISPLAY) {
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

pub unsafe trait PropertyType {}

unsafe impl PropertyType for u8 {}
unsafe impl PropertyType for u16 {}
unsafe impl PropertyType for u32 {}
