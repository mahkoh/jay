use crate::async_engine::{AsyncFd, SpawnedFuture};
use crate::client::error::LookupError;
use crate::client::objects::Objects;
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_display::WlDisplay;
use crate::ifs::wl_registry::{WlRegistry};
use crate::object::{Interface, Object, ObjectId, WL_DISPLAY_ID};
use crate::state::State;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError, OutBufferSwapchain};
use crate::utils::numcell::NumCell;
use crate::utils::oneshot::{oneshot, OneshotTx};
use crate::utils::queue::AsyncQueue;
use crate::ErrorFmt;
use ahash::AHashMap;
pub use error::{ClientError, ObjectError};
use std::cell::{Cell, RefCell, RefMut};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::mem;
use std::rc::Rc;
use uapi::{c, OwnedFd};
use crate::utils::asyncevent::AsyncEvent;
use crate::wire::WlRegistryId;

mod error;
mod objects;
mod tasks;

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ClientId(u64);

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
                        std::io::Error::from(e)
                    );
                    return Ok(());
                }
            }
        };
        let (send, recv) = oneshot();
        let data = Rc::new(Client {
            id,
            state: global.clone(),
            checking_queue_size: Cell::new(false),
            socket: global.eng.fd(&Rc::new(socket))?,
            objects: Objects::new(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            shutdown: Cell::new(Some(send)),
            dispatch_frame_requests: AsyncQueue::new(),
        });
        let display = Rc::new(WlDisplay::new(&data));
        data.objects.display.set(Some(display.clone()));
        data.objects.add_client_object(display).expect("");
        let client = ClientHolder {
            _handler: global.eng.spawn(tasks::client(data.clone(), recv)),
            data,
        };
        log::info!(
            "Client {} connected, pid: {}, uid: {}, fd: {}",
            id,
            pid,
            uid,
            client.data.socket.raw()
        );
        self.clients.borrow_mut().insert(client.data.id, client);
        Ok(())
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
            client.data.shutdown.replace(None).unwrap().send(());
            client.data.flush_request.trigger();
            self.shutdown_clients.borrow_mut().insert(client_id, client);
        }
    }

    pub fn broadcast<B>(&self, mut f: B)
    where
        B: FnMut(&Rc<Client>),
    {
        let clients = self.clients.borrow();
        for client in clients.values() {
            f(&client.data);
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
    }
}

pub trait EventFormatter: Debug {
    fn format(self, fmt: &mut MsgFormatter<'_>);
    fn id(&self) -> ObjectId;
    fn interface(&self) -> Interface;
}

pub trait RequestParser<'a>: Debug + Sized {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError>;
}

pub struct Client {
    pub id: ClientId,
    pub state: Rc<State>,
    checking_queue_size: Cell<bool>,
    socket: AsyncFd,
    pub objects: Objects,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    shutdown: Cell<Option<OneshotTx<()>>>,
    pub dispatch_frame_requests: AsyncQueue<Rc<WlCallback>>,
}

const MAX_PENDING_BUFFERS: usize = 10;

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
            },
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
            },
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

    pub fn protocol_error(&self, obj: &dyn Object, code: u32, message: String) {
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
            if swapchain.pending.len() > MAX_PENDING_BUFFERS {
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

    pub fn lock_registries(&self) -> RefMut<AHashMap<WlRegistryId, Rc<WlRegistry>>> {
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
            _ => {
                return Err(ClientError::LookupError(LookupError {
                    interface: Id::INTERFACE,
                    id: id.into(),
                }))
            }
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
