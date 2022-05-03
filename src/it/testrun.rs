use {
    crate::{
        client::{ClientId, RequestParser},
        ifs::wl_seat::WlSeatGlobal,
        it::{
            test_backend::TestBackend,
            test_client::TestClient,
            test_config::TestConfig,
            test_error::{TestError, TestErrorExt},
            test_ifs::test_display::TestDisplay,
            test_transport::TestTransport,
        },
        object::WL_DISPLAY_ID,
        state::State,
        utils::{bitfield::Bitfield, buffd::MsgParser, oserror::OsErrorExt, stack::Stack},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    uapi::c,
};

pub struct TestRun {
    pub state: Rc<State>,
    pub backend: Rc<TestBackend>,
    pub errors: Stack<String>,
    pub server_addr: c::sockaddr_un,
    pub dir: String,
    pub cfg: Rc<TestConfig>,
}

impl TestRun {
    pub async fn create_client(self: &Rc<Self>) -> Result<Rc<TestClient>, TestError> {
        self.create_client2()
            .await
            .with_context(|| "Could not create a client")
    }

    async fn create_client2(self: &Rc<Self>) -> Result<Rc<TestClient>, TestError> {
        let socket = uapi::socket(
            c::AF_UNIX,
            c::SOCK_STREAM | c::SOCK_CLOEXEC | c::SOCK_NONBLOCK,
            0,
        )
        .to_os_error()
        .with_context(|| "Could not create a unix socket")?;
        let socket = Rc::new(socket);
        uapi::connect(socket.raw(), &self.server_addr)
            .to_os_error()
            .with_context(|| "Could not connect to the compositor")?;
        let fd = self
            .state
            .eng
            .fd(&socket)
            .with_context(|| "Could not create an async fd")?;
        let mut obj_ids = Bitfield::default();
        obj_ids.take(0);
        obj_ids.take(1);
        let tran = Rc::new(TestTransport {
            run: self.clone(),
            fd,
            client_id: Cell::new(ClientId::from_raw(0)),
            bufs: Default::default(),
            swapchain: Default::default(),
            flush_request: Default::default(),
            incoming: Default::default(),
            outgoing: Default::default(),
            objects: Default::default(),
            obj_ids: RefCell::new(obj_ids),
            killed: Cell::new(false),
        });
        tran.add_obj(Rc::new(TestDisplay {
            tran: tran.clone(),
            id: WL_DISPLAY_ID,
        }))?;
        tran.init();
        let registry = tran.get_registry();
        let jc = registry.get_jay_compositor().await?;
        let client_id = jc.get_client_id().await?;
        let client = self.state.clients.get(client_id)?;
        Ok(Rc::new(TestClient {
            run: self.clone(),
            server: client,
            tran,
            jc,
            comp: registry.get_compositor().await?,
            shm: registry.get_shm().await?,
            xdg: registry.get_xdg().await?,
            registry,
        }))
    }

    pub fn get_seat(&self, name: &str) -> Result<Rc<WlSeatGlobal>, TestError> {
        let id = self.cfg.get_seat(name)?;
        for seat in self.state.globals.seats.lock().values() {
            if seat.id() == id {
                return Ok(seat.clone());
            }
        }
        bail!("Seat {} does not exist", id)
    }
}

pub trait ParseFull<'a>: Sized {
    fn parse_full(parser: MsgParser<'_, 'a>) -> Result<Self, TestError>;
}

impl<'a, T: RequestParser<'a>> ParseFull<'a> for T {
    fn parse_full(mut parser: MsgParser<'_, 'a>) -> Result<Self, TestError> {
        let res = T::parse(&mut parser)?;
        parser.eof()?;
        Ok(res)
    }
}
