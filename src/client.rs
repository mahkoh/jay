use {
    crate::{
        async_engine::SpawnedFuture,
        client::{error::LookupError, objects::Objects},
        criteria::{
            CritDestroyListener, CritMatcherId,
            clm::{CL_CHANGED_DESTROYED, CL_CHANGED_NEW, ClMatcherChange},
        },
        ifs::{
            wl_display::WlDisplay,
            wl_registry::WlRegistry,
            wl_surface::{WlSurface, commit_timeline::CommitTimelines},
        },
        leaks::Tracker,
        object::{Interface, Object, ObjectId, WL_DISPLAY_ID},
        security_context_acceptor::AcceptorMetadata,
        state::State,
        utils::{
            activation_token::ActivationToken,
            asyncevent::AsyncEvent,
            buffd::{MsgFormatter, MsgParser, MsgParserError, OutBufferSwapchain},
            copyhashmap::{CopyHashMap, Locked},
            errorfmt::ErrorFmt,
            numcell::NumCell,
            pending_serial::PendingSerial,
            pid_info::{PidInfo, get_pid_info, get_socket_creds},
            pidfd_send_signal::pidfd_send_signal,
        },
        wire::WlRegistryId,
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        error::Error,
        fmt::{Debug, Display, Formatter},
        mem,
        ops::DerefMut,
        rc::{Rc, Weak},
    },
    uapi::{OwnedFd, c},
};
pub use {
    error::{ClientError, ParserError},
    objects::MIN_SERVER_ID,
};

mod error;
mod objects;
mod tasks;

bitflags! {
    ClientCaps: u32;
        CAP_DATA_CONTROL_MANAGER     = 1 << 0,
        CAP_VIRTUAL_KEYBOARD_MANAGER = 1 << 1,
        CAP_FOREIGN_TOPLEVEL_LIST    = 1 << 2,
        CAP_IDLE_NOTIFIER            = 1 << 3,
        CAP_SESSION_LOCK_MANAGER     = 1 << 4,
        CAP_JAY_COMPOSITOR           = 1 << 5,
        CAP_LAYER_SHELL              = 1 << 6,
        CAP_SCREENCOPY_MANAGER       = 1 << 7,
        CAP_SEAT_MANAGER             = 1 << 8,
        CAP_DRM_LEASE                = 1 << 9,
        CAP_INPUT_METHOD             = 1 << 10,
        CAP_WORKSPACE                = 1 << 11,
        CAP_FOREIGN_TOPLEVEL_MANAGER = 1 << 12,
        CAP_HEAD_MANAGER             = 1 << 13,
}

pub const CAPS_DEFAULT: ClientCaps = ClientCaps(CAP_LAYER_SHELL.0 | CAP_DRM_LEASE.0);
pub const CAPS_DEFAULT_SANDBOXED: ClientCaps = ClientCaps(CAP_DRM_LEASE.0);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ClientId(u64);

impl ClientId {
    pub fn raw(self) -> u64 {
        self.0
    }

    pub fn from_raw(val: u64) -> Self {
        Self(val)
    }
}

impl Display for ClientId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub struct Clients {
    next_client_id: NumCell<u64>,
    pub clients: RefCell<AHashMap<ClientId, ClientHolder>>,
    shutdown_clients: RefCell<AHashMap<ClientId, ClientHolder>>,
}

impl Clients {
    pub fn new() -> Self {
        Self {
            next_client_id: NumCell::new(1),
            clients: Default::default(),
            shutdown_clients: Default::default(),
        }
    }

    pub fn clear(&self) {
        mem::take(self.clients.borrow_mut().deref_mut());
        mem::take(self.shutdown_clients.borrow_mut().deref_mut());
    }

    pub fn id(&self) -> ClientId {
        ClientId(self.next_client_id.fetch_add(1))
    }

    pub fn get(&self, id: ClientId) -> Result<Rc<Client>, ClientError> {
        let clients = self.clients.borrow();
        match clients.get(&id) {
            Some(c) => Ok(c.data.clone()),
            _ => Err(ClientError::ClientDoesNotExist(id)),
        }
    }

    pub fn spawn(
        &self,
        id: ClientId,
        global: &Rc<State>,
        socket: Rc<OwnedFd>,
        effective_caps: ClientCaps,
        bounding_caps: ClientCaps,
        acceptor: &Rc<AcceptorMetadata>,
    ) -> Result<(), ClientError> {
        let Some((uid, pid)) = get_socket_creds(&socket) else {
            return Ok(());
        };
        self.spawn2(
            id,
            global,
            socket,
            uid,
            pid,
            effective_caps,
            bounding_caps,
            false,
            acceptor,
        )?;
        Ok(())
    }

    pub fn spawn2(
        &self,
        id: ClientId,
        global: &Rc<State>,
        socket: Rc<OwnedFd>,
        uid: c::uid_t,
        pid: c::pid_t,
        effective_caps: ClientCaps,
        bounding_caps: ClientCaps,
        is_xwayland: bool,
        acceptor: &Rc<AcceptorMetadata>,
    ) -> Result<Rc<Client>, ClientError> {
        let data = Rc::new_cyclic(|slf| Client {
            id,
            state: global.clone(),
            checking_queue_size: Cell::new(false),
            socket,
            objects: Objects::new(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            shutdown: Default::default(),
            tracker: Default::default(),
            is_xwayland,
            effective_caps,
            bounding_caps,
            last_enter_serial: Default::default(),
            pid_info: get_pid_info(uid, pid),
            serials: Default::default(),
            symmetric_delete: Cell::new(false),
            last_xwayland_serial: Cell::new(0),
            surfaces_by_xwayland_serial: Default::default(),
            activation_tokens: Default::default(),
            commit_timelines: Rc::new(CommitTimelines::new(
                &global.wait_for_sync_obj,
                &global.ring,
                &global.eng,
                slf,
            )),
            wire_scale: Default::default(),
            focus_stealing_serial: Default::default(),
            changed_properties: Default::default(),
            destroyed: Default::default(),
            acceptor: acceptor.clone(),
            v2: Default::default(),
        });
        track!(data, data);
        let display = Rc::new(WlDisplay::new(&data));
        track!(data, display);
        data.objects.display.set(Some(display.clone()));
        data.objects.add_client_object(display).expect("");
        let client = ClientHolder {
            _handler: global.eng.spawn("client", tasks::client(data.clone())),
            data: data.clone(),
        };
        log::info!(
            "Client {} connected, pid: {}, uid: {}, fd: {}, comm: {:?}, caps: {:?}",
            id,
            pid,
            uid,
            client.data.socket.raw(),
            data.pid_info.comm,
            effective_caps,
        );
        client.data.property_changed(CL_CHANGED_NEW);
        self.clients.borrow_mut().insert(client.data.id, client);
        Ok(data)
    }

    pub fn kill(&self, client: ClientId) {
        log::info!("Removing client {}", client.0);
        if self.clients.borrow_mut().remove(&client).is_none() {
            self.shutdown_clients.borrow_mut().remove(&client);
        }
    }

    pub fn shutdown(&self, client_id: ClientId) {
        if let Some(client) = self.clients.borrow_mut().remove(&client_id) {
            log::info!("Shutting down client {}", client.data.id.0);
            client.data.shutdown.trigger();
            client.data.flush_request.trigger();
            self.shutdown_clients.borrow_mut().insert(client_id, client);
        }
    }

    pub fn broadcast<B>(&self, required_caps: ClientCaps, xwayland_only: bool, mut f: B)
    where
        B: FnMut(&Rc<Client>),
    {
        let clients = self.clients.borrow();
        for client in clients.values() {
            if client.data.effective_caps.contains(required_caps)
                && (!xwayland_only || client.data.is_xwayland)
            {
                f(&client.data);
            }
        }
    }
}

impl Drop for Clients {
    fn drop(&mut self) {
        let _clients1 = mem::take(&mut *self.clients.borrow_mut());
        let _clients2 = mem::take(&mut *self.shutdown_clients.borrow_mut());
    }
}

pub struct ClientHolder {
    pub data: Rc<Client>,
    _handler: SpawnedFuture<()>,
}

impl Drop for ClientHolder {
    fn drop(&mut self) {
        self.data.objects.destroy();
        self.data.flush_request.clear();
        self.data.shutdown.clear();
        self.data.surfaces_by_xwayland_serial.clear();
        self.data.remove_activation_tokens();
        self.data.commit_timelines.clear();
        self.data.property_changed(CL_CHANGED_DESTROYED);
        if self.data.is_xwayland
            && let Some(pidfd) = self.data.state.xwayland.pidfd.get()
            && let Err(e) = pidfd_send_signal(&pidfd, c::SIGKILL)
        {
            log::error!("Could not kill Xwayland: {}", ErrorFmt(e));
        }
    }
}

pub trait EventFormatter: Debug {
    fn format(self, fmt: &mut MsgFormatter<'_>);
    fn id(&self) -> ObjectId;
    fn interface(&self) -> Interface;
}

pub trait RequestParser<'a>: Debug + Sized {
    type Generic<'b>: RequestParser<'b>;
    const ID: u32;
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError>;
}

pub struct Client {
    pub id: ClientId,
    pub state: Rc<State>,
    checking_queue_size: Cell<bool>,
    socket: Rc<OwnedFd>,
    pub objects: Objects,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    shutdown: AsyncEvent,
    pub tracker: Tracker<Client>,
    pub is_xwayland: bool,
    pub effective_caps: ClientCaps,
    pub bounding_caps: ClientCaps,
    pub last_enter_serial: Cell<Option<u64>>,
    pub pid_info: PidInfo,
    pub serials: RefCell<VecDeque<SerialRange>>,
    pub symmetric_delete: Cell<bool>,
    pub last_xwayland_serial: Cell<u64>,
    pub surfaces_by_xwayland_serial: CopyHashMap<u64, Rc<WlSurface>>,
    pub activation_tokens: RefCell<VecDeque<ActivationToken>>,
    pub commit_timelines: Rc<CommitTimelines>,
    pub wire_scale: Cell<Option<i32>>,
    pub focus_stealing_serial: Cell<Option<u64>>,
    pub changed_properties: Cell<ClMatcherChange>,
    pub destroyed: CopyHashMap<CritMatcherId, Weak<dyn CritDestroyListener<Rc<Self>>>>,
    pub acceptor: Rc<AcceptorMetadata>,
    pub v2: Cell<bool>,
}

pub const NUM_CACHED_SERIAL_RANGES: usize = 64;

pub struct SerialRange {
    pub lo: u64,
    pub hi: u64,
}

impl Client {
    pub fn invalid_request(&self, obj: &dyn Object, request: u32) {
        log::error!(
            "Client {} sent an invalid request {} on object {} of type {}",
            self.id.0,
            request,
            obj.id(),
            obj.interface().name(),
        );
        match self.display() {
            Ok(d) => {
                d.send_invalid_request(obj, request);
                self.state.clients.shutdown(self.id);
            }
            Err(e) => {
                log::error!(
                    "Could not retrieve display of client {}: {}",
                    self.id,
                    ErrorFmt(e),
                );
                self.state.clients.kill(self.id);
            }
        }
    }

    pub fn map_serial(&self, serial: u32) -> Option<u64> {
        let serials = self.serials.borrow_mut();
        let latest = serials.back()?;
        let mut serial = ((latest.hi >> 32) << 32) | (serial as u64);
        if serial > latest.hi {
            serial = serial.checked_sub(1 << 32)?;
        }
        for range in serials.iter().rev() {
            if serial > range.hi {
                return None;
            }
            if serial >= range.lo {
                return Some(serial);
            }
        }
        if serials.len() == NUM_CACHED_SERIAL_RANGES {
            return Some(serial);
        }
        None
    }

    pub fn next_serial(&self) -> u64 {
        self.state.next_serial(Some(self))
    }

    pub fn pending_serial(&self) -> PendingSerial<'_> {
        PendingSerial::new(self)
    }

    pub fn new_id<T: From<ObjectId>>(&self) -> Result<T, ClientError> {
        self.objects.id(self)
    }

    pub fn display(&self) -> Result<Rc<WlDisplay>, ClientError> {
        match self.objects.display.get() {
            Some(d) => Ok(d),
            _ => Err(ClientError::NotADisplay(WL_DISPLAY_ID)),
        }
    }

    pub fn parse<'a, R: RequestParser<'a>>(
        &self,
        obj: &impl Object,
        mut parser: MsgParser<'_, 'a>,
    ) -> Result<R, MsgParserError> {
        let res = R::parse(&mut parser)?;
        parser.eof()?;
        log::trace!(
            "Client {} -> {}@{}.{:?}",
            self.id,
            obj.interface().name(),
            obj.id(),
            res
        );
        Ok(res)
    }

    pub fn error(&self, message: impl Error) {
        let msg = ErrorFmt(message).to_string();
        log::error!("Client {}: A fatal error occurred: {}", self.id.0, msg,);
        match self.display() {
            Ok(d) => {
                d.send_implementation_error(msg);
                self.state.clients.shutdown(self.id);
            }
            Err(e) => {
                log::error!(
                    "Could not retrieve display of client {}: {}",
                    self.id,
                    ErrorFmt(e),
                );
                self.state.clients.kill(self.id);
            }
        }
    }

    pub fn protocol_error(&self, obj: &dyn Object, code: u32, message: &str) {
        if let Ok(d) = self.display() {
            d.send_error(obj.id(), code, message);
        }
        self.state.clients.shutdown(self.id);
    }

    pub fn event<T: EventFormatter>(self: &Rc<Self>, event: T) {
        if log::log_enabled!(log::Level::Trace) {
            self.log_event(&event);
        }
        let mut fds = vec![];
        let mut swapchain = self.swapchain.borrow_mut();
        let mut fmt = MsgFormatter::new(&mut swapchain.cur, &mut fds, self.v2.get());
        event.format(&mut fmt);
        fmt.write_len();
        if swapchain.cur.is_full() {
            swapchain.commit();
            if swapchain.exceeds_limit() {
                if !self.checking_queue_size.replace(true) {
                    self.state.slow_clients.push(self.clone());
                }
            }
        }
        self.flush_request.trigger();
    }

    // pub fn flush(&self) {
    //     self.flush_request.trigger();
    // }

    pub async fn check_queue_size(&self) {
        if self.swapchain.borrow_mut().exceeds_limit() {
            self.state.eng.yield_now().await;
            if self.swapchain.borrow_mut().exceeds_limit() {
                log::error!("Client {} is too slow at fetching events", self.id.0);
                self.state.clients.kill(self.id);
                return;
            }
        }
        self.checking_queue_size.set(false);
    }

    pub fn lock_registries(&self) -> Locked<'_, WlRegistryId, Rc<WlRegistry>> {
        self.objects.registries()
    }

    pub fn log_event<T: EventFormatter>(&self, event: &T) {
        log::trace!(
            "Client {} <= {}@{}.{:?}",
            self.id,
            event.interface().name(),
            event.id(),
            event,
        );
    }

    pub fn add_client_obj<T: WaylandObject>(&self, obj: &Rc<T>) -> Result<(), ClientError> {
        self.add_obj(obj, true)
    }

    pub fn add_server_obj<T: WaylandObject>(&self, obj: &Rc<T>) {
        self.add_obj(obj, false).expect("add_server_obj failed")
    }

    fn add_obj<T: WaylandObject>(&self, obj: &Rc<T>, client: bool) -> Result<(), ClientError> {
        if client {
            self.objects.add_client_object(obj.clone())?;
        } else {
            self.objects.add_server_object(obj.clone());
        }
        obj.clone().add(self);
        Ok(())
    }

    pub fn remove_obj<T: WaylandObject>(self: &Rc<Self>, obj: &T) -> Result<(), ClientError> {
        obj.remove(self);
        self.objects.remove_obj(self, obj.id())
    }

    pub fn lookup<Id: WaylandObjectLookup>(&self, id: Id) -> Result<Rc<Id::Object>, ClientError> {
        match Id::lookup(self, id) {
            Some(t) => Ok(t),
            _ => Err(ClientError::LookupError(LookupError {
                interface: Id::INTERFACE,
                id: id.into(),
            })),
        }
    }

    fn remove_activation_tokens(&self) {
        for token in &*self.activation_tokens.borrow() {
            self.state.activation_tokens.remove(token);
        }
    }

    pub fn property_changed(self: &Rc<Self>, change: ClMatcherChange) {
        let props = self.changed_properties.get();
        self.changed_properties.set(props | change);
        if props.is_none() && change.is_some() {
            self.state.cl_matcher_manager.changed(self);
        }
    }
}

pub trait WaylandObject: Object {
    fn add(self: Rc<Self>, client: &Client) {
        let _ = client;
    }
    fn remove(&self, client: &Client) {
        let _ = client;
    }
}

pub trait WaylandObjectLookup: Copy + Into<ObjectId> {
    type Object;
    const INTERFACE: Interface;

    fn lookup(client: &Client, id: Self) -> Option<Rc<Self::Object>>;
}
