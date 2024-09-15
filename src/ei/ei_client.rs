pub use crate::ei::ei_client::ei_error::{EiClientError, EiParserError};
use {
    crate::{
        async_engine::SpawnedFuture,
        client::ClientId,
        ei::{
            ei_client::ei_objects::EiObjects,
            ei_ifs::{ei_connection::EiConnection, ei_handshake::EiHandshake},
            ei_object::{EiInterface, EiObject, EiObjectId},
            EiContext, EiInterfaceVersion,
        },
        ifs::wl_seat::WlSeatGlobal,
        leaks::Tracker,
        state::State,
        utils::{
            asyncevent::AsyncEvent,
            buffd::{EiMsgFormatter, EiMsgParser, EiMsgParserError, OutBufferSwapchain},
            clonecell::CloneCell,
            debug_fn::debug_fn,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            pid_info::{get_pid_info, get_socket_creds, PidInfo},
        },
        wire_ei::EiInterfaceVersions,
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        error::Error,
        fmt::Debug,
        mem,
        ops::DerefMut,
        rc::Rc,
    },
    uapi::OwnedFd,
};

mod ei_error;
mod ei_objects;
mod ei_tasks;

pub struct EiClients {
    pub clients: RefCell<AHashMap<ClientId, EiClientHolder>>,
    shutdown_clients: RefCell<AHashMap<ClientId, EiClientHolder>>,
}

impl EiClients {
    pub fn new() -> Self {
        Self {
            clients: Default::default(),
            shutdown_clients: Default::default(),
        }
    }

    pub fn announce_seat(&self, seat: &Rc<WlSeatGlobal>) {
        for ei_client in self.clients.borrow().values() {
            if let Some(connection) = ei_client.data.connection.get() {
                connection.announce_seat(&seat);
            }
        }
    }

    pub fn clear(&self) {
        mem::take(self.clients.borrow_mut().deref_mut());
        mem::take(self.shutdown_clients.borrow_mut().deref_mut());
    }

    pub fn spawn(&self, global: &Rc<State>, socket: Rc<OwnedFd>) -> Result<(), EiClientError> {
        let Some((uid, pid)) = get_socket_creds(&socket) else {
            return Ok(());
        };
        let pid_info = get_pid_info(uid, pid);
        self.spawn2(global, socket, Some(pid_info), None)?;
        Ok(())
    }

    pub fn spawn2(
        &self,
        global: &Rc<State>,
        socket: Rc<OwnedFd>,
        pid_info: Option<PidInfo>,
        app_id: Option<String>,
    ) -> Result<Rc<EiClient>, EiClientError> {
        let versions = EiInterfaceVersions {
            ei_button: EiInterfaceVersion::new(1),
            ei_callback: EiInterfaceVersion::new(1),
            ei_connection: EiInterfaceVersion::new(1),
            ei_device: EiInterfaceVersion::new(2),
            ei_handshake: EiInterfaceVersion::new(1),
            ei_keyboard: EiInterfaceVersion::new(1),
            ei_pingpong: EiInterfaceVersion::new(1),
            ei_pointer: EiInterfaceVersion::new(1),
            ei_pointer_absolute: EiInterfaceVersion::new(1),
            ei_scroll: EiInterfaceVersion::new(1),
            ei_seat: EiInterfaceVersion::new(1),
            ei_touchscreen: EiInterfaceVersion::new(1),
        };
        let data = Rc::new(EiClient {
            id: global.clients.id(),
            state: global.clone(),
            context: Cell::new(EiContext::Receiver),
            connection: Default::default(),
            checking_queue_size: Cell::new(false),
            socket,
            objects: EiObjects::new(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            shutdown: Default::default(),
            tracker: Default::default(),
            pid_info,
            disconnect_announced: Cell::new(false),
            versions,
            name: Default::default(),
            app_id,
            last_serial: Default::default(),
        });
        track!(data, data);
        let handshake = Rc::new(EiHandshake::new(&data));
        track!(data, handshake);
        handshake.send_handshake_version();
        data.objects.add_handshake(&handshake);
        let client = EiClientHolder {
            _handler: global
                .eng
                .spawn("ei client", ei_tasks::ei_client(data.clone())),
            data: data.clone(),
        };
        log::info!(
            "Client {} connected{:?}",
            data.id,
            debug_fn(|fmt| {
                if let Some(p) = &data.pid_info {
                    write!(fmt, ", pid: {}, uid: {}, comm: {:?}", p.pid, p.uid, p.comm)?;
                }
                if let Some(app_id) = &data.app_id {
                    write!(fmt, ", app-id: {app_id:?}")?;
                }
                Ok(())
            }),
        );
        self.clients.borrow_mut().insert(client.data.id, client);
        Ok(data)
    }

    pub fn kill(&self, client: ClientId) {
        log::info!("Removing client {}", client);
        if self.clients.borrow_mut().remove(&client).is_none() {
            self.shutdown_clients.borrow_mut().remove(&client);
        }
    }

    pub fn shutdown(&self, client_id: ClientId) {
        if let Some(client) = self.clients.borrow_mut().remove(&client_id) {
            log::info!("Shutting down client {}", client.data.id);
            client.data.shutdown.trigger();
            client.data.flush_request.trigger();
            self.shutdown_clients.borrow_mut().insert(client_id, client);
        }
    }
}

impl Drop for EiClients {
    fn drop(&mut self) {
        let _clients1 = mem::take(&mut *self.clients.borrow_mut());
        let _clients2 = mem::take(&mut *self.shutdown_clients.borrow_mut());
    }
}

pub struct EiClientHolder {
    pub data: Rc<EiClient>,
    _handler: SpawnedFuture<()>,
}

impl Drop for EiClientHolder {
    fn drop(&mut self) {
        self.data.objects.destroy();
        self.data.flush_request.clear();
        self.data.shutdown.clear();
        self.data.connection.take();
    }
}

pub trait EiEventFormatter: Debug {
    fn format(self, fmt: &mut EiMsgFormatter<'_>);
    fn id(&self) -> EiObjectId;
    fn interface(&self) -> EiInterface;
}

pub trait EiRequestParser<'a>: Debug + Sized {
    type Generic<'b>: EiRequestParser<'b>;
    fn parse(parser: &mut EiMsgParser<'_, 'a>) -> Result<Self, EiMsgParserError>;
}

pub struct EiClient {
    pub id: ClientId,
    pub state: Rc<State>,
    pub context: Cell<EiContext>,
    pub connection: CloneCell<Option<Rc<EiConnection>>>,
    checking_queue_size: Cell<bool>,
    socket: Rc<OwnedFd>,
    pub objects: EiObjects,
    swapchain: Rc<RefCell<OutBufferSwapchain>>,
    flush_request: AsyncEvent,
    shutdown: AsyncEvent,
    pub tracker: Tracker<EiClient>,
    pub pid_info: Option<PidInfo>,
    pub disconnect_announced: Cell<bool>,
    pub versions: EiInterfaceVersions,
    pub name: RefCell<Option<String>>,
    pub app_id: Option<String>,
    pub last_serial: NumCell<u64>,
}

impl EiClient {
    pub fn new_id<T: From<EiObjectId>>(&self) -> T {
        self.objects.id()
    }

    pub fn serial(&self) -> u32 {
        (self.last_serial.fetch_add(1) + 1) as u32
    }

    pub fn last_serial(&self) -> u32 {
        self.last_serial.get() as u32
    }

    pub fn error(&self, message: impl Error) {
        let msg = ErrorFmt(message).to_string();
        log::error!("Client {}: A fatal error occurred: {}", self.id, msg);
        match self.connection.get() {
            Some(d) => {
                d.send_disconnected(Some(&msg));
                self.state.clients.shutdown(self.id);
            }
            _ => {
                self.state.clients.kill(self.id);
            }
        }
    }

    pub fn parse<'a, R: EiRequestParser<'a>>(
        &self,
        obj: &impl EiObject,
        mut parser: EiMsgParser<'_, 'a>,
    ) -> Result<R, EiMsgParserError> {
        let res = R::parse(&mut parser)?;
        parser.eof()?;
        log::trace!(
            "Client {} -> {}@{:x}.{:?}",
            self.id,
            obj.interface().name(),
            obj.id(),
            res
        );
        Ok(res)
    }

    pub fn event<T: EiEventFormatter>(self: &Rc<Self>, event: T) {
        log::trace!(
            "Client {} <= {}@{:x}.{:?}",
            self.id,
            event.interface().name(),
            event.id(),
            event,
        );
        let mut fds = vec![];
        let mut swapchain = self.swapchain.borrow_mut();
        let mut fmt = EiMsgFormatter::new(&mut swapchain.cur, &mut fds);
        event.format(&mut fmt);
        fmt.write_len();
        if swapchain.cur.is_full() {
            swapchain.commit();
            if swapchain.exceeds_limit() {
                if !self.checking_queue_size.replace(true) {
                    self.state.slow_ei_clients.push(self.clone());
                }
            }
        }
        self.flush_request.trigger();
    }

    pub async fn check_queue_size(&self) {
        if self.swapchain.borrow_mut().exceeds_limit() {
            self.state.eng.yield_now().await;
            if self.swapchain.borrow_mut().exceeds_limit() {
                log::error!("Client {} is too slow at fetching events", self.id);
                self.state.ei_clients.kill(self.id);
                return;
            }
        }
        self.checking_queue_size.set(false);
    }

    pub fn add_client_obj<T: EiObject>(&self, obj: &Rc<T>) -> Result<(), EiClientError> {
        self.objects.add_client_object(obj.clone())?;
        Ok(())
    }

    pub fn add_server_obj<T: EiObject>(&self, obj: &Rc<T>) {
        self.objects.add_server_object(obj.clone());
    }

    pub fn remove_obj<T: EiObject>(self: &Rc<Self>, obj: &T) -> Result<(), EiClientError> {
        self.objects.remove_obj(obj.id())
    }
}
