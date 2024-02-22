#![allow(clippy::declare_interior_mutable_const, clippy::type_complexity)]

use {
    crate::{
        _private::{
            bincode_ops,
            ipc::{ClientMessage, InitMessage, Response, ServerMessage},
            logging, Config, ConfigEntry, ConfigEntryGen, VERSION,
        },
        exec::Command,
        input::{acceleration::AccelProfile, capability::Capability, InputDevice, Seat},
        keyboard::Keymap,
        logging::LogLevel,
        theme::{colors::Colorable, sized::Resizable, Color},
        timer::Timer,
        video::{
            connector_type::{ConnectorType, CON_UNKNOWN},
            Connector, DrmDevice, GfxApi, Mode,
        },
        Axis, Direction, ModifiedKeySym, PciId, Workspace,
    },
    bincode::Options,
    std::{
        cell::{Cell, RefCell},
        collections::{hash_map::Entry, HashMap},
        ops::Deref,
        ptr,
        rc::Rc,
        slice,
        time::Duration,
    },
};

pub(crate) struct Client {
    configure: extern "C" fn(),
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    key_handlers: RefCell<HashMap<(Seat, ModifiedKeySym), Rc<dyn Fn()>>>,
    timer_handlers: RefCell<HashMap<Timer, Rc<dyn Fn()>>>,
    response: RefCell<Vec<Response>>,
    on_new_seat: RefCell<Option<Rc<dyn Fn(Seat)>>>,
    on_new_input_device: RefCell<Option<Rc<dyn Fn(InputDevice)>>>,
    on_connector_connected: RefCell<Option<Rc<dyn Fn(Connector)>>>,
    on_graphics_initialized: Cell<Option<Box<dyn FnOnce()>>>,
    on_devices_enumerated: Cell<Option<Box<dyn FnOnce()>>>,
    on_new_connector: RefCell<Option<Rc<dyn Fn(Connector)>>>,
    on_new_drm_device: RefCell<Option<Rc<dyn Fn(DrmDevice)>>>,
    on_del_drm_device: RefCell<Option<Rc<dyn Fn(DrmDevice)>>>,
    on_idle: RefCell<Option<Rc<dyn Fn()>>>,
    bufs: RefCell<Vec<Vec<u8>>>,
    reload: Cell<bool>,
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            (self.srv_unref)(self.srv_data);
        }
    }
}

thread_local! {
    pub(crate) static CLIENT: Cell<*const Client> = const { Cell::new(ptr::null()) };
}

unsafe fn with_client<T, F: FnOnce(&Client) -> T>(data: *const u8, f: F) -> T {
    struct Reset<'a> {
        cell: &'a Cell<*const Client>,
        val: *const Client,
    }
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.cell.set(self.val);
        }
    }
    CLIENT.with(|cell| {
        let client = data as *const Client;
        Rc::increment_strong_count(client);
        let client = Rc::from_raw(client);
        let old = cell.replace(client.deref());
        let _reset = Reset { cell, val: old };
        f(&client)
    })
}

impl<T: Config> ConfigEntryGen<T> {
    pub const ENTRY: ConfigEntry = ConfigEntry {
        version: VERSION,
        init: Self::init,
        unref,
        handle_msg,
    };

    pub unsafe extern "C" fn init(
        srv_data: *const u8,
        srv_unref: unsafe extern "C" fn(data: *const u8),
        srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
        init_data: *const u8,
        size: usize,
    ) -> *const u8 {
        logging::init();
        init(
            srv_data,
            srv_unref,
            srv_handler,
            init_data,
            size,
            T::configure,
        )
    }
}

pub unsafe extern "C" fn init(
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    init: *const u8,
    size: usize,
    f: extern "C" fn(),
) -> *const u8 {
    let client = Rc::new(Client {
        configure: f,
        srv_data,
        srv_unref,
        srv_handler,
        key_handlers: Default::default(),
        timer_handlers: Default::default(),
        response: Default::default(),
        on_new_seat: Default::default(),
        on_new_input_device: Default::default(),
        on_connector_connected: Default::default(),
        on_graphics_initialized: Default::default(),
        on_devices_enumerated: Default::default(),
        on_new_connector: Default::default(),
        on_new_drm_device: Default::default(),
        on_del_drm_device: Default::default(),
        on_idle: Default::default(),
        bufs: Default::default(),
        reload: Cell::new(false),
    });
    let init = slice::from_raw_parts(init, size);
    client.handle_init_msg(init);
    Rc::into_raw(client) as *const u8
}

pub unsafe extern "C" fn unref(data: *const u8) {
    let client = data as *const Client;
    drop(Rc::from_raw(client));
}

pub unsafe extern "C" fn handle_msg(data: *const u8, msg: *const u8, size: usize) {
    with_client(data, |client| {
        let msg = slice::from_raw_parts(msg, size);
        client.handle_msg(msg);
    });
}

macro_rules! get_response {
    ($res:expr, $def:expr, $ty:ident { $($field:ident),+ }) => {
        let ($($field,)+) = match $res {
            Response::$ty { $($field,)+ } => ($($field,)+),
            _ => {
                log::error!("Server did not send a response to a {} request", stringify!($ty));
                return $def;
            }
        };
    }
}

impl Client {
    fn send(&self, msg: &ClientMessage) {
        let mut buf = self.bufs.borrow_mut().pop().unwrap_or_default();
        buf.clear();
        bincode_ops().serialize_into(&mut buf, msg).unwrap();
        unsafe {
            (self.srv_handler)(self.srv_data, buf.as_ptr(), buf.len());
        }
        self.bufs.borrow_mut().push(buf);
    }

    fn send_with_response(&self, msg: &ClientMessage) -> Response {
        self.with_response(|| self.send(msg))
    }

    pub fn reload(&self) {
        self.send(&ClientMessage::Reload);
    }

    pub fn is_reload(&self) -> bool {
        self.reload.get()
    }

    pub fn spawn(&self, command: &Command) {
        let env = command
            .env
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        self.send(&ClientMessage::Run {
            prog: &command.prog,
            args: command.args.clone(),
            env,
        });
    }

    pub fn grab(&self, kb: InputDevice, grab: bool) {
        self.send(&ClientMessage::GrabKb { kb, grab });
    }

    pub fn focus(&self, seat: Seat, direction: Direction) {
        self.send(&ClientMessage::Focus { seat, direction });
    }

    pub fn move_(&self, seat: Seat, direction: Direction) {
        self.send(&ClientMessage::Move { seat, direction });
    }

    pub fn unbind<T: Into<ModifiedKeySym>>(&self, seat: Seat, mod_sym: T) {
        let mod_sym = mod_sym.into();
        let deregister = self
            .key_handlers
            .borrow_mut()
            .remove(&(seat, mod_sym))
            .is_some();
        if deregister {
            self.send(&ClientMessage::RemoveShortcut {
                seat,
                mods: mod_sym.mods,
                sym: mod_sym.sym,
            })
        }
    }

    fn with_response<F: FnOnce()>(&self, f: F) -> Response {
        f();
        self.response.borrow_mut().pop().unwrap_or(Response::None)
    }

    pub fn seats(&self) -> Vec<Seat> {
        let res = self.send_with_response(&ClientMessage::GetSeats);
        get_response!(res, vec![], GetSeats { seats });
        seats
    }

    pub fn mono(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetMono { seat });
        get_response!(res, false, GetMono { mono });
        mono
    }

    pub fn get_timer(&self, name: &str) -> Timer {
        let res = self.send_with_response(&ClientMessage::GetTimer { name });
        get_response!(res, Timer(0), GetTimer { timer });
        timer
    }

    pub fn remove_timer(&self, timer: Timer) {
        self.send(&ClientMessage::RemoveTimer { timer });
    }

    pub fn program_timer(
        &self,
        timer: Timer,
        initial: Option<Duration>,
        periodic: Option<Duration>,
    ) {
        self.send(&ClientMessage::ProgramTimer {
            timer,
            initial,
            periodic,
        });
    }

    pub fn on_timer_tick<F: Fn() + 'static>(&self, timer: Timer, f: F) {
        self.timer_handlers.borrow_mut().insert(timer, Rc::new(f));
    }

    pub fn get_workspace(&self, name: &str) -> Workspace {
        let res = self.send_with_response(&ClientMessage::GetWorkspace { name });
        get_response!(res, Workspace(0), GetWorkspace { workspace });
        workspace
    }

    pub fn get_connector(&self, ty: ConnectorType, idx: u32) -> Connector {
        let res = self.send_with_response(&ClientMessage::GetConnector { ty, idx });
        get_response!(res, Connector(0), GetConnector { connector });
        connector
    }

    pub fn get_seat_workspace(&self, seat: Seat) -> Workspace {
        let res = self.send_with_response(&ClientMessage::GetSeatWorkspace { seat });
        get_response!(res, Workspace(0), GetSeatWorkspace { workspace });
        workspace
    }

    pub fn set_default_workspace_capture(&self, capture: bool) {
        self.send(&ClientMessage::SetDefaultWorkspaceCapture { capture });
    }

    pub fn set_workspace_capture(&self, workspace: Workspace, capture: bool) {
        self.send(&ClientMessage::SetWorkspaceCapture { workspace, capture });
    }

    pub fn get_default_workspace_capture(&self) -> bool {
        let res = self.send_with_response(&ClientMessage::GetDefaultWorkspaceCapture);
        get_response!(res, true, GetDefaultWorkspaceCapture { capture });
        capture
    }

    pub fn get_workspace_capture(&self, workspace: Workspace) -> bool {
        let res = self.send_with_response(&ClientMessage::GetWorkspaceCapture { workspace });
        get_response!(res, true, GetWorkspaceCapture { capture });
        capture
    }

    pub fn show_workspace(&self, seat: Seat, workspace: Workspace) {
        self.send(&ClientMessage::ShowWorkspace { seat, workspace });
    }

    pub fn set_workspace(&self, seat: Seat, workspace: Workspace) {
        self.send(&ClientMessage::SetWorkspace { seat, workspace });
    }

    pub fn split(&self, seat: Seat) -> Axis {
        let res = self.send_with_response(&ClientMessage::GetSplit { seat });
        get_response!(res, Axis::Horizontal, GetSplit { axis });
        axis
    }

    pub fn disable_pointer_constraint(&self, seat: Seat) {
        self.send(&ClientMessage::DisablePointerConstraint { seat });
    }

    pub fn set_fullscreen(&self, seat: Seat, fullscreen: bool) {
        self.send(&ClientMessage::SetFullscreen { seat, fullscreen });
    }

    pub fn get_fullscreen(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetFullscreen { seat });
        get_response!(res, false, GetFullscreen { fullscreen });
        fullscreen
    }

    pub fn reset_font(&self) {
        self.send(&ClientMessage::ResetFont);
    }

    pub fn set_font(&self, font: &str) {
        self.send(&ClientMessage::SetFont { font });
    }

    pub fn get_font(&self) -> String {
        let res = self.send_with_response(&ClientMessage::GetFont);
        get_response!(res, String::new(), GetFont { font });
        font
    }

    pub fn get_floating(&self, seat: Seat) -> bool {
        let res = self.send_with_response(&ClientMessage::GetFloating { seat });
        get_response!(res, false, GetFloating { floating });
        floating
    }

    pub fn set_floating(&self, seat: Seat, floating: bool) {
        self.send(&ClientMessage::SetFloating { seat, floating });
    }

    pub fn toggle_floating(&self, seat: Seat) {
        self.set_floating(seat, !self.get_floating(seat));
    }

    pub fn reset_colors(&self) {
        self.send(&ClientMessage::ResetColors);
    }

    pub fn reset_sizes(&self) {
        self.send(&ClientMessage::ResetSizes);
    }

    pub fn get_color(&self, colorable: Colorable) -> Color {
        let res = self.send_with_response(&ClientMessage::GetColor { colorable });
        get_response!(res, Color::BLACK, GetColor { color });
        color
    }

    pub fn set_color(&self, colorable: Colorable, color: Color) {
        self.send(&ClientMessage::SetColor { colorable, color });
    }

    pub fn get_size(&self, sized: Resizable) -> i32 {
        let res = self.send_with_response(&ClientMessage::GetSize { sized });
        get_response!(res, 0, GetSize { size });
        size
    }

    pub fn set_cursor_size(&self, seat: Seat, size: i32) {
        self.send(&ClientMessage::SetCursorSize { seat, size })
    }

    pub fn set_use_hardware_cursor(&self, seat: Seat, use_hardware_cursor: bool) {
        self.send(&ClientMessage::SetUseHardwareCursor {
            seat,
            use_hardware_cursor,
        })
    }

    pub fn set_size(&self, sized: Resizable, size: i32) {
        self.send(&ClientMessage::SetSize { sized, size })
    }

    pub fn set_mono(&self, seat: Seat, mono: bool) {
        self.send(&ClientMessage::SetMono { seat, mono });
    }

    pub fn set_env(&self, key: &str, val: &str) {
        self.send(&ClientMessage::SetEnv { key, val });
    }

    pub fn set_status(&self, status: &str) {
        self.send(&ClientMessage::SetStatus { status });
    }

    pub fn set_split(&self, seat: Seat, axis: Axis) {
        self.send(&ClientMessage::SetSplit { seat, axis });
    }

    pub fn create_split(&self, seat: Seat, axis: Axis) {
        self.send(&ClientMessage::CreateSplit { seat, axis });
    }

    pub fn close(&self, seat: Seat) {
        self.send(&ClientMessage::Close { seat });
    }

    pub fn focus_parent(&self, seat: Seat) {
        self.send(&ClientMessage::FocusParent { seat });
    }

    pub fn get_seat(&self, name: &str) -> Seat {
        let res = self.send_with_response(&ClientMessage::GetSeat { name });
        get_response!(res, Seat(0), GetSeat { seat });
        seat
    }

    pub fn get_input_devices(&self, seat: Option<Seat>) -> Vec<InputDevice> {
        let res = self.send_with_response(&ClientMessage::GetInputDevices { seat });
        get_response!(res, vec!(), GetInputDevices { devices });
        devices
    }

    pub fn on_new_seat<F: Fn(Seat) + 'static>(&self, f: F) {
        *self.on_new_seat.borrow_mut() = Some(Rc::new(f));
    }

    pub fn quit(&self) {
        self.send(&ClientMessage::Quit)
    }

    pub fn switch_to_vt(&self, vtnr: u32) {
        self.send(&ClientMessage::SwitchTo { vtnr })
    }

    pub fn on_new_input_device<F: Fn(InputDevice) + 'static>(&self, f: F) {
        *self.on_new_input_device.borrow_mut() = Some(Rc::new(f));
    }

    pub fn connector_set_position(&self, connector: Connector, x: i32, y: i32) {
        self.send(&ClientMessage::ConnectorSetPosition { connector, x, y });
    }

    pub fn connector_set_enabled(&self, connector: Connector, enabled: bool) {
        self.send(&ClientMessage::ConnectorSetEnabled { connector, enabled });
    }

    pub fn device_connectors(&self, device: DrmDevice) -> Vec<Connector> {
        let res = self.send_with_response(&ClientMessage::GetDeviceConnectors { device });
        get_response!(res, vec![], GetDeviceConnectors { connectors });
        connectors
    }

    pub fn drm_device_syspath(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceSyspath { device });
        get_response!(res, String::new(), GetDrmDeviceSyspath { syspath });
        syspath
    }

    pub fn drm_device_vendor(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceVendor { device });
        get_response!(res, String::new(), GetDrmDeviceVendor { vendor });
        vendor
    }

    pub fn drm_device_model(&self, device: DrmDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDrmDeviceModel { device });
        get_response!(res, String::new(), GetDrmDeviceModel { model });
        model
    }

    pub fn drm_device_pci_id(&self, device: DrmDevice) -> PciId {
        let res = self.send_with_response(&ClientMessage::GetDrmDevicePciId { device });
        get_response!(res, Default::default(), GetDrmDevicePciId { pci_id });
        pci_id
    }

    pub fn make_render_device(&self, device: DrmDevice) {
        self.send(&ClientMessage::MakeRenderDevice { device });
    }

    pub fn set_gfx_api(&self, device: Option<DrmDevice>, api: GfxApi) {
        self.send(&ClientMessage::SetGfxApi { device, api });
    }

    pub fn set_direct_scanout_enabled(&self, device: Option<DrmDevice>, enabled: bool) {
        self.send(&ClientMessage::SetDirectScanoutEnabled { device, enabled });
    }

    pub fn connector_connected(&self, connector: Connector) -> bool {
        let res = self.send_with_response(&ClientMessage::ConnectorConnected { connector });
        get_response!(res, false, ConnectorConnected { connected });
        connected
    }

    pub fn connector_set_scale(&self, connector: Connector, scale: f64) {
        self.send(&ClientMessage::ConnectorSetScale { connector, scale });
    }

    pub fn connector_get_scale(&self, connector: Connector) -> f64 {
        let res = self.send_with_response(&ClientMessage::ConnectorGetScale { connector });
        get_response!(res, 1.0, ConnectorGetScale { scale });
        scale
    }

    pub fn connector_type(&self, connector: Connector) -> ConnectorType {
        let res = self.send_with_response(&ClientMessage::ConnectorType { connector });
        get_response!(res, CON_UNKNOWN, ConnectorType { ty });
        ty
    }

    pub fn connector_mode(&self, connector: Connector) -> Mode {
        let res = self.send_with_response(&ClientMessage::ConnectorMode { connector });
        get_response!(
            res,
            Mode::zeroed(),
            ConnectorMode {
                width,
                height,
                refresh_millihz
            }
        );
        Mode {
            width,
            height,
            refresh_millihz,
        }
    }

    pub fn connector_size(&self, connector: Connector) -> (i32, i32) {
        let res = self.send_with_response(&ClientMessage::ConnectorSize { connector });
        get_response!(res, (0, 0), ConnectorSize { width, height });
        (width, height)
    }

    pub fn drm_devices(&self) -> Vec<DrmDevice> {
        let res = self.send_with_response(&ClientMessage::GetDrmDevices);
        get_response!(res, vec![], GetDrmDevices { devices });
        devices
    }

    pub fn on_new_drm_device<F: Fn(DrmDevice) + 'static>(&self, f: F) {
        *self.on_new_drm_device.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_del_drm_device<F: Fn(DrmDevice) + 'static>(&self, f: F) {
        *self.on_del_drm_device.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_new_connector<F: Fn(Connector) + 'static>(&self, f: F) {
        *self.on_new_connector.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_idle<F: Fn() + 'static>(&self, f: F) {
        *self.on_idle.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_connector_connected<F: Fn(Connector) + 'static>(&self, f: F) {
        *self.on_connector_connected.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_graphics_initialized<F: FnOnce() + 'static>(&self, f: F) {
        self.on_graphics_initialized.set(Some(Box::new(f)));
    }

    pub fn on_devices_enumerated<F: FnOnce() + 'static>(&self, f: F) {
        self.on_devices_enumerated.set(Some(Box::new(f)));
    }

    pub fn set_seat(&self, device: InputDevice, seat: Seat) {
        self.send(&ClientMessage::SetSeat { device, seat })
    }

    pub fn set_left_handed(&self, device: InputDevice, left_handed: bool) {
        self.send(&ClientMessage::SetLeftHanded {
            device,
            left_handed,
        })
    }

    pub fn set_accel_profile(&self, device: InputDevice, profile: AccelProfile) {
        self.send(&ClientMessage::SetAccelProfile { device, profile })
    }

    pub fn set_accel_speed(&self, device: InputDevice, speed: f64) {
        self.send(&ClientMessage::SetAccelSpeed { device, speed })
    }

    pub fn set_transform_matrix(&self, device: InputDevice, matrix: [[f64; 2]; 2]) {
        self.send(&ClientMessage::SetTransformMatrix { device, matrix })
    }

    pub fn set_px_per_wheel_scroll(&self, device: InputDevice, px: f64) {
        self.send(&ClientMessage::SetPxPerWheelScroll { device, px })
    }

    pub fn set_input_tap_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetTapEnabled { device, enabled })
    }

    pub fn set_input_natural_scrolling_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetNaturalScrollingEnabled { device, enabled })
    }

    pub fn set_input_drag_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetDragEnabled { device, enabled })
    }

    pub fn set_input_drag_lock_enabled(&self, device: InputDevice, enabled: bool) {
        self.send(&ClientMessage::SetDragLockEnabled { device, enabled })
    }

    pub fn device_name(&self, device: InputDevice) -> String {
        let res = self.send_with_response(&ClientMessage::GetDeviceName { device });
        get_response!(res, String::new(), GetDeviceName { name });
        name
    }

    pub fn has_capability(&self, device: InputDevice, cap: Capability) -> bool {
        let res = self.send_with_response(&ClientMessage::HasCapability { device, cap });
        get_response!(res, false, HasCapability { has });
        has
    }

    pub fn seat_set_keymap(&self, seat: Seat, keymap: Keymap) {
        self.send(&ClientMessage::SeatSetKeymap { seat, keymap })
    }

    pub fn seat_set_repeat_rate(&self, seat: Seat, rate: i32, delay: i32) {
        self.send(&ClientMessage::SeatSetRepeatRate { seat, rate, delay })
    }

    pub fn seat_get_repeat_rate(&self, seat: Seat) -> (i32, i32) {
        let res = self.send_with_response(&ClientMessage::SeatGetRepeatRate { seat });
        get_response!(res, (25, 250), GetRepeatRate { rate, delay });
        (rate, delay)
    }

    pub fn parse_keymap(&self, keymap: &str) -> Keymap {
        let res = self.send_with_response(&ClientMessage::ParseKeymap { keymap });
        get_response!(res, Keymap(0), ParseKeymap { keymap });
        keymap
    }

    pub fn bind<T: Into<ModifiedKeySym>, F: Fn() + 'static>(&self, seat: Seat, mod_sym: T, f: F) {
        let mod_sym = mod_sym.into();
        let register = {
            let mut kh = self.key_handlers.borrow_mut();
            let f = Rc::new(f);
            match kh.entry((seat, mod_sym)) {
                Entry::Occupied(mut o) => {
                    *o.get_mut() = f;
                    false
                }
                Entry::Vacant(v) => {
                    v.insert(f);
                    true
                }
            }
        };
        if register {
            self.send(&ClientMessage::AddShortcut {
                seat,
                mods: mod_sym.mods,
                sym: mod_sym.sym,
            });
        }
    }

    pub fn log(&self, level: LogLevel, msg: &str, file: Option<&str>, line: Option<u32>) {
        self.send(&ClientMessage::Log {
            level,
            msg,
            file,
            line,
        })
    }

    fn handle_msg(&self, msg: &[u8]) {
        let res = bincode_ops().deserialize::<ServerMessage>(msg);
        let msg = match res {
            Ok(msg) => msg,
            Err(e) => {
                let msg = format!("could not deserialize message: {}", e);
                self.log(LogLevel::Error, &msg, None, None);
                return;
            }
        };
        match msg {
            ServerMessage::Configure { reload } => {
                self.reload.set(reload);
                (self.configure)();
                self.reload.set(false);
            }
            ServerMessage::Response { response } => {
                self.response.borrow_mut().push(response);
            }
            ServerMessage::InvokeShortcut { seat, mods, sym } => {
                let ms = ModifiedKeySym { mods, sym };
                let handler = self.key_handlers.borrow_mut().get(&(seat, ms)).cloned();
                if let Some(handler) = handler {
                    handler();
                }
            }
            ServerMessage::NewInputDevice { device } => {
                let handler = self.on_new_input_device.borrow_mut().clone();
                if let Some(handler) = handler {
                    handler(device);
                }
            }
            ServerMessage::DelInputDevice { .. } => {}
            ServerMessage::ConnectorConnect { device } => {
                let handler = self.on_connector_connected.borrow_mut().clone();
                if let Some(handler) = handler {
                    handler(device);
                }
            }
            ServerMessage::ConnectorDisconnect { .. } => {}
            ServerMessage::NewConnector { device } => {
                let handler = self.on_new_connector.borrow_mut().clone();
                if let Some(handler) = handler {
                    handler(device);
                }
            }
            ServerMessage::DelConnector { .. } => {}
            ServerMessage::TimerExpired { timer } => {
                let handler = self.timer_handlers.borrow_mut().get(&timer).cloned();
                if let Some(handler) = handler {
                    handler();
                }
            }
            ServerMessage::GraphicsInitialized => {
                if let Some(handler) = self.on_graphics_initialized.take() {
                    handler();
                }
            }
            ServerMessage::Clear => {
                // only used by test config
            }
            ServerMessage::NewDrmDev { device } => {
                let handler = self.on_new_drm_device.borrow_mut();
                if let Some(handler) = handler.deref() {
                    handler(device);
                }
            }
            ServerMessage::DelDrmDev { device } => {
                let handler = self.on_del_drm_device.borrow_mut();
                if let Some(handler) = handler.deref() {
                    handler(device);
                }
            }
            ServerMessage::Idle => {
                let handler = self.on_idle.borrow_mut();
                if let Some(handler) = handler.deref() {
                    handler();
                }
            }
            ServerMessage::DevicesEnumerated => {
                if let Some(handler) = self.on_devices_enumerated.take() {
                    handler();
                }
            }
        }
    }

    fn handle_init_msg(&self, msg: &[u8]) {
        let init = match bincode_ops().deserialize::<InitMessage>(msg) {
            Ok(m) => m,
            Err(e) => {
                let msg = format!("could not deserialize message: {}", e);
                self.log(LogLevel::Error, &msg, None, None);
                return;
            }
        };
        match init {
            InitMessage::V1(_) => {}
        }
    }
}
