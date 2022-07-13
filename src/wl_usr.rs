pub mod usr_ifs;
pub mod usr_object;

use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        client::{EventFormatter, RequestParser, MIN_SERVER_ID},
        io_uring::IoUring,
        object::{ObjectId, WL_DISPLAY_ID},
        utils::{
            asyncevent::AsyncEvent,
            bitfield::Bitfield,
            buffd::{
                BufFdError, BufFdIn, BufFdOut, MsgFormatter, MsgParser, MsgParserError, OutBuffer,
                OutBufferSwapchain,
            },
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            oserror::OsError,
            vec_ext::VecExt,
        },
        wheel::Wheel,
        wire::wl_display,
        wl_usr::{
            usr_ifs::{
                usr_wl_callback::UsrWlCallback, usr_wl_display::UsrWlDisplay,
                usr_wl_registry::UsrWlRegistry,
            },
            usr_object::{UsrObject, UsrObjectError},
        },
    },
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        mem,
        rc::Rc,
    },
    thiserror::Error,
    uapi::c,
};

#[derive(Debug, Error)]
pub enum UsrConError {
    #[error("Could not create a socket")]
    CreateSocket(#[source] OsError),
    #[error("The socket path is too long")]
    SocketPathTooLong,
    #[error("Could not connect to the compositor")]
    Connect(#[source] OsError),
    #[error("The message length is smaller than 8 bytes")]
    MsgLenTooSmall,
    #[error("The size of the message is not a multiple of 4")]
    UnalignedMessage,
    #[error(transparent)]
    BufFdError(#[from] BufFdError),
    #[error("Could not read from the compositor")]
    Read(#[source] BufFdError),
    #[error("Could not write to the compositor")]
    Write(#[source] BufFdError),
    #[error(transparent)]
    UsrObjectError(#[from] UsrObjectError),
    #[error("Server sent an event for object {0} that does not exist")]
    MissingObject(ObjectId),
}

pub struct UsrCon {
    pub ring: Rc<IoUring>,
    pub wheel: Rc<Wheel>,
    pub eng: Rc<AsyncEngine>,
    pub server_id: u32,
    obj_ids: RefCell<Bitfield>,
    objects: CopyHashMap<ObjectId, Option<Rc<dyn UsrObject>>>,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    incoming: Cell<Option<SpawnedFuture<()>>>,
    outgoing: Cell<Option<SpawnedFuture<()>>>,
    pub owner: CloneCell<Option<Rc<dyn UsrConOwner>>>,
}

pub trait UsrConOwner {
    fn killed(&self);
}

impl UsrCon {
    pub fn new(
        ring: &Rc<IoUring>,
        wheel: &Rc<Wheel>,
        eng: &Rc<AsyncEngine>,
        path: &str,
        server_id: u32,
    ) -> Result<Rc<Self>, UsrConError> {
        let socket = match uapi::socket(
            c::AF_UNIX,
            c::SOCK_STREAM | c::SOCK_CLOEXEC | c::SOCK_NONBLOCK,
            0,
        ) {
            Ok(s) => Rc::new(s),
            Err(e) => return Err(UsrConError::CreateSocket(e.into())),
        };
        let mut addr: c::sockaddr_un = uapi::pod_zeroed();
        addr.sun_family = c::AF_UNIX as _;
        if path.len() >= addr.sun_path.len() {
            return Err(UsrConError::SocketPathTooLong);
        }
        let sun_path = uapi::as_bytes_mut(&mut addr.sun_path[..]);
        sun_path[..path.len()].copy_from_slice(path.as_bytes());
        sun_path[path.len()] = 0;
        if let Err(e) = uapi::connect(socket.raw(), &addr) {
            return Err(UsrConError::Connect(e.into()));
        }
        let mut obj_ids = Bitfield::default();
        obj_ids.take(0);
        obj_ids.take(1);
        let slf = Rc::new(Self {
            ring: ring.clone(),
            wheel: wheel.clone(),
            eng: eng.clone(),
            server_id,
            obj_ids: RefCell::new(obj_ids),
            objects: Default::default(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            incoming: Default::default(),
            outgoing: Default::default(),
            owner: Default::default(),
        });
        slf.objects.set(
            WL_DISPLAY_ID.into(),
            Some(Rc::new(UsrWlDisplay {
                id: WL_DISPLAY_ID,
                con: slf.clone(),
            })),
        );
        slf.incoming.set(Some(
            slf.eng.spawn(
                Incoming {
                    con: slf.clone(),
                    buf: BufFdIn::new(&socket, &slf.ring),
                    data: vec![],
                }
                .run(),
            ),
        ));
        slf.outgoing.set(Some(
            slf.eng.spawn(
                Outgoing {
                    con: slf.clone(),
                    buf: BufFdOut::new(&socket, &slf.ring),
                    buffers: Default::default(),
                }
                .run(),
            ),
        ));
        Ok(slf)
    }

    pub fn kill(&self) {
        for (_, obj) in self.objects.lock().drain() {
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
        self.objects.set(obj.id(), Some(obj));
    }

    pub fn get_registry(self: &Rc<Self>) -> Rc<UsrWlRegistry> {
        let registry = Rc::new(UsrWlRegistry {
            id: self.id(),
            con: self.clone(),
            owner: Default::default(),
        });
        self.request(wl_display::GetRegistry {
            self_id: WL_DISPLAY_ID,
            registry: registry.id,
        });
        self.objects.set(registry.id.into(), Some(registry.clone()));
        registry
    }

    pub fn sync<F>(self: &Rc<Self>, handler: F)
    where
        F: FnOnce() + 'static,
    {
        let callback = Rc::new(UsrWlCallback::new(self, handler));
        self.request(wl_display::Sync {
            self_id: WL_DISPLAY_ID,
            callback: callback.id,
        });
        self.objects.set(callback.id.into(), Some(callback));
    }

    pub fn parse<'a, R: RequestParser<'a>>(
        &self,
        obj: &impl UsrObject,
        mut parser: MsgParser<'_, 'a>,
    ) -> Result<R, MsgParserError> {
        let res = R::parse(&mut parser)?;
        parser.eof()?;
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
    async fn run(mut self: Self) {
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
    buf: BufFdIn,
    data: Vec<u32>,
}

impl Incoming {
    async fn run(mut self: Self) {
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
        let mut hdr = [0u32, 0];
        if let Err(e) = self.buf.read_full(&mut hdr[..]).await {
            return Err(UsrConError::Read(e));
        }
        let obj_id = ObjectId::from_raw(hdr[0]);
        let len = (hdr[1] >> 16) as usize;
        let event = hdr[1] & 0xffff;
        if len < 8 {
            return Err(UsrConError::MsgLenTooSmall);
        }
        if len % 4 != 0 {
            return Err(UsrConError::UnalignedMessage);
        }
        let len = len / 4 - 2;
        self.data.clear();
        self.data.reserve(len);
        let unused = self.data.split_at_spare_mut_ext().1;
        if let Err(e) = self.buf.read_full(&mut unused[..len]).await {
            return Err(UsrConError::Read(e));
        }
        unsafe {
            self.data.set_len(len);
        }
        if let Some(obj) = self.con.objects.get(&obj_id) {
            if let Some(obj) = obj {
                let parser = MsgParser::new(&mut self.buf, &self.data);
                obj.handle_event(event, parser)?;
            }
        } else if obj_id.raw() < MIN_SERVER_ID {
            return Err(UsrConError::MissingObject(obj_id));
        } else {
            // ignore events for server-created objects that were never added to the state
        }
        Ok(())
    }
}
