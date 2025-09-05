use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        client::{EventFormatter, RequestParser},
        compositor::WAYLAND_DISPLAY,
        io_uring::{IoUring, IoUringError},
        logger::Logger,
        object::{ObjectId, WL_DISPLAY_ID},
        utils::{
            asyncevent::AsyncEvent,
            bitfield::Bitfield,
            buffd::{
                BufFdError, BufFdIn, BufFdOut, MsgFormatter, MsgParser, MsgParserError, OutBuffer,
                OutBufferSwapchain,
            },
            clonecell::CloneCell,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            oserror::OsError,
            stack::Stack,
            vec_ext::VecExt,
            xrd::xrd,
        },
        wheel::{Wheel, WheelError},
        wire::{
            JayCompositor, JayCompositorId, JayDamageTracking, JayDamageTrackingId, JayToplevelId,
            JayWorkspaceId, WlCallbackId, WlRegistryId, WlSeatId, jay_compositor,
            jay_select_toplevel, jay_select_workspace, jay_toplevel, wl_callback, wl_display,
            wl_registry,
        },
    },
    ahash::AHashMap,
    log::Level,
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        future::{Future, Pending},
        mem,
        rc::Rc,
        sync::Arc,
    },
    thiserror::Error,
    uapi::{c, format_ustr},
};

#[derive(Debug, Error)]
pub enum ToolClientError {
    #[error("Could not create a timer wheel")]
    CreateWheel(#[source] WheelError),
    #[error("Could not create an io-uring")]
    CreateRing(#[source] IoUringError),
    #[error("XDG_RUNTIME_DIR is not set")]
    XrdNotSet,
    #[error("WAYLAND_DISPLAY is not set")]
    WaylandDisplayNotSet,
    #[error("Could not create a socket")]
    CreateSocket(#[source] OsError),
    #[error("The socket path is too long")]
    SocketPathTooLong,
    #[error("Could not connect to the compositor")]
    Connect(#[source] IoUringError),
    #[error("The message length is smaller than 8 bytes")]
    MsgLenTooSmall,
    #[error("The size of the message is not a multiple of 4")]
    UnalignedMessage,
    #[error(transparent)]
    BufFdError(#[from] BufFdError),
    #[error("Could not parse a message of type {}", .0)]
    Parsing(&'static str, #[source] MsgParserError),
    #[error("Could not read from the compositor")]
    Read(#[source] BufFdError),
    #[error("Could not write to the compositor")]
    Write(#[source] BufFdError),
}

pub struct ToolClient {
    pub _logger: Arc<Logger>,
    pub ring: Rc<IoUring>,
    pub _wheel: Rc<Wheel>,
    pub eng: Rc<AsyncEngine>,
    obj_ids: RefCell<Bitfield>,
    handlers: RefCell<
        AHashMap<
            ObjectId,
            AHashMap<u32, Rc<dyn Fn(&mut MsgParser) -> Result<(), ToolClientError>>>,
        >,
    >,
    bufs: Stack<Vec<u32>>,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    pending_futures: RefCell<AHashMap<u32, SpawnedFuture<()>>>,
    next_id: NumCell<u32>,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    outgoing: Cell<Option<SpawnedFuture<()>>>,
    singletons: CloneCell<Option<Rc<Singletons>>>,
    jay_compositor: Cell<Option<JayCompositorId>>,
    jay_damage_tracking: Cell<Option<Option<JayDamageTrackingId>>>,
}

pub fn with_tool_client<T, F>(level: Level, f: F)
where
    F: FnOnce(Rc<ToolClient>) -> T + 'static,
    T: Future<Output = ()> + 'static,
{
    if let Err(e) = with_tool_client_(level, f) {
        handle_error(e);
    }
}

fn handle_error(e: ToolClientError) -> ! {
    fatal!("Could not create a tool client: {}", ErrorFmt(e));
}

fn with_tool_client_<T, F>(level: Level, f: F) -> Result<(), ToolClientError>
where
    F: FnOnce(Rc<ToolClient>) -> T + 'static,
    T: Future<Output = ()> + 'static,
{
    let logger = Logger::install_stderr(level);
    let eng = AsyncEngine::new();
    let ring = match IoUring::new(&eng, 32) {
        Ok(e) => e,
        Err(e) => return Err(ToolClientError::CreateRing(e)),
    };
    let eng2 = eng.clone();
    let ring2 = ring.clone();
    let _f = eng.spawn("tool client", async move {
        let tc = match ToolClient::try_new(logger, eng2, ring2).await {
            Ok(tc) => tc,
            Err(e) => handle_error(e),
        };
        f(tc).await;
        std::process::exit(0);
    });
    if let Err(e) = ring.run() {
        fatal!("A fatal error occurred: {}", ErrorFmt(e));
    }
    Ok(())
}

impl ToolClient {
    async fn try_new(
        logger: Arc<Logger>,
        eng: Rc<AsyncEngine>,
        ring: Rc<IoUring>,
    ) -> Result<Rc<Self>, ToolClientError> {
        let wheel = match Wheel::new(&eng, &ring) {
            Ok(w) => w,
            Err(e) => return Err(ToolClientError::CreateWheel(e)),
        };
        let xrd = match xrd() {
            Some(d) => d,
            _ => return Err(ToolClientError::XrdNotSet),
        };
        let wd = match std::env::var(WAYLAND_DISPLAY) {
            Ok(d) => d,
            Err(_) => return Err(ToolClientError::WaylandDisplayNotSet),
        };
        let path = format_ustr!("{}/{}.jay", xrd, wd);
        let socket = match uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0) {
            Ok(s) => Rc::new(s),
            Err(e) => return Err(ToolClientError::CreateSocket(e.into())),
        };
        let mut addr: c::sockaddr_un = uapi::pod_zeroed();
        addr.sun_family = c::AF_UNIX as _;
        if path.len() >= addr.sun_path.len() {
            return Err(ToolClientError::SocketPathTooLong);
        }
        let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
        sun_path[..path.len()].copy_from_slice(path.as_bytes());
        sun_path[path.len()] = 0;
        if let Err(e) = ring.connect(&socket, &addr).await {
            return Err(ToolClientError::Connect(e));
        }
        let mut obj_ids = Bitfield::default();
        obj_ids.take(0);
        obj_ids.take(1);
        let slf = Rc::new(Self {
            _logger: logger,
            ring,
            _wheel: wheel,
            eng,
            obj_ids: RefCell::new(obj_ids),
            handlers: Default::default(),
            bufs: Default::default(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            pending_futures: Default::default(),
            next_id: Default::default(),
            incoming: Default::default(),
            outgoing: Default::default(),
            singletons: Default::default(),
            jay_compositor: Default::default(),
            jay_damage_tracking: Default::default(),
        });
        wl_display::Error::handle(&slf, WL_DISPLAY_ID, (), |_, val| {
            fatal!("The compositor returned a fatal error: {}", val.message);
        });
        wl_display::DeleteId::handle(&slf, WL_DISPLAY_ID, slf.clone(), |tc, val| {
            tc.handlers.borrow_mut().remove(&ObjectId::from_raw(val.id));
            tc.obj_ids.borrow_mut().release(val.id);
        });
        slf.incoming.set(Some(
            slf.eng.spawn(
                "tool client incoming",
                Incoming {
                    tc: slf.clone(),
                    buf: BufFdIn::new(&socket, &slf.ring),
                }
                .run(),
            ),
        ));
        slf.outgoing.set(Some(
            slf.eng.spawn(
                "tool client outgoing",
                Outgoing {
                    tc: slf.clone(),
                    buf: BufFdOut::new(&socket, &slf.ring),
                    buffers: Default::default(),
                }
                .run(),
            ),
        ));
        Ok(slf)
    }

    fn handle<T, F, R, H>(self: &Rc<Self>, id: ObjectId, recv: R, h: H)
    where
        T: RequestParser<'static>,
        F: Future<Output = ()> + 'static,
        R: 'static,
        H: for<'a> Fn(&R, T::Generic<'a>) -> Option<F> + 'static,
    {
        let slf = self.clone();
        let mut handlers = self.handlers.borrow_mut();
        handlers.entry(id).or_default().insert(
            T::ID,
            Rc::new(move |parser| {
                let val = match <T::Generic<'_> as RequestParser<'_>>::parse(parser) {
                    Ok(val) => val,
                    Err(e) => return Err(ToolClientError::Parsing(std::any::type_name::<T>(), e)),
                };
                let res = h(&recv, val);
                if let Some(res) = res {
                    let id = slf.next_id.fetch_add(1);
                    let slf2 = slf.clone();
                    let future = slf.eng.spawn("tool client handler", async move {
                        res.await;
                        slf2.pending_futures.borrow_mut().remove(&id);
                    });
                    slf.pending_futures.borrow_mut().insert(id, future);
                }
                Ok(())
            }),
        );
    }

    pub fn send<M: EventFormatter>(&self, msg: M) {
        let mut fds = vec![];
        let mut swapchain = self.swapchain.borrow_mut();
        let mut fmt = MsgFormatter::new(&mut swapchain.cur, &mut fds);
        msg.format(&mut fmt);
        fmt.write_len();
        if swapchain.cur.is_full() {
            swapchain.commit();
        }
        self.flush_request.trigger();
    }

    pub fn id<T: From<ObjectId>>(&self) -> T {
        let id = self.obj_ids.borrow_mut().acquire();
        ObjectId::from_raw(id).into()
    }

    pub async fn round_trip(self: &Rc<Self>) {
        let callback: WlCallbackId = self.id();
        self.send(wl_display::Sync {
            self_id: WL_DISPLAY_ID,
            callback,
        });
        let ah = Rc::new(AsyncEvent::default());
        wl_callback::Done::handle(self, callback, ah.clone(), |ah, _| {
            ah.trigger();
        });
        ah.triggered().await;
    }

    pub async fn singletons(self: &Rc<Self>) -> Rc<Singletons> {
        if let Some(res) = self.singletons.get() {
            return res;
        }
        #[derive(Default)]
        struct S {
            jay_compositor: Cell<Option<(u32, u32)>>,
            jay_damage_tracking: Cell<Option<u32>>,
        }
        let s = Rc::new(S::default());
        let registry: WlRegistryId = self.id();
        self.send(wl_display::GetRegistry {
            self_id: WL_DISPLAY_ID,
            registry,
        });
        wl_registry::Global::handle(self, registry, s.clone(), |s, g| {
            if g.interface == JayCompositor.name() {
                s.jay_compositor.set(Some((g.name, g.version)));
            } else if g.interface == JayDamageTracking.name() {
                s.jay_damage_tracking.set(Some(g.name));
            }
        });
        self.round_trip().await;
        macro_rules! get {
            ($field:ident, $if:expr) => {
                match s.$field.get() {
                    Some(j) => j,
                    _ => fatal!("Compositor does not provide the {} singleton", $if.name()),
                }
            };
        }
        let res = Rc::new(Singletons {
            registry,
            jay_compositor: get!(jay_compositor, JayCompositor),
            jay_damage_tracking: s.jay_damage_tracking.get(),
        });
        self.singletons.set(Some(res.clone()));
        res
    }

    pub async fn jay_compositor(self: &Rc<Self>) -> JayCompositorId {
        if let Some(id) = self.jay_compositor.get() {
            return id;
        }
        let s = self.singletons().await;
        let id: JayCompositorId = self.id();
        self.send(wl_registry::Bind {
            self_id: s.registry,
            name: s.jay_compositor.0,
            interface: JayCompositor.name(),
            version: s.jay_compositor.1.min(21),
            id: id.into(),
        });
        self.jay_compositor.set(Some(id));
        id
    }

    pub async fn jay_damage_tracking(self: &Rc<Self>) -> Option<JayDamageTrackingId> {
        if let Some(id) = self.jay_damage_tracking.get() {
            return id;
        }
        let s = self.singletons().await;
        let Some(name) = s.jay_damage_tracking else {
            self.jay_damage_tracking.set(Some(None));
            return None;
        };
        let id: JayDamageTrackingId = self.id();
        self.send(wl_registry::Bind {
            self_id: s.registry,
            name,
            interface: JayDamageTracking.name(),
            version: 1,
            id: id.into(),
        });
        self.jay_damage_tracking.set(Some(Some(id)));
        Some(id)
    }

    pub async fn select_workspace(self: &Rc<Self>) -> JayWorkspaceId {
        let id = self.id();
        self.send(jay_compositor::SelectWorkspace {
            self_id: self.jay_compositor().await,
            id,
            seat: WlSeatId::NONE,
        });
        let ae = Rc::new(AsyncEvent::default());
        let ws = Rc::new(Cell::new(JayWorkspaceId::NONE));
        jay_select_workspace::Cancelled::handle(self, id, ae.clone(), |ae, _event| {
            ae.trigger();
        });
        jay_select_workspace::Selected::handle(
            self,
            id,
            (ae.clone(), ws.clone()),
            |(ae, ws), event| {
                ws.set(event.id);
                ae.trigger();
            },
        );
        ae.triggered().await;
        ws.get()
    }

    pub async fn select_toplevel(self: &Rc<Self>) -> JayToplevelId {
        let id = self.id();
        self.send(jay_compositor::SelectToplevel {
            self_id: self.jay_compositor().await,
            id,
            seat: WlSeatId::NONE,
        });
        let ae = Rc::new(AsyncEvent::default());
        let toplevel = Rc::new(Cell::new(JayToplevelId::NONE));
        jay_select_toplevel::Done::handle(
            self,
            id,
            (ae.clone(), toplevel.clone()),
            |(ae, toplevel), event| {
                toplevel.set(event.id);
                ae.trigger();
            },
        );
        ae.triggered().await;
        toplevel.get()
    }

    pub async fn select_toplevel_client(self: &Rc<Self>) -> u64 {
        let id = self.id();
        self.send(jay_compositor::SelectToplevel {
            self_id: self.jay_compositor().await,
            id,
            seat: WlSeatId::NONE,
        });
        let ae = Rc::new(AsyncEvent::default());
        let client_id = Rc::new(Cell::new(0));
        jay_select_toplevel::Done::handle(
            self,
            id,
            (self.clone(), ae.clone(), client_id.clone()),
            |(tc, ae, client_id), event| {
                if event.id.is_some() {
                    jay_toplevel::ClientId::handle(
                        tc,
                        event.id,
                        client_id.clone(),
                        |client_id, event| {
                            client_id.set(event.id);
                        },
                    );
                    jay_toplevel::Done::handle(tc, event.id, ae.clone(), |ae, _event| {
                        ae.trigger();
                    });
                } else {
                    ae.trigger();
                }
            },
        );
        ae.triggered().await;
        client_id.get()
    }
}

pub struct Singletons {
    registry: WlRegistryId,
    pub jay_compositor: (u32, u32),
    pub jay_damage_tracking: Option<u32>,
}

pub const NONE_FUTURE: Option<Pending<()>> = None;

pub trait Handle: RequestParser<'static> {
    fn handle<R, H>(tl: &Rc<ToolClient>, id: impl Into<ObjectId>, r: R, h: H)
    where
        R: 'static,
        H: for<'a> Fn(&R, Self::Generic<'a>) + 'static;

    #[expect(dead_code)]
    fn handle2<R, F, H>(tl: &Rc<ToolClient>, id: impl Into<ObjectId>, r: R, h: H)
    where
        R: 'static,
        F: Future<Output = ()> + 'static,
        H: for<'a> Fn(&R, Self::Generic<'a>) -> F + 'static;
}

impl<T: RequestParser<'static>> Handle for T {
    fn handle<R, H>(tl: &Rc<ToolClient>, id: impl Into<ObjectId>, r: R, h: H)
    where
        R: 'static,
        H: for<'a> Fn(&R, T::Generic<'a>) + 'static,
    {
        tl.handle::<Self, _, _, _>(id.into(), r, move |a, b| {
            h(a, b);
            NONE_FUTURE
        });
    }

    fn handle2<R, F, H>(tl: &Rc<ToolClient>, id: impl Into<ObjectId>, r: R, h: H)
    where
        R: 'static,
        F: Future<Output = ()> + 'static,
        H: for<'a> Fn(&R, T::Generic<'a>) -> F + 'static,
    {
        tl.handle::<Self, _, _, _>(id.into(), r, move |a, b| Some(h(a, b)));
    }
}

struct Outgoing {
    tc: Rc<ToolClient>,
    buf: BufFdOut,
    buffers: VecDeque<OutBuffer>,
}

impl Outgoing {
    async fn run(mut self) {
        loop {
            self.tc.flush_request.triggered().await;
            if let Err(e) = self.flush().await {
                fatal!("Could not process an outgoing message: {}", ErrorFmt(e));
            }
        }
    }

    async fn flush(&mut self) -> Result<(), ToolClientError> {
        {
            let mut swapchain = self.tc.swapchain.borrow_mut();
            swapchain.commit();
            mem::swap(&mut swapchain.pending, &mut self.buffers);
        }
        while let Some(mut cur) = self.buffers.pop_front() {
            if let Err(e) = self.buf.flush_no_timeout(&mut cur).await {
                return Err(ToolClientError::Write(e));
            }
            self.tc.swapchain.borrow_mut().free.push(cur);
        }
        Ok(())
    }
}

struct Incoming {
    tc: Rc<ToolClient>,
    buf: BufFdIn,
}

impl Incoming {
    async fn run(mut self) {
        loop {
            if let Err(e) = self.handle_msg().await {
                fatal!("Could not process an incoming message: {}", ErrorFmt(e));
            }
        }
    }

    async fn handle_msg(&mut self) -> Result<(), ToolClientError> {
        let mut hdr = [0u32, 0];
        if let Err(e) = self.buf.read_full(&mut hdr[..]).await {
            return Err(ToolClientError::Read(e));
        }
        let obj_id = ObjectId::from_raw(hdr[0]);
        let len = (hdr[1] >> 16) as usize;
        let request = hdr[1] & 0xffff;
        if len < 8 {
            return Err(ToolClientError::MsgLenTooSmall);
        }
        if len % 4 != 0 {
            return Err(ToolClientError::UnalignedMessage);
        }
        let len = len / 4 - 2;
        let mut data_buf = self.tc.bufs.pop().unwrap_or_default();
        data_buf.clear();
        data_buf.reserve(len);
        let unused = data_buf.split_at_spare_mut_ext().1;
        if let Err(e) = self.buf.read_full(&mut unused[..len]).await {
            return Err(ToolClientError::Read(e));
        }
        unsafe {
            data_buf.set_len(len);
        }
        let mut handler = None;
        {
            let handlers = self.tc.handlers.borrow_mut();
            if let Some(handlers) = handlers.get(&obj_id) {
                handler = handlers.get(&request).cloned();
            }
        }
        if let Some(handler) = handler {
            let mut parser = MsgParser::new(&mut self.buf, &data_buf);
            handler(&mut parser)?;
        }
        if data_buf.capacity() > 0 {
            self.tc.bufs.push(data_buf);
        }
        Ok(())
    }
}
