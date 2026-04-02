pub mod usr_ifs;
pub mod usr_object;

use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        client::{EventFormatter, MIN_SERVER_ID, RequestParser},
        io_uring::{IoUring, IoUringError},
        object::{Interface, ObjectId, Version, WL_DISPLAY_ID},
        utils::{
            asyncevent::AsyncEvent,
            bitfield::Bitfield,
            buffd::{
                BufFdError, BufFdOut, MsgFormatter, MsgParser, MsgParserError, OutBuffer,
                OutBufferSwapchain, WlBufFdIn, WlMessage,
            },
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            hash_map_ext::HashMapExt,
            oserror::{OsError, OsErrorExt2},
        },
        video::dmabuf::DmaBufIds,
        wheel::Wheel,
        wire::wl_display,
        wl_usr::{
            usr_ifs::{
                usr_wl_callback::UsrWlCallback, usr_wl_display::UsrWlDisplay,
                usr_wl_registry::UsrWlRegistry,
            },
            usr_object::UsrObject,
        },
    },
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        error::Error,
        mem,
        rc::Rc,
    },
    thiserror::Error,
    uapi::{OwnedFd, c},
};

#[derive(Debug, Error)]
pub enum UsrConError {
    #[error("Could not create a socket")]
    CreateSocket(#[source] OsError),
    #[error("The socket path is too long")]
    SocketPathTooLong,
    #[error("Could not connect to the compositor")]
    Connect(#[source] IoUringError),
    #[error(transparent)]
    BufFdError(#[from] BufFdError),
    #[error("Could not read from the compositor")]
    Read(#[source] BufFdError),
    #[error("Could not write to the compositor")]
    Write(#[source] BufFdError),
    #[error("Server sent an event for object {0} that does not exist")]
    MissingObject(ObjectId),
    #[error("Could not process a `{}#{}.{}` event", .interface.name(), .id, .method)]
    MethodError {
        interface: Interface,
        id: ObjectId,
        method: &'static str,
        #[source]
        error: Box<dyn Error + 'static>,
    },
    #[error("Client tried to invoke a non-existent method")]
    InvalidMethod,
}

pub struct UsrCon {
    pub ring: Rc<IoUring>,
    pub _wheel: Rc<Wheel>,
    pub eng: Rc<AsyncEngine>,
    pub server_id: u32,
    obj_ids: RefCell<Bitfield>,
    objects: CopyHashMap<ObjectId, Option<Rc<dyn UsrObject>>>,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    outgoing: Cell<Option<SpawnedFuture<()>>>,
    pub owner: CloneCell<Option<Rc<dyn UsrConOwner>>>,
    dead: Cell<bool>,
    dma_buf_ids: Rc<DmaBufIds>,
}

pub trait UsrConOwner {
    fn killed(&self);
}

impl UsrCon {
    pub async fn new(
        ring: &Rc<IoUring>,
        wheel: &Rc<Wheel>,
        eng: &Rc<AsyncEngine>,
        dma_buf_ids: &Rc<DmaBufIds>,
        path: &str,
        server_id: u32,
    ) -> Result<Rc<Self>, UsrConError> {
        let socket = uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0)
            .map(Rc::new)
            .map_os_err(UsrConError::CreateSocket)?;
        let mut addr: c::sockaddr_un = uapi::pod_zeroed();
        addr.sun_family = c::AF_UNIX as _;
        if path.len() >= addr.sun_path.len() {
            return Err(UsrConError::SocketPathTooLong);
        }
        let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
        sun_path[..path.len()].copy_from_slice(path.as_bytes());
        sun_path[path.len()] = 0;
        if let Err(e) = ring.connect(&socket, &addr).await {
            return Err(UsrConError::Connect(e));
        }
        Ok(Self::from_socket(
            ring,
            wheel,
            eng,
            dma_buf_ids,
            &socket,
            server_id,
        ))
    }

    pub fn from_socket(
        ring: &Rc<IoUring>,
        wheel: &Rc<Wheel>,
        eng: &Rc<AsyncEngine>,
        dma_buf_ids: &Rc<DmaBufIds>,
        socket: &Rc<OwnedFd>,
        server_id: u32,
    ) -> Rc<Self> {
        let mut obj_ids = Bitfield::default();
        obj_ids.take(0);
        obj_ids.take(1);
        let slf = Rc::new(Self {
            ring: ring.clone(),
            _wheel: wheel.clone(),
            eng: eng.clone(),
            server_id,
            obj_ids: RefCell::new(obj_ids),
            objects: Default::default(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            incoming: Default::default(),
            outgoing: Default::default(),
            owner: Default::default(),
            dead: Cell::new(false),
            dma_buf_ids: dma_buf_ids.clone(),
        });
        slf.objects.set(
            WL_DISPLAY_ID.into(),
            Some(Rc::new(UsrWlDisplay {
                id: WL_DISPLAY_ID,
                con: slf.clone(),
                version: Version(1),
            })),
        );
        slf.incoming.set(Some(
            slf.eng.spawn(
                "wl_usr incoming",
                Incoming {
                    con: slf.clone(),
                    buf: WlBufFdIn::new(socket, &slf.ring),
                }
                .run(),
            ),
        ));
        slf.outgoing.set(Some(
            slf.eng.spawn(
                "wl_usr outgoing",
                Outgoing {
                    con: slf.clone(),
                    buf: BufFdOut::new(socket, &slf.ring),
                    buffers: Default::default(),
                }
                .run(),
            ),
        ));
        slf
    }

    pub fn kill(&self) {
        self.dead.set(true);
        for obj in self.objects.lock().drain_values() {
            if let Some(obj) = obj {
                obj.break_loops();
            }
        }
        self.incoming.take();
        self.outgoing.take();
        if let Some(owner) = self.owner.take() {
            owner.killed();
        }
    }

    pub fn release_id(&self, id: u32) {
        self.obj_ids.borrow_mut().release(id);
        self.objects.remove(&ObjectId::from_raw(id));
    }

    pub fn remove_obj(&self, obj: &impl UsrObject) {
        obj.destroy();
        obj.break_loops();
        if obj.id().raw() >= MIN_SERVER_ID {
            self.objects.remove(&obj.id());
        } else {
            self.objects.set(obj.id(), None);
        }
    }

    pub fn add_object(&self, obj: Rc<dyn UsrObject>) {
        if !self.dead.get() {
            self.objects.set(obj.id(), Some(obj));
        }
    }

    pub fn get_registry(self: &Rc<Self>) -> Rc<UsrWlRegistry> {
        let registry = Rc::new(UsrWlRegistry {
            id: self.id(),
            con: self.clone(),
            owner: Default::default(),
            version: Version(1),
        });
        self.request(wl_display::GetRegistry {
            self_id: WL_DISPLAY_ID,
            registry: registry.id,
        });
        self.add_object(registry.clone());
        registry
    }

    pub fn sync<F>(self: &Rc<Self>, handler: F)
    where
        F: FnOnce() + 'static,
    {
        let callback = Rc::new(UsrWlCallback::new(self));
        callback.owner.set(Some(Rc::new(Cell::new(Some(handler)))));
        self.request(wl_display::Sync {
            self_id: WL_DISPLAY_ID,
            callback: callback.id,
        });
        self.add_object(callback);
    }

    pub fn parse<'a, R: RequestParser<'a>>(
        &self,
        obj: &impl UsrObject,
        mut parser: MsgParser<'_, 'a>,
    ) -> Result<R, MsgParserError> {
        let res = R::parse(&mut parser)?;
        log::trace!(
            "Server {} -> {}@{}.{:?}",
            self.server_id,
            obj.interface().name(),
            obj.id(),
            res
        );
        Ok(res)
    }

    pub fn request<T: EventFormatter>(self: &Rc<Self>, event: T) {
        if self.dead.get() {
            return;
        }
        if log::log_enabled!(log::Level::Trace) {
            log::trace!(
                "Server {} <= {}@{}.{:?}",
                self.server_id,
                event.interface().name(),
                event.id(),
                event,
            );
        }
        let mut fds = vec![];
        let mut swapchain = self.swapchain.borrow_mut();
        let mut fmt = MsgFormatter::new(&mut swapchain.cur, &mut fds);
        event.format(&mut fmt);
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
}

struct Outgoing {
    con: Rc<UsrCon>,
    buf: BufFdOut,
    buffers: VecDeque<OutBuffer>,
}

impl Outgoing {
    async fn run(mut self) {
        loop {
            self.con.flush_request.triggered().await;
            if let Err(e) = self.flush().await {
                log::error!(
                    "Server {}: Could not process an outgoing message: {}",
                    self.con.server_id,
                    ErrorFmt(e)
                );
                self.con.kill();
                return;
            }
        }
    }

    async fn flush(&mut self) -> Result<(), UsrConError> {
        {
            let mut swapchain = self.con.swapchain.borrow_mut();
            swapchain.commit();
            mem::swap(&mut swapchain.pending, &mut self.buffers);
        }
        while let Some(mut cur) = self.buffers.pop_front() {
            if let Err(e) = self.buf.flush_no_timeout(&mut cur).await {
                return Err(UsrConError::Write(e));
            }
            self.con.swapchain.borrow_mut().free.push(cur);
        }
        Ok(())
    }
}

struct Incoming {
    con: Rc<UsrCon>,
    buf: WlBufFdIn,
}

impl Incoming {
    async fn run(mut self) {
        loop {
            if let Err(e) = self.handle_msg().await {
                log::error!(
                    "Server {}: Could not process an incoming message: {}",
                    self.con.server_id,
                    ErrorFmt(e)
                );
                self.con.kill();
                return;
            }
        }
    }

    async fn handle_msg(&mut self) -> Result<(), UsrConError> {
        let WlMessage {
            obj_id,
            message,
            body,
            fds,
        } = self.buf.read_message().await.map_err(UsrConError::Read)?;
        if let Some(obj) = self.con.objects.get(&obj_id) {
            if let Some(obj) = obj {
                let parser = MsgParser::new(fds, body);
                obj.handle_event(&self.con, message, parser)?;
            }
        } else if obj_id.raw() < MIN_SERVER_ID {
            return Err(UsrConError::MissingObject(obj_id));
        } else {
            // ignore events for server-created objects that were never added to the state
        }
        Ok(())
    }
}
