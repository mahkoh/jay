use {
    crate::{
        backend::InputDeviceId,
        ifs::wl_seat::SeatId,
        it::test_error::{TestError, TestResult},
        tree::OutputNode,
        utils::{copyhashmap::CopyHashMap, stack::Stack},
    },
    bincode::Options,
    isnt::std_1::primitive::IsntConstPtrExt,
    jay_config::{
        _private::{
            ConfigEntry, VERSION, bincode_ops,
            ipc::{ClientMessage, Response, ServerMessage},
        },
        Axis, Direction,
        input::{InputDevice, Seat},
        keyboard::{Keymap, ModifiedKeySym},
        video::{Connector, Transform},
    },
    std::{cell::Cell, ops::Deref, ptr, rc::Rc, time::Duration},
};

pub static TEST_CONFIG_ENTRY: ConfigEntry = ConfigEntry {
    version: VERSION,
    init,
    unref,
    handle_msg,
};

thread_local! {
    static CONFIG: Cell<*const TestConfig> = const { Cell::new(ptr::null()) };
}

pub fn with_test_config<T, F>(f: F) -> T
where
    F: FnOnce(Rc<TestConfig>) -> T,
{
    let tc = Rc::new(TestConfig {
        srv: Cell::new(None),
        responses: Default::default(),
        invoked_shortcuts: Default::default(),
        graphics_initialized: Cell::new(false),
    });
    let old = CONFIG.get();
    CONFIG.set(tc.deref());
    let res = f(tc.clone());
    CONFIG.set(old);
    res
}

unsafe extern "C" fn init(
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    _msg: *const u8,
    _size: usize,
) -> *const u8 {
    let tc = CONFIG.get();
    assert!(tc.is_not_null());
    unsafe {
        Rc::increment_strong_count(tc);
        {
            let tc = &*tc;
            tc.srv.set(Some(ServerData {
                srv_data,
                srv_unref,
                srv_handler,
            }));
        }
        tc.cast()
    }
}

unsafe extern "C" fn unref(data: *const u8) {
    unsafe {
        Rc::decrement_strong_count(data.cast::<TestConfig>());
    }
}

unsafe extern "C" fn handle_msg(data: *const u8, msg: *const u8, size: usize) {
    let tc = unsafe { &*data.cast::<TestConfig>() };
    let msg = unsafe { std::slice::from_raw_parts(msg, size) };
    let res = bincode_ops().deserialize::<ServerMessage>(msg);
    let msg = match res {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("could not deserialize message: {}", e);
            return;
        }
    };
    match msg {
        ServerMessage::Configure { .. } => {}
        ServerMessage::Response { response } => {
            tc.responses.push(response);
        }
        ServerMessage::InvokeShortcut { seat, mods, sym } => {
            tc.invoked_shortcuts
                .set((SeatId::from_raw(seat.0 as _), mods | sym), ());
        }
        ServerMessage::InvokeShortcut2 {
            seat,
            unmasked_mods,
            effective_mods,
            sym,
        } => {
            let _ = unmasked_mods;
            tc.invoked_shortcuts
                .set((SeatId::from_raw(seat.0 as _), effective_mods | sym), ());
        }
        ServerMessage::NewInputDevice { .. } => {}
        ServerMessage::DelInputDevice { .. } => {}
        ServerMessage::ConnectorConnect { .. } => {}
        ServerMessage::ConnectorDisconnect { .. } => {}
        ServerMessage::NewConnector { .. } => {}
        ServerMessage::DelConnector { .. } => {}
        ServerMessage::TimerExpired { .. } => {}
        ServerMessage::GraphicsInitialized => tc.graphics_initialized.set(true),
        ServerMessage::Clear => tc.clear(),
        ServerMessage::NewDrmDev { .. } => {}
        ServerMessage::DelDrmDev { .. } => {}
        ServerMessage::Idle => {}
        ServerMessage::DevicesEnumerated => {}
        ServerMessage::InterestReady { .. } => {}
        ServerMessage::Features { .. } => {}
        ServerMessage::SwitchEvent { .. } => {}
        ServerMessage::ClientMatcherMatched { .. } => {}
        ServerMessage::ClientMatcherUnmatched { .. } => {}
    }
}

#[derive(Copy, Clone)]
struct ServerData {
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
}

pub struct TestConfig {
    srv: Cell<Option<ServerData>>,
    responses: Stack<Response>,
    pub invoked_shortcuts: CopyHashMap<(SeatId, ModifiedKeySym), ()>,
    pub graphics_initialized: Cell<bool>,
}

macro_rules! get_response {
    ($res:expr, $ty:ident { $($field:ident),+ }) => {
        let ($($field,)+) = match $res {
            Response::$ty { $($field,)+ } => ($($field,)+),
            _ => {
                bail!("Server did not send a response to a {} request", stringify!($ty));
            }
        };
    }
}

impl TestConfig {
    fn send(&self, msg: ClientMessage) -> Result<(), TestError> {
        self.send_(&msg)
    }

    fn send_(&self, msg: &ClientMessage) -> Result<(), TestError> {
        let srv = match self.srv.get() {
            Some(srv) => srv,
            _ => bail!("srv not set"),
        };
        let mut buf = vec![];
        bincode_ops().serialize_into(&mut buf, msg).unwrap();
        unsafe {
            (srv.srv_handler)(srv.srv_data, buf.as_ptr(), buf.len());
        }
        Ok(())
    }

    fn send_with_reply(&self, msg: ClientMessage) -> Result<Response, TestError> {
        self.send_(&msg)?;
        match self.responses.pop() {
            Some(r) => Ok(r),
            _ => bail!("Compositor did not send a response to {:?}", msg),
        }
    }

    pub fn quit(&self) -> Result<(), TestError> {
        self.send(ClientMessage::Quit)
    }

    pub fn get_seat(&self, name: &str) -> Result<SeatId, TestError> {
        let reply = self.send_with_reply(ClientMessage::GetSeat { name })?;
        get_response!(reply, GetSeat { seat });
        Ok(SeatId::from_raw(seat.0 as _))
    }

    pub fn show_workspace(&self, seat: SeatId, name: &str) -> Result<(), TestError> {
        let reply = self.send_with_reply(ClientMessage::GetWorkspace { name })?;
        get_response!(reply, GetWorkspace { workspace });
        self.send(ClientMessage::ShowWorkspace {
            seat: Seat(seat.raw() as _),
            workspace,
        })
    }

    pub fn parse_keymap(&self, keymap: &str) -> Result<Keymap, TestError> {
        let reply = self.send_with_reply(ClientMessage::ParseKeymap { keymap })?;
        get_response!(reply, ParseKeymap { keymap });
        if keymap.is_invalid() {
            bail!("Could not parse the keymap");
        }
        Ok(keymap)
    }

    pub fn set_keymap(&self, seat: SeatId, keymap: Keymap) -> TestResult {
        self.send(ClientMessage::SeatSetKeymap {
            seat: Seat(seat.raw() as _),
            keymap,
        })
    }

    pub fn create_split(&self, seat: SeatId, axis: Axis) -> TestResult {
        self.send(ClientMessage::CreateSeatSplit {
            seat: Seat(seat.raw() as _),
            axis,
        })
    }

    pub fn set_mono(&self, seat: SeatId, mono: bool) -> TestResult {
        self.send(ClientMessage::SetSeatMono {
            seat: Seat(seat.raw() as _),
            mono,
        })
    }

    pub fn add_shortcut<T: Into<ModifiedKeySym>>(
        &self,
        seat: SeatId,
        key: T,
    ) -> Result<(), TestError> {
        let key = key.into();
        self.send(ClientMessage::AddShortcut {
            seat: Seat(seat.raw() as _),
            mods: key.mods,
            sym: key.sym,
        })
    }

    pub fn set_input_device_seat(&self, id: InputDeviceId, seat: SeatId) -> Result<(), TestError> {
        self.send(ClientMessage::SetSeat {
            device: InputDevice(id.raw() as _),
            seat: Seat(seat.raw() as _),
        })
    }

    pub fn focus(&self, seat: SeatId, direction: Direction) -> TestResult {
        self.send(ClientMessage::SeatFocus {
            seat: Seat(seat.raw() as _),
            direction,
        })
    }

    pub fn set_fullscreen(&self, seat: SeatId, fs: bool) -> TestResult {
        self.send(ClientMessage::SetSeatFullscreen {
            seat: Seat(seat.raw() as _),
            fullscreen: fs,
        })
    }

    pub fn set_idle(&self, timeout: Duration) -> TestResult {
        self.send(ClientMessage::SetIdle { timeout })
    }

    pub fn set_idle_grace_period(&self, period: Duration) -> TestResult {
        self.send(ClientMessage::SetIdleGracePeriod { period })
    }

    pub fn set_floating(&self, seat: SeatId, floating: bool) -> TestResult {
        self.send(ClientMessage::SetSeatFloating {
            seat: Seat(seat.raw() as _),
            floating,
        })
    }

    fn clear(&self) {
        unsafe {
            if let Some(srv) = self.srv.take() {
                (srv.srv_unref)(srv.srv_data);
            }
        }
    }

    pub fn set_scale(&self, output: &OutputNode, scale: f64) -> TestResult {
        self.send(ClientMessage::ConnectorSetScale {
            connector: Connector(output.global.connector.connector.id().raw() as _),
            scale,
        })
    }

    pub fn set_output_transform(&self, output: &OutputNode, transform: Transform) -> TestResult {
        self.send(ClientMessage::ConnectorSetTransform {
            connector: Connector(output.global.connector.connector.id().raw() as _),
            transform,
        })
    }
}

impl Drop for TestConfig {
    fn drop(&mut self) {
        self.clear();
    }
}
