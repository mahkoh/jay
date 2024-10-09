use {
    crate::{
        client::{ClientId, RequestParser},
        fixed::Fixed,
        ifs::wl_seat::WlSeatGlobal,
        it::{
            test_backend::{TestBackend, TestBackendKb, TestBackendMouse, TestConnector},
            test_client::TestClient,
            test_config::TestConfig,
            test_error::{TestError, TestErrorExt},
            test_ifs::test_display::TestDisplay,
            test_transport::TestTransport,
        },
        object::WL_DISPLAY_ID,
        state::State,
        tree::OutputNode,
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
    pub out_dir: String,
    pub in_dir: String,
    pub cfg: Rc<TestConfig>,
}

impl TestRun {
    pub async fn create_client(self: &Rc<Self>) -> Result<Rc<TestClient>, TestError> {
        self.create_client2()
            .await
            .with_context(|| "Could not create a client")
    }

    async fn create_client2(self: &Rc<Self>) -> Result<Rc<TestClient>, TestError> {
        let socket = uapi::socket(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0)
            .to_os_error()
            .with_context(|| "Could not create a unix socket")?;
        let socket = Rc::new(socket);
        self.backend
            .state
            .ring
            .connect(&socket, &self.server_addr)
            .await
            .with_context(|| "Could not connect to the compositor")?;
        let mut obj_ids = Bitfield::default();
        obj_ids.take(0);
        obj_ids.take(1);
        let tran = Rc::new(TestTransport {
            run: self.clone(),
            socket,
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
        jc.enable_symmetric_delete()?;
        let client_id = jc.get_client_id().await?;
        let client = self.state.clients.get(client_id)?;
        Ok(Rc::new(TestClient {
            run: self.clone(),
            server: client,
            tran,
            jc,
            comp: registry.get_compositor().await?,
            sub: registry.get_subcompositor().await?,
            shm: registry.get_shm().await?,
            spbm: registry.get_spbm().await?,
            viewporter: registry.get_viewporter().await?,
            xdg: registry.get_xdg().await?,
            activation: registry.get_activation().await?,
            data_device_manager: registry.get_data_device_manager().await?,
            cursor_shape_manager: registry.get_cursor_shape_manager().await?,
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

    pub async fn create_default_setup(&self) -> Result<DefaultSetup, TestError> {
        self.create_default_setup2(false).await
    }

    pub async fn create_default_setup2(&self, need_drm: bool) -> Result<DefaultSetup, TestError> {
        self.backend.install_default2(need_drm)?;
        let seat = self.get_seat("default")?;
        self.state.eng.yield_now().await;
        let output = match self.state.root.outputs.lock().values().next() {
            None => bail!("No output"),
            Some(d) => d.clone(),
        };
        self.cfg
            .set_input_device_seat(self.backend.default_kb.common.id, seat.id())?;
        self.cfg
            .set_input_device_seat(self.backend.default_mouse.common.id, seat.id())?;
        self.backend.default_mouse.click(1);
        self.state.eng.yield_now().await;
        self.cfg.show_workspace(seat.id(), "")?;
        Ok(DefaultSetup {
            connector: self.backend.default_connector.clone(),
            output,
            kb: self.backend.default_kb.clone(),
            mouse: self.backend.default_mouse.clone(),
            seat,
        })
    }

    pub async fn sync(&self) {
        self.state.eng.yield_now().await;
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

pub struct DefaultSetup {
    pub connector: Rc<TestConnector>,
    pub output: Rc<OutputNode>,
    pub kb: Rc<TestBackendKb>,
    pub mouse: Rc<TestBackendMouse>,
    pub seat: Rc<WlSeatGlobal>,
}

impl DefaultSetup {
    pub fn move_to(&self, x: i32, y: i32) {
        let (ox, oy) = self.seat.pointer_cursor().position();
        let (nx, ny) = (Fixed::from_int(x), Fixed::from_int(y));
        let (dx, dy) = (nx - ox, ny - oy);
        self.mouse.rel(dx.to_f64(), dy.to_f64())
    }
}
