use bstr::ByteSlice;
pub use error::{ClientError, MethodError, ObjectError};
use {
    crate::{
        async_engine::{AsyncFd, SpawnedFuture},
        client::{error::LookupError, objects::Objects},
        ifs::{wl_callback::WlCallback, wl_display::WlDisplay, wl_registry::WlRegistry},
        leaks::Tracker,
        object::{Interface, Object, ObjectId, WL_DISPLAY_ID},
        state::State,
        utils::{
            asyncevent::AsyncEvent,
            buffd::{MsgFormatter, MsgParser, MsgParserError, OutBufferSwapchain},
            copyhashmap::Locked,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            queue::AsyncQueue,
        },
        wire::WlRegistryId,
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        error::Error,
        fmt::{Debug, Display, Formatter},
        mem,
        ops::DerefMut,
        rc::Rc,
    },
    uapi::{c, OwnedFd},
};
use crate::utils::trim::AsciiTrim;

mod error;
mod objects;
mod tasks;

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ClientId(u64);

impl ClientId {
    pub fn raw(self) -> u64 {
        self.0
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

    #[allow(dead_code)]
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
        socket: OwnedFd,
        secure: bool,
    ) -> Result<(), ClientError> {
        let (uid, pid) = {
            let mut cred = c::ucred {
                pid: 0,
                uid: 0,
                gid: 0,
            };
            match uapi::getsockopt(socket.raw(), c::SOL_SOCKET, c::SO_PEERCRED, &mut cred) {
                Ok(_) => (cred.uid, cred.pid),
                Err(e) => {
                    log::error!(
                        "Cannot determine peer credentials of new connection: {:?}",
                        crate::utils::oserror::OsError::from(e)
                    );
                    return Ok(());
                }
            }
        };
        self.spawn2(id, global, socket, uid, pid, secure, false)?;
        Ok(())
    }

    pub fn spawn2(
        &self,
        id: ClientId,
        global: &Rc<State>,
        socket: OwnedFd,
        uid: c::uid_t,
        pid: c::pid_t,
        secure: bool,
        is_xwayland: bool,
    ) -> Result<Rc<Client>, ClientError> {
        let data = Rc::new(Client {
            id,
            state: global.clone(),
            checking_queue_size: Cell::new(false),
            socket: global.eng.fd(&Rc::new(socket))?,
            objects: Objects::new(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            shutdown: Default::default(),
            dispatch_frame_requests: AsyncQueue::new(),
            tracker: Default::default(),
            is_xwayland,
            secure,
            last_serial: Cell::new(0),
            last_enter_serial: Cell::new(0),
            pid_info: get_pid_info(uid, pid),
        });
        track!(data, data);
        let display = Rc::new(WlDisplay::new(&data));
        track!(data, display);
        data.objects.display.set(Some(display.clone()));
        data.objects.add_client_object(display).expect("");
        let client = ClientHolder {
            _handler: global.eng.spawn(tasks::client(data.clone())),
            data: data.clone(),
        };
        log::info!(
            "Client {} connected, pid: {}, uid: {}, fd: {}, secure: {}, comm: {:?}",
            id,
            pid,
            uid,
            client.data.socket.raw(),
            secure,
            data.pid_info.comm,
        );
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

    pub fn broadcast<B>(&self, secure: bool, mut f: B)
    where
        B: FnMut(&Rc<Client>),
    {
        let clients = self.clients.borrow();
        for client in clients.values() {
            if !secure || client.data.secure {
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
        self.data.dispatch_frame_requests.clear();
        self.data.flush_request.clear();
        self.data.shutdown.clear();
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

pub struct PidInfo {
    pub uid: c::uid_t,
    pub pid: c::pid_t,
    pub comm: String,
}

pub struct Client {
    pub id: ClientId,
    pub state: Rc<State>,
    checking_queue_size: Cell<bool>,
    socket: AsyncFd,
    pub objects: Objects,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    shutdown: AsyncEvent,
    pub dispatch_frame_requests: AsyncQueue<Rc<WlCallback>>,
    pub tracker: Tracker<Client>,
    pub is_xwayland: bool,
    pub secure: bool,
    pub last_serial: Cell<u32>,
    pub last_enter_serial: Cell<u32>,
    pub pid_info: PidInfo,
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

    pub fn validate_serial(&self, serial: u32) -> Result<(), ClientError> {
        if serial > self.last_serial.get() {
            Err(ClientError::InvalidSerial)
        } else {
            Ok(())
        }
    }

    pub fn next_serial(&self) -> u32 {
        self.state.next_serial(Some(self))
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
        let mut fmt = MsgFormatter::new(&mut swapchain.cur, &mut fds);
        event.format(&mut fmt);
        fmt.write_len();
        if swapchain.cur.is_full() {
            swapchain.commit();
            if swapchain.exceeds_limit() {
                if !self.checking_queue_size.replace(true) {
                    self.state.slow_clients.push(self.clone());
                }
            }
            self.flush_request.trigger();
        }
    }

    pub fn flush(&self) {
        self.flush_request.trigger();
    }

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

    pub fn lock_registries(&self) -> Locked<WlRegistryId, Rc<WlRegistry>> {
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

    #[allow(dead_code)]
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

fn get_pid_info(uid: c::uid_t, pid: c::pid_t) -> PidInfo {
    let comm = match std::fs::read(format!("/proc/{}/comm", pid)) {
        Ok(name) => name.trim().as_bstr().to_string(),
        Err(e) => {
            log::warn!("Could not read `comm` of pid {}: {}", pid, ErrorFmt(e));
            "Unknown".to_string()
        }
    };
    PidInfo {
        uid,
        pid,
        comm,
    }
}
