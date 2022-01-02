use crate::async_engine::{AsyncError, AsyncFd, SpawnedFuture};
use crate::client::objects::Objects;
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_compositor::{WlCompositorError, WlCompositorObj};
use crate::ifs::wl_display::{WlDisplay, WlDisplayError};
use crate::ifs::wl_region::{WlRegion, WlRegionError};
use crate::ifs::wl_registry::{WlRegistry, WlRegistryError};
use crate::ifs::wl_shm::{WlShmError, WlShmObj};
use crate::ifs::wl_shm_pool::{WlShmPool, WlShmPoolError};
use crate::ifs::wl_subcompositor::{WlSubcompositorError, WlSubcompositorObj};
use crate::ifs::wl_surface::wl_subsurface::{WlSubsurface, WlSubsurfaceError};
use crate::ifs::wl_surface::{WlSurface, WlSurfaceError};
use crate::ifs::xdg_wm_base::XdgWmBaseObj;
use crate::object::{Object, ObjectId, WL_DISPLAY_ID};
use crate::state::State;
use crate::utils::buffd::{BufFdError, MsgFormatter, MsgParser, MsgParserError};
use crate::utils::numcell::NumCell;
use crate::utils::oneshot::{oneshot, OneshotTx};
use crate::utils::queue::AsyncQueue;
use ahash::AHashMap;
use anyhow::anyhow;
use std::cell::{Cell, RefCell, RefMut};
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::mem;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

mod objects;
mod tasks;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("An error occurred in the async engine")]
    Async(#[from] AsyncError),
    #[error("An error occurred reading from/writing to the client")]
    Io(#[from] BufFdError),
    #[error("An error occurred while processing a request")]
    RequestError(#[source] Box<ClientError>),
    #[error("Client tried to invoke a non-existent method")]
    InvalidMethod,
    #[error("Client tried to access non-existent object {0}")]
    InvalidObject(ObjectId),
    #[error("The message size is < 8")]
    MessageSizeTooSmall,
    #[error("The size of the message is not a multiple of 4")]
    UnalignedMessage,
    #[error("The outgoing buffer overflowed")]
    OutBufferOverflow,
    #[error("The requested client {0} does not exist")]
    ClientDoesNotExist(ClientId),
    #[error("There is no region with id {0}")]
    RegionDoesNotExist(ObjectId),
    #[error("There is no surface with id {0}")]
    SurfaceDoesNotExist(ObjectId),
    #[error("Cannot parse the message")]
    ParserError(#[source] Box<MsgParserError>),
    #[error("Server tried to allocate more than 0x1_00_00_00 ids")]
    TooManyIds,
    #[error("The server object id is out of bounds")]
    ServerIdOutOfBounds,
    #[error("The object id is unknown")]
    UnknownId,
    #[error("The id is already in use")]
    IdAlreadyInUse,
    #[error("The client object id is out of bounds")]
    ClientIdOutOfBounds,
    #[error("An error occurred in a `wl_display`")]
    WlDisplayError(#[source] Box<WlDisplayError>),
    #[error("An error occurred in a `wl_registry`")]
    WlRegistryError(#[source] Box<WlRegistryError>),
    #[error("Could not add object {0} to the client")]
    AddObjectError(ObjectId, #[source] Box<ClientError>),
    #[error("An error occurred in a `wl_surface`")]
    WlSurfaceError(#[source] Box<WlSurfaceError>),
    #[error("An error occurred in a `wl_compositor`")]
    WlCompositorError(#[source] Box<WlCompositorError>),
    #[error("An error occurred in a `wl_shm`")]
    WlShmError(#[source] Box<WlShmError>),
    #[error("An error occurred in a `wl_shm_pool`")]
    WlShmPoolError(#[source] Box<WlShmPoolError>),
    #[error("An error occurred in a `wl_region`")]
    WlRegionError(#[source] Box<WlRegionError>),
    #[error("An error occurred in a `wl_subsurface`")]
    WlSubsurfaceError(#[source] Box<WlSubsurfaceError>),
    #[error("An error occurred in a `wl_subcompositor`")]
    WlSubcompositorError(#[source] Box<WlSubcompositorError>),
    #[error("Object {0} is not a display")]
    NotADisplay(ObjectId),
}

efrom!(ClientError, ParserError, MsgParserError);
efrom!(ClientError, WlDisplayError, WlDisplayError);
efrom!(ClientError, WlRegistryError, WlRegistryError);
efrom!(ClientError, WlSurfaceError, WlSurfaceError);
efrom!(ClientError, WlCompositorError, WlCompositorError);
efrom!(ClientError, WlShmError, WlShmError);
efrom!(ClientError, WlShmPoolError, WlShmPoolError);
efrom!(ClientError, WlRegionError, WlRegionError);
efrom!(ClientError, WlSubsurfaceError, WlSubsurfaceError);
efrom!(ClientError, WlSubcompositorError, WlSubcompositorError);

impl ClientError {
    fn peer_closed(&self) -> bool {
        match self {
            ClientError::Io(BufFdError::Closed) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ClientId(u64);

impl Display for ClientId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub struct Clients {
    next_client_id: NumCell<u64>,
    clients: RefCell<AHashMap<ClientId, ClientHolder>>,
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
        let (send, recv) = oneshot();
        let data = Rc::new(Client {
            id,
            state: global.clone(),
            socket: global.eng.fd(&Rc::new(socket))?,
            objects: Objects::new(),
            events: AsyncQueue::new(),
            shutdown: Cell::new(Some(send)),
            shutdown_sent: Cell::new(false),
        });
        data.objects
            .add_client_object(Rc::new(WlDisplay::new(&data)))
            .expect("");
        let client = ClientHolder {
            _handler: global.eng.spawn(tasks::client(data.clone(), recv)),
            data,
        };
        self.clients.borrow_mut().insert(client.data.id, client);
        log::info!("Client {} connected", id);
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
            client.data.events.push(WlEvent::Shutdown);
            client.data.shutdown_sent.set(true);
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

struct ClientHolder {
    data: Rc<Client>,
    _handler: SpawnedFuture<()>,
}

impl Drop for ClientHolder {
    fn drop(&mut self) {
        self.data.objects.destroy();
    }
}

pub trait EventFormatter: Debug {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>);
    fn obj(&self) -> &dyn Object;
}

pub type DynEventFormatter = Box<dyn EventFormatter>;

pub trait RequestParser<'a>: Debug + Sized {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError>;
}

enum WlEvent {
    Flush,
    Shutdown,
    Event(Box<dyn EventFormatter>),
}

pub struct Client {
    pub id: ClientId,
    pub state: Rc<State>,
    socket: AsyncFd,
    objects: Objects,
    events: AsyncQueue<WlEvent>,
    shutdown: Cell<Option<OneshotTx<()>>>,
    shutdown_sent: Cell<bool>,
}

const MAX_PENDING_EVENTS: usize = 100;

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
            Ok(d) => self.fatal_event(d.invalid_request(obj, request)),
            Err(e) => {
                log::error!(
                    "Could not retrieve display of client {}: {:#}",
                    self.id,
                    anyhow!(e)
                );
                self.state.clients.kill(self.id);
            }
        }
    }

    pub fn display(&self) -> Result<Rc<WlDisplay>, ClientError> {
        Ok(self.objects.get_obj(WL_DISPLAY_ID)?.into_display()?)
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

    pub fn fatal_event(&self, event: Box<dyn EventFormatter>) {
        self.events.push(WlEvent::Event(event));
        self.state.clients.shutdown(self.id);
    }

    pub fn event_locked(&self, event: Box<dyn EventFormatter>) -> bool {
        self.events.push(WlEvent::Event(event));
        self.events.size() > MAX_PENDING_EVENTS
    }

    pub async fn event(&self, event: Box<dyn EventFormatter>) -> Result<(), ClientError> {
        self.event2(WlEvent::Event(event)).await
    }

    async fn event2(&self, event: WlEvent) -> Result<(), ClientError> {
        self.events.push(event);
        self.check_queue_size().await
    }

    pub async fn check_queue_size(&self) -> Result<(), ClientError> {
        if self.events.size() > MAX_PENDING_EVENTS {
            self.state.eng.yield_now().await;
            if self.events.size() > MAX_PENDING_EVENTS {
                log::error!("Client {} is too slow at fetching events", self.id.0);
                self.state.clients.kill(self.id);
                return Err(ClientError::OutBufferOverflow);
            }
        }
        Ok(())
    }

    pub fn get_region(&self, id: ObjectId) -> Result<Rc<WlRegion>, ClientError> {
        match self.objects.regions.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::RegionDoesNotExist(id)),
        }
    }

    pub fn get_surface(&self, id: ObjectId) -> Result<Rc<WlSurface>, ClientError> {
        match self.objects.surfaces.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::SurfaceDoesNotExist(id)),
        }
    }

    fn simple_add_obj<T: Object>(&self, obj: &Rc<T>, client: bool) -> Result<(), ClientError> {
        if client {
            self.objects.add_client_object(obj.clone())
        } else {
            self.objects.add_server_object(obj.clone());
            Ok(())
        }
    }

    fn simple_remove_obj<'a>(
        &'a self,
        id: ObjectId,
    ) -> impl Future<Output = Result<(), ClientError>> + 'a {
        self.objects.remove_obj(self, id)
    }

    pub fn lock_registries(&self) -> RefMut<AHashMap<ObjectId, Rc<WlRegistry>>> {
        self.objects.registries()
    }

    pub fn log_event(&self, event: &dyn EventFormatter) {
        let obj = event.obj();
        log::trace!(
            "Client {} <= {}@{}.{:?}",
            self.id,
            obj.interface().name(),
            obj.id(),
            event,
        );
    }
}

pub trait AddObj<T> {
    type RemoveObj<'a>: Future<Output = Result<(), ClientError>> + 'a;

    fn add_client_obj(&self, obj: &Rc<T>) -> Result<(), ClientError> {
        self.add_obj(obj, true)
    }

    fn add_server_obj(&self, obj: &Rc<T>) {
        self.add_obj(obj, false).expect("add_server_obj failed")
    }

    fn add_obj(&self, obj: &Rc<T>, client: bool) -> Result<(), ClientError>;

    fn remove_obj<'a>(&'a self, obj: &'a T) -> Self::RemoveObj<'a>;
}

macro_rules! simple_add_obj {
    ($ty:ty) => {
        impl AddObj<$ty> for Client {
            type RemoveObj<'a> = impl Future<Output = Result<(), ClientError>> + 'a;

            fn add_obj(&self, obj: &Rc<$ty>, client: bool) -> Result<(), ClientError> {
                self.simple_add_obj(obj, client)
            }
            fn remove_obj<'a>(&'a self, obj: &'a $ty) -> Self::RemoveObj<'a> {
                self.simple_remove_obj(obj.id())
            }
        }
    };
}

simple_add_obj!(WlCompositorObj);
simple_add_obj!(WlCallback);
simple_add_obj!(WlRegistry);
simple_add_obj!(WlShmObj);
simple_add_obj!(WlShmPool);
simple_add_obj!(WlSubcompositorObj);
simple_add_obj!(WlSubsurface);
simple_add_obj!(XdgWmBaseObj);

macro_rules! dedicated_add_obj {
    ($ty:ty, $field:ident) => {
        impl AddObj<$ty> for Client {
            type RemoveObj<'a> = impl Future<Output = Result<(), ClientError>> + 'a;

            fn add_obj(&self, obj: &Rc<$ty>, client: bool) -> Result<(), ClientError> {
                self.simple_add_obj(obj, client)?;
                self.objects.$field.set(obj.id(), obj.clone());
                Ok(())
            }
            fn remove_obj<'a>(&'a self, obj: &'a $ty) -> Self::RemoveObj<'a> {
                self.objects.$field.remove(&obj.id());
                self.simple_remove_obj(obj.id())
            }
        }
    };
}

dedicated_add_obj!(WlRegion, regions);
dedicated_add_obj!(WlSurface, surfaces);
