use crate::async_engine::{AsyncError, AsyncFd, SpawnedFuture};
use crate::ifs::wl_display::WlDisplay;
use crate::objects::{Object, ObjectError, ObjectId, Objects, WL_DISPLAY_ID};
use crate::state::State;
use crate::utils::buffd::{BufFdError, BufFdIn, BufFdOut, WlFormatter, WlParser, WlParserError};
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::numcell::NumCell;
use crate::utils::oneshot::{oneshot, OneshotRx, OneshotTx};
use crate::utils::queue::AsyncQueue;
use crate::utils::vec_ext::VecExt;
use anyhow::anyhow;
use futures::{select, FutureExt};
use std::cell::Cell;
use std::fmt::{Debug, Display, Formatter};
use std::mem;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::ifs::wl_region::WlRegion;

#[derive(Debug, Error)]
pub enum WlClientError {
    #[error("An error occurred in the async engine")]
    Async(#[from] AsyncError),
    #[error("An error occurred reading from/writing to the client")]
    Io(#[from] BufFdError),
    #[error("An error occurred while processing a request")]
    RequestError(#[source] Box<ObjectError>),
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
    #[error(transparent)]
    ObjectError(Box<ObjectError>),
    #[error("There is no region with id {0}")]
    RegionDoesNotExist(ObjectId),
}

efrom!(WlClientError, ObjectError, ObjectError);

impl WlClientError {
    fn peer_closed(&self) -> bool {
        match self {
            WlClientError::Io(BufFdError::Closed) => true,
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

pub struct WlClients {
    next_client_id: NumCell<u64>,
    clients: CopyHashMap<ClientId, Rc<WlClient>>,
    shutdown_clients: CopyHashMap<ClientId, Rc<WlClient>>,
}

impl WlClients {
    pub fn new() -> Self {
        Self {
            next_client_id: NumCell::new(1),
            clients: CopyHashMap::new(),
            shutdown_clients: CopyHashMap::new(),
        }
    }

    pub fn id(&self) -> ClientId {
        ClientId(self.next_client_id.fetch_add(1))
    }

    pub fn get(&self, id: ClientId) -> Result<Rc<WlClientData>, WlClientError> {
        match self.clients.get(&id) {
            Some(c) => Ok(c.data.clone()),
            _ => Err(WlClientError::ClientDoesNotExist(id)),
        }
    }

    pub fn spawn(
        &self,
        id: ClientId,
        global: &Rc<State>,
        socket: OwnedFd,
    ) -> Result<(), WlClientError> {
        let (send, recv) = oneshot();
        let data = Rc::new(WlClientData {
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
        let client = Rc::new(WlClient {
            _handler: global.eng.spawn(client(data.clone(), recv)),
            data,
        });
        self.clients.set(client.data.id, client.clone());
        log::info!("Client {} connected", id);
        Ok(())
    }

    pub fn kill(&self, client: ClientId) {
        log::info!("Removing client {}", client.0);
        if self.clients.remove(&client).is_none() {
            self.shutdown_clients.remove(&client);
        }
    }

    pub fn shutdown(&self, client_id: ClientId) {
        if let Some(client) = self.clients.remove(&client_id) {
            log::info!("Shutting down client {}", client.data.id.0);
            client.data.shutdown.replace(None).unwrap().send(());
            client.data.events.push(WlEvent::Shutdown);
            client.data.shutdown_sent.set(true);
            self.shutdown_clients.set(client_id, client);
        }
    }

    pub fn broadcast<B>(&self, mut f: B)
    where
        B: FnMut(&Rc<WlClientData>),
    {
        let clients = self.clients.lock();
        for client in clients.values() {
            f(&client.data);
        }
    }
}

struct WlClient {
    data: Rc<WlClientData>,
    _handler: SpawnedFuture<()>,
}

impl Drop for WlClient {
    fn drop(&mut self) {
        self.data.objects.destroy();
    }
}

pub trait EventFormatter: Debug {
    fn format(self: Box<Self>, fmt: &mut WlFormatter<'_>);
    fn obj(&self) -> &dyn Object;
}

pub type DynEventFormatter = Box<dyn EventFormatter>;

pub trait RequestParser<'a>: Debug + Sized {
    fn parse(parser: &mut WlParser<'_, 'a>) -> Result<Self, WlParserError>;
}

enum WlEvent {
    Flush,
    Shutdown,
    Event(Box<dyn EventFormatter>),
}

pub struct WlClientData {
    pub id: ClientId,
    pub state: Rc<State>,
    socket: AsyncFd,
    pub objects: Objects,
    events: AsyncQueue<WlEvent>,
    shutdown: Cell<Option<OneshotTx<()>>>,
    shutdown_sent: Cell<bool>,
}

const MAX_PENDING_EVENTS: usize = 100;

impl WlClientData {
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

    pub fn display(&self) -> Result<Rc<WlDisplay>, WlClientError> {
        Ok(self.objects.get_obj(WL_DISPLAY_ID)?.into_display()?)
    }

    pub fn parse<'a, R: RequestParser<'a>>(
        &self,
        obj: &impl Object,
        mut parser: WlParser<'_, 'a>,
    ) -> Result<R, WlParserError> {
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

    pub async fn event(&self, event: Box<dyn EventFormatter>) -> Result<(), WlClientError> {
        self.event2(WlEvent::Event(event)).await
    }

    async fn event2(&self, event: WlEvent) -> Result<(), WlClientError> {
        self.events.push(event);
        self.check_queue_size().await
    }

    pub async fn check_queue_size(&self) -> Result<(), WlClientError> {
        if self.events.size() > MAX_PENDING_EVENTS {
            self.state.eng.yield_now().await;
            if self.events.size() > MAX_PENDING_EVENTS {
                log::error!("Client {} is too slow at fetching events", self.id.0);
                self.state.clients.kill(self.id);
                return Err(WlClientError::OutBufferOverflow);
            }
        }
        Ok(())
    }

    pub fn attach_client_object(&self, obj: Rc<dyn Object>) -> Result<(), WlClientError> {
        self.objects.add_client_object(obj.clone())?;
        obj.post_attach();
        Ok(())
    }

    pub fn get_region(&self, id: ObjectId) -> Result<Rc<WlRegion>, WlClientError> {
        match self.objects.regions.get(&id) {
            Some(r) => Ok(r),
            _ => Err(WlClientError::RegionDoesNotExist(id)),
        }
    }
}

async fn client(data: Rc<WlClientData>, shutdown: OneshotRx<()>) {
    let mut recv = data.state.eng.spawn(receive(data.clone())).fuse();
    let _send = data.state.eng.spawn(send(data.clone()));
    select! {
        _ = recv => { },
        _ = shutdown.fuse() => { },
    }
    drop(recv);
    if !data.shutdown_sent.get() {
        data.events.push(WlEvent::Shutdown);
    }
    match data.state.eng.timeout(5000) {
        Ok(timeout) => {
            timeout.await;
            log::error!("Could not shut down client {} within 5 seconds", data.id.0);
        }
        Err(e) => {
            log::error!("Could not create a timeout: {:#}", e);
        }
    }
    data.state.clients.kill(data.id);
}

async fn receive(data: Rc<WlClientData>) {
    let display = data.display().unwrap();
    let recv = async {
        let mut buf = BufFdIn::new(data.socket.clone());
        let mut data_buf = Vec::<u32>::new();
        loop {
            let mut hdr = [0u32, 0];
            buf.read_full(&mut hdr[..]).await?;
            let obj_id = ObjectId::from_raw(hdr[0]);
            let len = (hdr[1] >> 16) as usize;
            let request = hdr[1] & 0xffff;
            let obj = match data.objects.get_obj(obj_id) {
                Ok(obj) => obj,
                _ => {
                    data.fatal_event(display.invalid_object(obj_id));
                    return Err(WlClientError::InvalidObject(obj_id));
                }
            };
            // log::trace!("obj: {}, request: {}, len: {}", obj_id, request, len);
            if request >= obj.num_requests() {
                data.invalid_request(&*obj, request);
                return Err(WlClientError::InvalidMethod);
            }
            if len < 8 {
                return Err(WlClientError::MessageSizeTooSmall);
            }
            if len % 4 != 0 {
                return Err(WlClientError::UnalignedMessage);
            }
            let len = len / 4 - 2;
            data_buf.clear();
            data_buf.reserve(len);
            let unused = data_buf.split_at_spare_mut_ext().1;
            buf.read_full(&mut unused[..len]).await?;
            unsafe {
                data_buf.set_len(len);
            }
            // log::trace!("{:x?}", data_buf);
            let parser = WlParser::new(&mut buf, &data_buf[..]);
            if let Err(e) = obj.handle_request(request, parser).await {
                return Err(WlClientError::RequestError(Box::new(e)));
            }
            data.event2(WlEvent::Flush).await?;
        }
    };
    let res: Result<(), WlClientError> = recv.await;
    if let Err(e) = res {
        if e.peer_closed() {
            log::info!("Client {} terminated the connection", data.id.0);
            data.state.clients.kill(data.id);
        } else {
            let e = anyhow!(e);
            log::error!(
                "An error occurred while trying to handle a message from client {}: {:#}",
                data.id.0,
                e
            );
            if !data.shutdown_sent.get() {
                data.fatal_event(display.implementation_error(format!("{:#}", e)));
            }
        }
    }
}

async fn send(data: Rc<WlClientData>) {
    let send = async {
        let mut buf = BufFdOut::new(data.socket.clone());
        let mut flush_requested = false;
        loop {
            let mut event = data.events.pop().await;
            loop {
                match event {
                    WlEvent::Flush => {
                        flush_requested = true;
                    }
                    WlEvent::Shutdown => {
                        buf.flush().await?;
                        return Ok(());
                    }
                    WlEvent::Event(e) => {
                        if log::log_enabled!(log::Level::Trace) {
                            let obj = e.obj();
                            log::trace!(
                                "Client {} <= {}@{}.{:?}",
                                data.id,
                                obj.interface().name(),
                                obj.id(),
                                e
                            );
                        }
                        e.format(&mut WlFormatter::new(&mut buf));
                        if buf.needs_flush() {
                            buf.flush().await?;
                            flush_requested = false;
                        }
                    }
                }
                event = match data.events.try_pop() {
                    Some(e) => e,
                    _ => break,
                };
            }
            if mem::take(&mut flush_requested) {
                buf.flush().await?;
            }
        }
    };
    let res: Result<(), WlClientError> = send.await;
    if let Err(e) = res {
        if e.peer_closed() {
            log::info!("Client {} terminated the connection", data.id.0);
        } else {
            log::error!(
                "An error occurred while sending data to client {}: {:#}",
                data.id.0,
                e
            );
        }
    }
    data.state.clients.kill(data.id);
}
