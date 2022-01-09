use crate::async_engine::{AsyncError, AsyncFd, SpawnedFuture};
use crate::client::objects::Objects;
use crate::ifs::wl_buffer::{WlBuffer, WlBufferError, WlBufferId};
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_compositor::{WlCompositorError, WlCompositorObj};
use crate::ifs::wl_data_device::{WlDataDevice, WlDataDeviceError};
use crate::ifs::wl_data_device_manager::{WlDataDeviceManagerError, WlDataDeviceManagerObj};
use crate::ifs::wl_data_offer::{WlDataOffer, WlDataOfferError};
use crate::ifs::wl_data_source::{WlDataSource, WlDataSourceError};
use crate::ifs::wl_display::{WlDisplay, WlDisplayError};
use crate::ifs::wl_output::{WlOutputError, WlOutputObj};
use crate::ifs::wl_region::{WlRegion, WlRegionError, WlRegionId};
use crate::ifs::wl_registry::{WlRegistry, WlRegistryError, WlRegistryId};
use crate::ifs::wl_seat::wl_keyboard::{WlKeyboard, WlKeyboardError};
use crate::ifs::wl_seat::wl_pointer::{WlPointer, WlPointerError};
use crate::ifs::wl_seat::wl_touch::{WlTouch, WlTouchError};
use crate::ifs::wl_seat::{WlSeatError, WlSeatId, WlSeatObj};
use crate::ifs::wl_shm::{WlShmError, WlShmObj};
use crate::ifs::wl_shm_pool::{WlShmPool, WlShmPoolError};
use crate::ifs::wl_subcompositor::{WlSubcompositorError, WlSubcompositorObj};
use crate::ifs::wl_surface::wl_subsurface::{WlSubsurface, WlSubsurfaceError};
use crate::ifs::wl_surface::xdg_surface::xdg_popup::{XdgPopup, XdgPopupError};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{XdgToplevel, XdgToplevelError};
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError, XdgSurfaceId};
use crate::ifs::wl_surface::{WlSurface, WlSurfaceError, WlSurfaceId};
use crate::ifs::xdg_positioner::{XdgPositioner, XdgPositionerError};
use crate::ifs::xdg_wm_base::{XdgWmBaseError, XdgWmBaseObj};
use crate::object::{Object, ObjectId, WL_DISPLAY_ID};
use crate::state::State;
use crate::utils::buffd::{BufFdError, MsgFormatter, MsgParser, MsgParserError};
use crate::utils::numcell::NumCell;
use crate::utils::oneshot::{oneshot, OneshotTx};
use crate::utils::queue::AsyncQueue;
use crate::ErrorFmt;
use ahash::AHashMap;
use std::cell::{Cell, RefCell, RefMut};
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::mem;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, OwnedFd};

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
    #[error("There is no wl_region with id {0}")]
    RegionDoesNotExist(WlRegionId),
    #[error("There is no wl_buffer with id {0}")]
    BufferDoesNotExist(WlBufferId),
    #[error("There is no wl_surface with id {0}")]
    SurfaceDoesNotExist(WlSurfaceId),
    #[error("There is no xdg_surface with id {0}")]
    XdgSurfaceDoesNotExist(XdgSurfaceId),
    #[error("There is no wl_seat with id {0}")]
    WlSeatDoesNotExist(WlSeatId),
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
    #[error("An error occurred in a `xdg_surface`")]
    XdgSurfaceError(#[source] Box<XdgSurfaceError>),
    #[error("An error occurred in a `xdg_positioner`")]
    XdgPositionerError(#[source] Box<XdgPositionerError>),
    #[error("An error occurred in a `xdg_popup`")]
    XdgPopupError(#[source] Box<XdgPopupError>),
    #[error("An error occurred in a `xdg_toplevel`")]
    XdgToplevelError(#[source] Box<XdgToplevelError>),
    #[error("An error occurred in a `xdg_wm_base`")]
    XdgWmBaseError(#[source] Box<XdgWmBaseError>),
    #[error("An error occurred in a `wl_buffer`")]
    WlBufferError(#[source] Box<WlBufferError>),
    #[error("An error occurred in a `wl_output`")]
    WlOutputError(#[source] Box<WlOutputError>),
    #[error("An error occurred in a `wl_seat`")]
    WlSeatError(#[source] Box<WlSeatError>),
    #[error("An error occurred in a `wl_pointer`")]
    WlPointerError(#[source] Box<WlPointerError>),
    #[error("An error occurred in a `wl_keyboard`")]
    WlKeyboardError(#[source] Box<WlKeyboardError>),
    #[error("An error occurred in a `wl_touch`")]
    WlTouchError(#[source] Box<WlTouchError>),
    #[error("Object {0} is not a display")]
    NotADisplay(ObjectId),
    #[error("An error occurred in a `wl_data_device`")]
    WlDataDeviceError(#[source] Box<WlDataDeviceError>),
    #[error("An error occurred in a `wl_data_device_manager`")]
    WlDataDeviceManagerError(#[source] Box<WlDataDeviceManagerError>),
    #[error("An error occurred in a `wl_data_offer`")]
    WlDataOfferError(#[source] Box<WlDataOfferError>),
    #[error("An error occurred in a `wl_data_source`")]
    WlDataSourceError(#[source] Box<WlDataSourceError>),
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
efrom!(ClientError, XdgSurfaceError, XdgSurfaceError);
efrom!(ClientError, XdgPositionerError, XdgPositionerError);
efrom!(ClientError, XdgWmBaseError, XdgWmBaseError);
efrom!(ClientError, XdgToplevelError, XdgToplevelError);
efrom!(ClientError, XdgPopupError, XdgPopupError);
efrom!(ClientError, WlBufferError, WlBufferError);
efrom!(ClientError, WlOutputError, WlOutputError);
efrom!(ClientError, WlSeatError, WlSeatError);
efrom!(ClientError, WlTouchError, WlTouchError);
efrom!(ClientError, WlPointerError, WlPointerError);
efrom!(ClientError, WlKeyboardError, WlKeyboardError);
efrom!(
    ClientError,
    WlDataDeviceManagerError,
    WlDataDeviceManagerError
);
efrom!(ClientError, WlDataDeviceError, WlDataDeviceError);
efrom!(ClientError, WlDataSourceError, WlDataSourceError);
efrom!(ClientError, WlDataOfferError, WlDataOfferError);

impl ClientError {
    fn peer_closed(&self) -> bool {
        matches!(self, ClientError::Io(BufFdError::Closed))
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
            socket: global.eng.fd(&Rc::new(socket))?,
            objects: Objects::new(),
            events: AsyncQueue::new(),
            shutdown: Cell::new(Some(send)),
            shutdown_sent: Cell::new(false),
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

pub struct ClientHolder {
    pub data: Rc<Client>,
    _handler: SpawnedFuture<()>,
}

impl Drop for ClientHolder {
    fn drop(&mut self) {
        self.data.objects.destroy();
        self.data.events.clear();
        self.data.dispatch_frame_requests.clear();
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

pub enum WlEvent {
    Flush,
    Shutdown,
    Event(Box<dyn EventFormatter>),
}

pub struct Client {
    pub id: ClientId,
    pub state: Rc<State>,
    socket: AsyncFd,
    pub objects: Objects,
    events: AsyncQueue<WlEvent>,
    shutdown: Cell<Option<OneshotTx<()>>>,
    shutdown_sent: Cell<bool>,
    pub dispatch_frame_requests: AsyncQueue<Rc<WlCallback>>,
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
                    "Could not retrieve display of client {}: {}",
                    self.id,
                    ErrorFmt(e),
                );
                self.state.clients.kill(self.id);
            }
        }
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

    pub fn protocol_error(&self, obj: &dyn Object, code: u32, message: String) {
        if let Ok(d) = self.display() {
            self.fatal_event(d.error(obj.id(), code, message));
        } else {
            self.state.clients.shutdown(self.id);
        }
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

    pub async fn flush(&self) -> Result<(), ClientError> {
        self.event2(WlEvent::Flush).await
    }

    async fn event2(&self, event: WlEvent) -> Result<(), ClientError> {
        self.events.push(event);
        self.check_queue_size().await
    }

    pub fn event2_locked(&self, event: WlEvent) -> bool {
        self.events.push(event);
        self.events.size() > MAX_PENDING_EVENTS
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

    pub fn get_buffer(&self, id: WlBufferId) -> Result<Rc<WlBuffer>, ClientError> {
        match self.objects.buffers.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::BufferDoesNotExist(id)),
        }
    }

    pub fn get_region(&self, id: WlRegionId) -> Result<Rc<WlRegion>, ClientError> {
        match self.objects.regions.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::RegionDoesNotExist(id)),
        }
    }

    pub fn get_surface(&self, id: WlSurfaceId) -> Result<Rc<WlSurface>, ClientError> {
        match self.objects.surfaces.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::SurfaceDoesNotExist(id)),
        }
    }

    pub fn get_xdg_surface(&self, id: XdgSurfaceId) -> Result<Rc<XdgSurface>, ClientError> {
        match self.objects.xdg_surfaces.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::XdgSurfaceDoesNotExist(id)),
        }
    }

    pub fn get_wl_seat(&self, id: WlSeatId) -> Result<Rc<WlSeatObj>, ClientError> {
        match self.objects.seats.get(&id) {
            Some(r) => Ok(r),
            _ => Err(ClientError::WlSeatDoesNotExist(id)),
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

    pub fn lock_registries(&self) -> RefMut<AHashMap<WlRegistryId, Rc<WlRegistry>>> {
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
simple_add_obj!(XdgPositioner);
simple_add_obj!(XdgToplevel);
simple_add_obj!(XdgPopup);
simple_add_obj!(WlOutputObj);
simple_add_obj!(WlKeyboard);
simple_add_obj!(WlPointer);
simple_add_obj!(WlTouch);
simple_add_obj!(WlDataDeviceManagerObj);
simple_add_obj!(WlDataDevice);
simple_add_obj!(WlDataOffer);
simple_add_obj!(WlDataSource);

macro_rules! dedicated_add_obj {
    ($ty:ty, $field:ident) => {
        impl AddObj<$ty> for Client {
            type RemoveObj<'a> = impl Future<Output = Result<(), ClientError>> + 'a;

            fn add_obj(&self, obj: &Rc<$ty>, client: bool) -> Result<(), ClientError> {
                self.simple_add_obj(obj, client)?;
                self.objects.$field.set(obj.id().into(), obj.clone());
                Ok(())
            }
            fn remove_obj<'a>(&'a self, obj: &'a $ty) -> Self::RemoveObj<'a> {
                self.objects.$field.remove(&obj.id().into());
                self.simple_remove_obj(obj.id())
            }
        }
    };
}

dedicated_add_obj!(WlRegion, regions);
dedicated_add_obj!(WlSurface, surfaces);
dedicated_add_obj!(XdgWmBaseObj, xdg_wm_bases);
dedicated_add_obj!(XdgSurface, xdg_surfaces);
dedicated_add_obj!(WlBuffer, buffers);
dedicated_add_obj!(WlSeatObj, seats);
