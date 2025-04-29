use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            self, BackendColorSpace, BackendTransferFunction, ConnectorId, DrmDeviceId,
            InputDeviceAccelProfile, InputDeviceCapability, InputDeviceId,
        },
        client::{Client, ClientId},
        cmm::cmm_transfer_function::TransferFunction,
        compositor::MAX_EXTENTS,
        config::ConfigProxy,
        format::config_formats,
        ifs::wl_seat::{SeatId, WlSeatGlobal},
        io_uring::TaskResultExt,
        kbvm::{KbvmError, KbvmMap},
        output_schedule::map_cursor_hz,
        scale::Scale,
        state::{ConnectorData, DeviceHandlerData, DrmDevData, OutputData, State},
        theme::{Color, ThemeSized},
        tree::{
            ContainerNode, ContainerSplit, FloatNode, Node, NodeVisitorBase, OutputNode,
            TearingMode, ToplevelNode, VrrMode, WorkspaceNode, WsMoveConfig, move_ws_to_output,
            toplevel_create_split, toplevel_parent_container, toplevel_set_floating,
            toplevel_set_workspace,
        },
        utils::{
            asyncevent::AsyncEvent,
            copyhashmap::CopyHashMap,
            debug_fn::debug_fn,
            errorfmt::ErrorFmt,
            numcell::NumCell,
            oserror::OsError,
            stack::Stack,
            timer::{TimerError, TimerFd},
            toplevel_identifier::ToplevelIdentifier,
        },
    },
    bincode::Options,
    jay_config::{
        _private::{
            PollableId, WireMode, bincode_ops,
            ipc::{ClientMessage, Response, ServerMessage, WorkspaceSource},
        },
        Axis, Direction, Workspace,
        client::Client as ConfigClient,
        input::{
            FocusFollowsMouseMode, InputDevice, Seat,
            acceleration::{ACCEL_PROFILE_ADAPTIVE, ACCEL_PROFILE_FLAT, AccelProfile},
            capability::{
                CAP_GESTURE, CAP_KEYBOARD, CAP_POINTER, CAP_SWITCH, CAP_TABLET_PAD,
                CAP_TABLET_TOOL, CAP_TOUCH, Capability,
            },
        },
        keyboard::{Keymap, mods::Modifiers, syms::KeySym},
        logging::LogLevel,
        theme::{colors::Colorable, sized::Resizable},
        timer::Timer as JayTimer,
        video::{
            ColorSpace, Connector, DrmDevice, Format as ConfigFormat, GfxApi,
            TearingMode as ConfigTearingMode, TransferFunction as ConfigTransferFunction,
            Transform, VrrMode as ConfigVrrMode,
        },
        window::Window,
        xwayland::XScalingMode,
    },
    libloading::Library,
    log::Level,
    std::{cell::Cell, ops::Deref, rc::Rc, sync::Arc, time::Duration},
    thiserror::Error,
    uapi::{OwnedFd, c, fcntl_dupfd_cloexec},
};

pub(super) struct ConfigProxyHandler {
    pub path: Option<String>,
    pub client_data: Cell<*const u8>,
    pub dropped: Cell<bool>,
    pub _lib: Option<Library>,
    pub _version: u32,
    pub unref: unsafe extern "C" fn(data: *const u8),
    pub handle_msg: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    pub state: Rc<State>,
    pub next_id: NumCell<u64>,
    pub keymaps: CopyHashMap<Keymap, Rc<KbvmMap>>,
    pub bufs: Stack<Vec<u8>>,

    pub workspace_ids: NumCell<u64>,
    pub workspaces_by_name: CopyHashMap<Rc<String>, u64>,
    pub workspaces_by_id: CopyHashMap<u64, Rc<String>>,

    pub timer_ids: NumCell<u64>,
    pub timers_by_name: CopyHashMap<Rc<String>, Rc<TimerData>>,
    pub timers_by_id: CopyHashMap<u64, Rc<TimerData>>,

    pub pollable_id: NumCell<u64>,
    pub pollables: CopyHashMap<PollableId, Rc<Pollable>>,

    pub window_ids: NumCell<u64>,
    pub windows_from_tl_id: CopyHashMap<ToplevelIdentifier, Window>,
    pub windows_to_tl_id: CopyHashMap<Window, ToplevelIdentifier>,
}

pub struct Pollable {
    write_trigger: Rc<AsyncEvent>,
    _write_future: SpawnedFuture<()>,
    read_trigger: Rc<AsyncEvent>,
    _read_future: SpawnedFuture<()>,
}

pub(super) struct TimerData {
    timer: TimerFd,
    id: u64,
    name: Rc<String>,
    _handler: SpawnedFuture<()>,
}

impl ConfigProxyHandler {
    pub fn do_drop(&self) {
        self.dropped.set(true);

        self.timers_by_name.clear();
        self.timers_by_id.clear();

        self.pollables.clear();

        if let Some(path) = &self.path {
            if let Err(e) = uapi::unlink(path.as_str()) {
                log::error!("Could not unlink {}: {}", path, ErrorFmt(OsError(e.0)));
            }
        }
    }

    pub fn send(&self, msg: &ServerMessage) {
        let mut buf = self.bufs.pop().unwrap_or_default();
        buf.clear();
        bincode_ops().serialize_into(&mut buf, msg).unwrap();
        unsafe {
            (self.handle_msg)(self.client_data.get(), buf.as_ptr(), buf.len());
        }
        self.bufs.push(buf);
    }

    pub fn respond(&self, msg: Response) {
        self.send(&ServerMessage::Response { response: msg })
    }

    fn id(&self) -> u64 {
        self.next_id.fetch_add(1)
    }

    fn get_workspace_by_name(&self, name: &String) -> Workspace {
        let id = match self.workspaces_by_name.get(name) {
            None => {
                let id = self.workspace_ids.fetch_add(1);
                let name = Rc::new(name.clone());
                self.workspaces_by_name.set(name.clone(), id);
                self.workspaces_by_id.set(id, name);
                id
            }
            Some(id) => id,
        };
        Workspace(id)
    }

    fn handle_log_request(
        &self,
        level: LogLevel,
        msg: &str,
        file: Option<&str>,
        line: Option<u32>,
    ) {
        let level = match level {
            LogLevel::Error => Level::Error,
            LogLevel::Warn => Level::Warn,
            LogLevel::Info => Level::Info,
            LogLevel::Debug => Level::Debug,
            LogLevel::Trace => Level::Trace,
        };
        let debug = debug_fn(|fmt| {
            if let Some(file) = file {
                write!(fmt, "{}", file)?;
                if let Some(line) = line {
                    write!(fmt, ":{}", line)?;
                }
                write!(fmt, ": ")?;
            }
            write!(fmt, "{}", msg)?;
            Ok(())
        });
        log::log!(level, "{:?}", debug);
    }

    fn handle_get_seat(&self, name: &str) {
        for seat in self.state.globals.seats.lock().values() {
            if seat.seat_name() == name {
                self.respond(Response::GetSeat {
                    seat: Seat(seat.id().raw() as _),
                });
                return;
            }
        }
        let seat = self.state.create_seat(name);
        self.respond(Response::GetSeat {
            seat: Seat(seat.id().raw() as _),
        });
    }

    fn handle_parse_keymap(&self, keymap: &str) -> Result<(), CphError> {
        let (keymap, res) = match self.state.kb_ctx.parse_keymap(keymap.as_bytes()) {
            Ok(keymap) => {
                let id = Keymap(self.id());
                self.keymaps.set(id, keymap);
                (id, Ok(()))
            }
            Err(e) => (Keymap::INVALID, Err(CphError::ParseKeymapError(e))),
        };
        self.respond(Response::ParseKeymap { keymap });
        res
    }

    fn handle_get_connectors(
        &self,
        dev: Option<DrmDevice>,
        connected_only: bool,
    ) -> Result<(), CphError> {
        let datas: Vec<_>;
        if let Some(dev) = dev {
            let dev = self.get_drm_device(dev)?;
            datas = dev.connectors.lock().values().cloned().collect();
        } else {
            datas = self.state.connectors.lock().values().cloned().collect();
        }
        let connectors = datas
            .iter()
            .flat_map(|d| match (connected_only, d.connected.get()) {
                (false, _) | (true, true) => Some(Connector(d.connector.id().raw() as _)),
                _ => None,
            })
            .collect();
        self.respond(Response::GetConnectors { connectors });
        Ok(())
    }

    fn handle_get_drm_device_syspath(&self, dev: DrmDevice) -> Result<(), CphError> {
        let dev = self.get_drm_device(dev)?;
        let syspath = dev.syspath.clone().unwrap_or_default();
        self.respond(Response::GetDrmDeviceSyspath { syspath });
        Ok(())
    }

    fn handle_get_drm_device_devnode(&self, dev: DrmDevice) -> Result<(), CphError> {
        let dev = self.get_drm_device(dev)?;
        let devnode = dev.devnode.clone().unwrap_or_default();
        self.respond(Response::GetDrmDeviceDevnode { devnode });
        Ok(())
    }

    fn handle_get_drm_device_vendor(&self, dev: DrmDevice) -> Result<(), CphError> {
        let dev = self.get_drm_device(dev)?;
        let vendor = dev.vendor.clone().unwrap_or_default();
        self.respond(Response::GetDrmDeviceVendor { vendor });
        Ok(())
    }

    fn handle_get_drm_devices(&self) {
        let devs = self.state.drm_devs.lock();
        let mut res = vec![];
        for dev in devs.values() {
            res.push(DrmDevice(dev.dev.id().raw() as _));
        }
        self.respond(Response::GetDrmDevices { devices: res });
    }

    fn handle_make_render_device(&self, dev: DrmDevice) -> Result<(), CphError> {
        let dev = self.get_drm_device(dev)?;
        dev.make_render_device();
        Ok(())
    }

    fn handle_get_drm_device_model(&self, dev: DrmDevice) -> Result<(), CphError> {
        let dev = self.get_drm_device(dev)?;
        let model = dev.model.clone().unwrap_or_default();
        self.respond(Response::GetDrmDeviceModel { model });
        Ok(())
    }

    fn handle_get_drm_device_pci_id(&self, dev: DrmDevice) -> Result<(), CphError> {
        let dev = self.get_drm_device(dev)?;
        let pci_id = dev.pci_id.unwrap_or_default();
        self.respond(Response::GetDrmDevicePciId { pci_id });
        Ok(())
    }

    fn handle_reload(&self) {
        log::info!("Reloading config");
        let config = match ConfigProxy::from_config_dir(&self.state) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Cannot reload config: {}", ErrorFmt(e));
                return;
            }
        };
        if let Some(config) = self.state.config.take() {
            config.destroy();
            for seat in self.state.globals.seats.lock().values() {
                seat.clear_shortcuts();
            }
        }
        config.configure(true);
        self.state.config.set(Some(Rc::new(config)));
    }

    fn handle_get_seat_fullscreen(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetFullscreen {
            fullscreen: seat.get_fullscreen(),
        });
        Ok(())
    }

    fn handle_set_seat_fullscreen(&self, seat: Seat, fullscreen: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_fullscreen(fullscreen);
        Ok(())
    }

    fn handle_get_window_fullscreen(&self, window: Window) -> Result<(), CphError> {
        let tl = self.get_window(window)?;
        self.respond(Response::GetWindowFullscreen {
            fullscreen: tl.tl_data().is_fullscreen.get(),
        });
        Ok(())
    }

    fn handle_set_window_fullscreen(
        &self,
        window: Window,
        fullscreen: bool,
    ) -> Result<(), CphError> {
        let tl = self.get_window(window)?;
        tl.tl_set_fullscreen(fullscreen);
        Ok(())
    }

    fn handle_set_keymap(&self, seat: Seat, keymap: Keymap) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let keymap = if keymap.is_invalid() {
            self.state.default_keymap.clone()
        } else {
            self.get_keymap(keymap)?
        };
        seat.set_seat_keymap(&keymap);
        Ok(())
    }

    fn handle_set_device_keymap(
        &self,
        device: InputDevice,
        keymap: Keymap,
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        let map = if keymap.is_invalid() {
            None
        } else {
            Some(self.get_keymap(keymap)?)
        };
        dev.set_keymap(map);
        Ok(())
    }

    fn handle_set_forward(&self, seat: Seat, forward: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_forward(forward);
        Ok(())
    }

    fn handle_set_focus_follows_mouse_mode(
        &self,
        seat: Seat,
        mode: FocusFollowsMouseMode,
    ) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let focus_follows_mouse = match mode {
            FocusFollowsMouseMode::True => true,
            FocusFollowsMouseMode::False => false,
        };
        seat.set_focus_follows_mouse(focus_follows_mouse);
        Ok(())
    }

    fn handle_set_window_management_enabled(
        &self,
        seat: Seat,
        enabled: bool,
    ) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_window_management_enabled(enabled);
        Ok(())
    }

    fn handle_set_input_device_connector(
        &self,
        input_device: InputDevice,
        connector: Connector,
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(input_device)?;
        let output = self.get_output_node(connector)?;
        dev.set_output(Some(&output.global));
        Ok(())
    }

    fn handle_remove_input_mapping(&self, input_device: InputDevice) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(input_device)?;
        dev.set_output(None);
        Ok(())
    }

    fn handle_set_status(&self, status: &str) {
        self.state.set_status(status);
    }

    fn get_timer(&self, timer: JayTimer) -> Result<Rc<TimerData>, CphError> {
        match self.timers_by_id.get(&timer.0) {
            Some(t) => Ok(t),
            _ => Err(CphError::TimerDoesNotExist(timer)),
        }
    }

    fn handle_remove_timer(&self, timer: JayTimer) -> Result<(), CphError> {
        let timer = self.get_timer(timer)?;
        self.timers_by_id.remove(&timer.id);
        self.timers_by_name.remove(&timer.name);
        Ok(())
    }

    fn handle_set_env(&self, key: &str, val: &str) {
        if let Some(f) = self.state.forker.get() {
            f.setenv(key.as_bytes(), val.as_bytes());
        }
    }

    fn handle_unset_env(&self, key: &str) {
        if let Some(f) = self.state.forker.get() {
            f.unsetenv(key.as_bytes());
        }
    }

    fn handle_get_config_dir(&self) {
        let dir = self.state.config_dir.clone().unwrap_or_default();
        self.respond(Response::GetConfigDir { dir });
    }

    fn handle_get_workspaces(&self) {
        let mut workspaces = vec![];
        for ws in self.state.workspaces.lock().values() {
            workspaces.push(self.get_workspace_by_name(&ws.name));
        }
        self.respond(Response::GetWorkspaces { workspaces });
    }

    fn handle_program_timer(
        &self,
        timer: JayTimer,
        initial: Option<Duration>,
        periodic: Option<Duration>,
    ) -> Result<(), CphError> {
        let timer = self.get_timer(timer)?;
        timer.timer.program(initial, periodic)?;
        Ok(())
    }

    fn handle_get_timer(self: &Rc<Self>, name: &str) -> Result<(), CphError> {
        let name = Rc::new(name.to_owned());
        if let Some(t) = self.timers_by_name.get(&name) {
            self.respond(Response::GetTimer {
                timer: JayTimer(t.id),
            });
            return Ok(());
        }
        let id = self.timer_ids.fetch_add(1);
        let timer = TimerFd::new(c::CLOCK_BOOTTIME)?;
        let handler = {
            let timer = timer.clone();
            let slf = self.clone();
            self.state.eng.spawn("config timer", async move {
                loop {
                    match timer.expired(&slf.state.ring).await {
                        Ok(_) => slf.send(&ServerMessage::TimerExpired {
                            timer: JayTimer(id),
                        }),
                        Err(e) => {
                            log::error!("Could not wait for timer expiration: {}", ErrorFmt(e));
                            if let Some(timer) = slf.timers_by_id.remove(&id) {
                                slf.timers_by_name.remove(&timer.name);
                            }
                            return;
                        }
                    }
                }
            })
        };
        let td = Rc::new(TimerData {
            timer,
            id,
            name: name.clone(),
            _handler: handler,
        });
        self.timers_by_name.set(name.clone(), td.clone());
        self.timers_by_id.set(id, td.clone());
        self.respond(Response::GetTimer {
            timer: JayTimer(id),
        });
        Ok(())
    }

    fn handle_seat_close(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.close();
        Ok(())
    }

    fn handle_window_close(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        window.tl_close();
        Ok(())
    }

    fn handle_seat_focus(&self, seat: Seat, direction: Direction) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.move_focus(direction.into());
        Ok(())
    }

    fn handle_seat_move(&self, seat: Seat, direction: Direction) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.move_focused(direction.into());
        Ok(())
    }

    fn handle_window_move(&self, window: Window, direction: Direction) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        if let Some(c) = toplevel_parent_container(&*window) {
            c.move_child(window, direction.into());
        }
        Ok(())
    }

    fn handle_get_repeat_rate(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let (rate, delay) = seat.get_rate();
        self.respond(Response::GetRepeatRate { rate, delay });
        Ok(())
    }

    fn handle_set_repeat_rate(&self, seat: Seat, rate: i32, delay: i32) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        if rate < 0 {
            return Err(CphError::NegativeRepeatRate);
        }
        if delay < 0 {
            return Err(CphError::NegativeRepeatDelay);
        }
        seat.set_rate(rate, delay);
        Ok(())
    }

    fn get_workspace(&self, ws: Workspace) -> Result<Rc<String>, CphError> {
        match self.workspaces_by_id.get(&ws.0) {
            Some(ws) => Ok(ws),
            _ => Err(CphError::WorkspaceDoesNotExist(ws)),
        }
    }

    fn get_existing_workspace(&self, ws: Workspace) -> Result<Option<Rc<WorkspaceNode>>, CphError> {
        self.get_workspace(ws)
            .map(|ws| self.state.workspaces.get(&*ws))
    }

    fn get_device_handler_data(
        &self,
        device: InputDevice,
    ) -> Result<Rc<DeviceHandlerData>, CphError> {
        let data = self
            .state
            .input_device_handlers
            .borrow_mut()
            .get(&InputDeviceId::from_raw(device.0 as _))
            .map(|d| d.data.clone());
        match data {
            Some(d) => Ok(d),
            _ => Err(CphError::DeviceDoesNotExist(device)),
        }
    }

    fn get_connector(&self, connector: Connector) -> Result<Rc<ConnectorData>, CphError> {
        let data = self
            .state
            .connectors
            .get(&ConnectorId::from_raw(connector.0 as _));
        match data {
            Some(d) => Ok(d),
            _ => Err(CphError::ConnectorDoesNotExist(connector)),
        }
    }

    fn get_output(&self, connector: Connector) -> Result<Rc<OutputData>, CphError> {
        let data = self
            .state
            .outputs
            .get(&ConnectorId::from_raw(connector.0 as _));
        match data {
            Some(d) => Ok(d),
            _ => Err(CphError::OutputDoesNotExist(connector)),
        }
    }

    fn get_output_node(&self, connector: Connector) -> Result<Rc<OutputNode>, CphError> {
        let data = self.get_output(connector)?;
        match data.node.clone() {
            Some(d) => Ok(d),
            _ => Err(CphError::OutputIsNotDesktop(connector)),
        }
    }

    fn get_drm_device(&self, dev: DrmDevice) -> Result<Rc<DrmDevData>, CphError> {
        match self.state.drm_devs.get(&DrmDeviceId::from_raw(dev.0 as _)) {
            Some(dev) => Ok(dev),
            _ => Err(CphError::DrmDeviceDoesNotExist(dev)),
        }
    }

    fn get_seat(&self, seat: Seat) -> Result<Rc<WlSeatGlobal>, CphError> {
        let seats = self.state.globals.seats.lock();
        for seat_global in seats.values() {
            if seat_global.id().raw() == seat.0 as u32 {
                return Ok(seat_global.clone());
            }
        }
        Err(CphError::SeatDoesNotExist(seat))
    }

    fn get_kb(&self, kb: InputDevice) -> Result<Rc<dyn backend::InputDevice>, CphError> {
        let kbs = self.state.input_device_handlers.borrow_mut();
        match kbs.get(&(InputDeviceId::from_raw(kb.0 as _))) {
            None => Err(CphError::KeyboardDoesNotExist(kb)),
            Some(kb) => Ok(kb.data.device.clone()),
        }
    }

    fn get_keymap(&self, keymap: Keymap) -> Result<Rc<KbvmMap>, CphError> {
        match self.keymaps.get(&keymap) {
            Some(k) => Ok(k),
            None => Err(CphError::KeymapDoesNotExist(keymap)),
        }
    }

    fn handle_set_seat(&self, device: InputDevice, seat: Seat) -> Result<(), CphError> {
        let seat = if seat.is_invalid() {
            None
        } else {
            Some(self.get_seat(seat)?)
        };
        let dev = self.get_device_handler_data(device)?;
        dev.set_seat(seat);
        Ok(())
    }

    fn handle_set_left_handed(
        &self,
        device: InputDevice,
        left_handed: bool,
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_left_handed(left_handed);
        Ok(())
    }

    fn handle_set_accel_profile(
        &self,
        device: InputDevice,
        accel_profile: AccelProfile,
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        let profile = match accel_profile {
            ACCEL_PROFILE_FLAT => InputDeviceAccelProfile::Flat,
            ACCEL_PROFILE_ADAPTIVE => InputDeviceAccelProfile::Adaptive,
            _ => return Err(CphError::UnknownAccelProfile(accel_profile)),
        };
        dev.device.set_accel_profile(profile);
        Ok(())
    }

    fn handle_set_accel_speed(&self, device: InputDevice, speed: f64) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_accel_speed(speed);
        Ok(())
    }

    fn handle_set_px_per_wheel_scroll(&self, device: InputDevice, px: f64) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.px_per_scroll_wheel.set(px);
        Ok(())
    }

    fn handle_set_tap_enabled(&self, device: InputDevice, enabled: bool) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_tap_enabled(enabled);
        Ok(())
    }

    fn handle_set_drag_enabled(&self, device: InputDevice, enabled: bool) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_drag_enabled(enabled);
        Ok(())
    }

    fn handle_set_natural_scrolling_enabled(
        &self,
        device: InputDevice,
        enabled: bool,
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_natural_scrolling_enabled(enabled);
        Ok(())
    }

    fn handle_set_drag_lock_enabled(
        &self,
        device: InputDevice,
        enabled: bool,
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_drag_lock_enabled(enabled);
        Ok(())
    }

    fn handle_set_transform_matrix(
        &self,
        device: InputDevice,
        matrix: [[f64; 2]; 2],
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_transform_matrix(matrix);
        Ok(())
    }

    fn handle_set_calibration_matrix(
        &self,
        device: InputDevice,
        matrix: [[f32; 3]; 2],
    ) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        dev.device.set_calibration_matrix(matrix);
        Ok(())
    }

    fn handle_set_ei_socket_enabled(&self, enabled: bool) {
        self.state.enable_ei_acceptor.set(enabled);
        self.state.update_ei_acceptor();
    }

    fn handle_get_workspace(&self, name: &str) {
        self.respond(Response::GetWorkspace {
            workspace: self.get_workspace_by_name(&name.to_owned()),
        });
    }

    fn handle_get_workspace_capture(&self, workspace: Workspace) -> Result<(), CphError> {
        let ws = self.get_existing_workspace(workspace)?;
        let capture = match ws {
            Some(ws) => ws.may_capture.get(),
            None => self.state.default_workspace_capture.get(),
        };
        self.respond(Response::GetWorkspaceCapture { capture });
        Ok(())
    }

    fn handle_set_workspace_capture(
        &self,
        workspace: Workspace,
        capture: bool,
    ) -> Result<(), CphError> {
        if let Some(ws) = self.get_existing_workspace(workspace)? {
            ws.may_capture.set(capture);
            ws.update_has_captures();
        }
        Ok(())
    }

    fn handle_set_gfx_api(&self, device: Option<DrmDevice>, api: GfxApi) -> Result<(), CphError> {
        match device {
            Some(dev) => self.get_drm_device(dev)?.dev.set_gfx_api(api),
            _ => self.state.default_gfx_api.set(api),
        }
        Ok(())
    }

    fn handle_set_flip_margin(&self, device: DrmDevice, margin: Duration) -> Result<(), CphError> {
        self.get_drm_device(device)?
            .dev
            .set_flip_margin(margin.as_nanos().try_into().unwrap_or(u64::MAX));
        Ok(())
    }

    fn handle_set_x_scaling_mode(&self, mode: XScalingMode) -> Result<(), CphError> {
        let use_wire_scale = match mode {
            XScalingMode::DEFAULT => false,
            XScalingMode::DOWNSCALED => true,
            _ => return Err(CphError::UnknownXScalingMode(mode)),
        };
        self.state.xwayland.use_wire_scale.set(use_wire_scale);
        self.state.update_xwayland_wire_scale();
        Ok(())
    }

    fn handle_set_ui_drag_enabled(&self, enabled: bool) {
        self.state.ui_drag_enabled.set(enabled);
    }

    fn handle_set_ui_drag_threshold(&self, threshold: i32) {
        let threshold = threshold.max(1);
        let squared = threshold.saturating_mul(threshold);
        self.state.ui_drag_threshold_squared.set(squared);
    }

    fn handle_set_direct_scanout_enabled(
        &self,
        device: Option<DrmDevice>,
        enabled: bool,
    ) -> Result<(), CphError> {
        match device {
            Some(dev) => self
                .get_drm_device(dev)?
                .dev
                .set_direct_scanout_enabled(enabled),
            _ => self.state.direct_scanout_enabled.set(enabled),
        }
        Ok(())
    }

    fn handle_get_default_workspace_capture(&self) {
        self.respond(Response::GetDefaultWorkspaceCapture {
            capture: self.state.default_workspace_capture.get(),
        });
    }

    fn handle_set_default_workspace_capture(&self, capture: bool) {
        self.state.default_workspace_capture.set(capture);
    }

    fn handle_set_double_click_interval_usec(&self, usec: u64) {
        self.state.double_click_interval_usec.set(usec);
    }

    fn handle_set_double_click_distance(&self, dist: i32) {
        self.state.double_click_distance.set(dist);
    }

    fn handle_get_seat_workspace(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let output = seat.get_output();
        let mut workspace = Workspace(0);
        if !output.is_dummy {
            if let Some(ws) = output.workspace.get() {
                workspace = self.get_workspace_by_name(&ws.name);
            }
        }
        self.respond(Response::GetSeatWorkspace { workspace });
        Ok(())
    }

    fn handle_get_seat_keyboard_workspace(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let mut workspace = Workspace(0);
        if let Some(output) = seat.get_keyboard_output() {
            if !output.is_dummy {
                if let Some(ws) = output.workspace.get() {
                    workspace = self.get_workspace_by_name(&ws.name);
                }
            }
        }
        self.respond(Response::GetSeatKeyboardWorkspace { workspace });
        Ok(())
    }

    fn handle_show_workspace(&self, seat: Seat, ws: Workspace) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let name = self.get_workspace(ws)?;
        self.state.show_workspace(&seat, &name);
        Ok(())
    }

    fn handle_set_seat_workspace(&self, seat: Seat, ws: Workspace) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let name = self.get_workspace(ws)?;
        let workspace = match self.state.workspaces.get(name.deref()) {
            Some(ws) => ws,
            _ => seat.get_output().create_workspace(name.deref()),
        };
        seat.set_workspace(&workspace);
        Ok(())
    }

    fn handle_set_window_workspace(&self, window: Window, ws: Workspace) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        let name = self.get_workspace(ws)?;
        let workspace = match self.state.workspaces.get(name.deref()) {
            Some(ws) => ws,
            _ => match window.node_output() {
                Some(o) => o.create_workspace(name.deref()),
                _ => return Ok(()),
            },
        };
        toplevel_set_workspace(&self.state, window, &workspace);
        Ok(())
    }

    fn handle_get_device_name(&self, device: InputDevice) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        let name = dev.device.name();
        self.respond(Response::GetDeviceName {
            name: name.to_string(),
        });
        Ok(())
    }

    fn handle_get_input_device_syspath(&self, device: InputDevice) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        self.respond(Response::GetInputDeviceSyspath {
            syspath: dev.syspath.clone().unwrap_or_default(),
        });
        Ok(())
    }

    fn handle_get_input_device_devnode(&self, device: InputDevice) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        self.respond(Response::GetInputDeviceDevnode {
            devnode: dev.devnode.clone().unwrap_or_default(),
        });
        Ok(())
    }

    fn handle_move_to_output(
        &self,
        workspace: WorkspaceSource,
        connector: Connector,
    ) -> Result<(), CphError> {
        let output = self.get_output_node(connector)?;
        let ws = match workspace {
            WorkspaceSource::Explicit(ws) => match self.get_existing_workspace(ws)? {
                Some(ws) => ws,
                _ => return Ok(()),
            },
            WorkspaceSource::Seat(s) => match self.get_seat(s)?.get_output().workspace.get() {
                Some(ws) => ws,
                _ => return Ok(()),
            },
        };
        if ws.is_dummy || output.is_dummy {
            return Ok(());
        }
        if ws.output.get().id == output.id {
            return Ok(());
        }
        let link = match &*ws.output_link.borrow() {
            None => return Ok(()),
            Some(l) => l.to_ref(),
        };
        let config = WsMoveConfig {
            make_visible_always: false,
            make_visible_if_empty: true,
            source_is_destroyed: false,
            before: None,
        };
        move_ws_to_output(&link, &output, config);
        ws.desired_output.set(output.global.output_id.clone());
        self.state.tree_changed();
        Ok(())
    }

    fn handle_set_idle(&self, timeout: Duration) {
        self.state.idle.set_timeout(timeout);
    }

    fn handle_set_idle_grace_period(&self, period: Duration) {
        self.state.idle.set_grace_period(period);
    }

    fn handle_set_explicit_sync_enabled(&self, enabled: bool) {
        self.state.explicit_sync_enabled.set(enabled);
    }

    fn handle_set_color_management_enabled(&self, enabled: bool) {
        self.state.color_management_enabled.set(enabled);
    }

    fn handle_get_socket_path(&self) {
        match self.state.acceptor.get() {
            Some(a) => {
                self.respond(Response::GetSocketPath {
                    path: a.socket_name().to_string(),
                });
            }
            _ => {
                log::warn!("There is no acceptor");
            }
        }
    }

    fn handle_connector_connected(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_connector(connector)?;
        self.respond(Response::ConnectorConnected {
            connected: connector.connected.get(),
        });
        Ok(())
    }

    fn handle_connector_type(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_connector(connector)?;
        self.respond(Response::ConnectorType {
            ty: connector.connector.kernel_id().ty.to_config(),
        });
        Ok(())
    }

    fn handle_connector_mode(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        let mode = connector.global.mode.get();
        self.respond(Response::ConnectorMode {
            width: mode.width,
            height: mode.height,
            refresh_millihz: mode.refresh_rate_millihz,
        });
        Ok(())
    }

    fn handle_connector_set_mode(
        &self,
        connector: Connector,
        mode: WireMode,
    ) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        connector.connector.connector.set_mode(backend::Mode {
            width: mode.width,
            height: mode.height,
            refresh_rate_millihz: mode.refresh_millihz,
        });
        Ok(())
    }

    fn handle_connector_modes(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        self.respond(Response::ConnectorModes {
            modes: connector
                .global
                .modes
                .iter()
                .map(|m| WireMode {
                    width: m.width,
                    height: m.height,
                    refresh_millihz: m.refresh_rate_millihz,
                })
                .collect(),
        });
        Ok(())
    }

    fn handle_connector_name(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_connector(connector)?;
        self.respond(Response::GetConnectorName {
            name: connector.name.clone(),
        });
        Ok(())
    }

    fn handle_connector_model(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        self.respond(Response::GetConnectorModel {
            model: connector.monitor_info.output_id.model.clone(),
        });
        Ok(())
    }

    fn handle_connector_manufacturer(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        self.respond(Response::GetConnectorManufacturer {
            manufacturer: connector.monitor_info.output_id.manufacturer.clone(),
        });
        Ok(())
    }

    fn handle_connector_serial_number(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        self.respond(Response::GetConnectorSerialNumber {
            serial_number: connector.monitor_info.output_id.serial_number.clone(),
        });
        Ok(())
    }

    fn handle_set_cursor_size(&self, seat: Seat, size: i32) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        if size < 0 {
            return Err(CphError::NegativeCursorSize);
        }
        seat.cursor_group().set_cursor_size(size as _);
        Ok(())
    }

    fn handle_disable_pointer_constraint(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.disable_pointer_constraint();
        Ok(())
    }

    fn handle_set_use_hardware_cursor(
        &self,
        seat: Seat,
        use_hardware_cursor: bool,
    ) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.cursor_group().set_hardware_cursor(use_hardware_cursor);
        self.state.refresh_hardware_cursors();
        Ok(())
    }

    fn handle_connector_size(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        let pos = connector.global.pos.get();
        self.respond(Response::ConnectorSize {
            width: pos.width(),
            height: pos.height(),
        });
        Ok(())
    }

    fn handle_connector_get_scale(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        self.respond(Response::ConnectorGetScale {
            scale: connector.global.persistent.scale.get().to_f64(),
        });
        Ok(())
    }

    fn handle_connector_set_scale(&self, connector: Connector, scale: f64) -> Result<(), CphError> {
        if scale < 0.1 {
            return Err(CphError::ScaleTooSmall(scale));
        }
        if scale > 1000.0 {
            return Err(CphError::ScaleTooLarge(scale));
        }
        let scale = Scale::from_f64(scale);
        let connector = self.get_output_node(connector)?;
        connector.set_preferred_scale(scale);
        Ok(())
    }

    fn handle_connector_set_format(
        &self,
        connector: Connector,
        format: ConfigFormat,
    ) -> Result<(), CphError> {
        let Some(&format) = config_formats().get(&format) else {
            return Err(CphError::UnknownFormat(format));
        };
        let connector = self.get_connector(connector)?;
        connector.connector.set_fb_format(format);
        Ok(())
    }

    fn handle_connector_set_colors(
        &self,
        connector: Connector,
        color_space: ColorSpace,
        transfer_function: ConfigTransferFunction,
    ) -> Result<(), CphError> {
        let bcs = match color_space {
            ColorSpace::DEFAULT => BackendColorSpace::Default,
            ColorSpace::BT2020 => BackendColorSpace::Bt2020,
            _ => return Err(CphError::UnknownColorSpace(color_space)),
        };
        let btf = match transfer_function {
            ConfigTransferFunction::DEFAULT => BackendTransferFunction::Default,
            ConfigTransferFunction::PQ => BackendTransferFunction::Pq,
            _ => return Err(CphError::UnknownTransferFunction(transfer_function)),
        };
        let connector = self.get_connector(connector)?;
        connector.connector.set_colors(bcs, btf);
        Ok(())
    }

    fn handle_connector_set_brightness(
        &self,
        connector: Connector,
        brightness: Option<f64>,
    ) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        connector.set_brightness(brightness);
        Ok(())
    }

    fn handle_set_float_above_fullscreen(&self, above: bool) {
        self.state.float_above_fullscreen.set(above);
        for seat in self.state.globals.seats.lock().values() {
            seat.emulate_cursor_moved();
            seat.trigger_tree_changed(false);
        }
        self.state.root.update_visible(&self.state);
    }

    fn handle_get_float_above_fullscreen(&self) {
        self.respond(Response::GetFloatAboveFullscreen {
            above: self.state.float_above_fullscreen.get(),
        });
    }

    fn handle_set_show_float_pin_icon(&self, show: bool) {
        self.state.show_pin_icon.set(show);
        for stacked in self.state.root.stacked.iter() {
            if let Some(float) = stacked.deref().clone().node_into_float() {
                float.schedule_render_titles();
            }
        }
    }

    fn handle_get_seat_float_pinned(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetFloatPinned {
            pinned: seat.pinned(),
        });
        Ok(())
    }

    fn handle_set_seat_float_pinned(&self, seat: Seat, pinned: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_pinned(pinned);
        Ok(())
    }

    fn handle_get_window_float_pinned(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        self.respond(Response::GetWindowFloatPinned {
            pinned: window.tl_pinned(),
        });
        Ok(())
    }

    fn handle_set_window_float_pinned(&self, window: Window, pinned: bool) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        window.tl_set_pinned(true, pinned);
        Ok(())
    }

    fn handle_set_vrr_mode(
        &self,
        connector: Option<Connector>,
        mode: ConfigVrrMode,
    ) -> Result<(), CphError> {
        let Some(mode) = VrrMode::from_config(mode) else {
            return Err(CphError::UnknownVrrMode(mode));
        };
        match connector {
            Some(c) => {
                let connector = self.get_output_node(c)?;
                connector.global.persistent.vrr_mode.set(mode);
                connector.update_presentation_type();
            }
            _ => self.state.default_vrr_mode.set(mode),
        }
        Ok(())
    }

    fn handle_set_vrr_cursor_hz(
        &self,
        connector: Option<Connector>,
        hz: f64,
    ) -> Result<(), CphError> {
        match connector {
            Some(c) => {
                let connector = self.get_output_node(c)?;
                connector.schedule.set_cursor_hz(hz);
            }
            _ => {
                let Some((hz, _)) = map_cursor_hz(hz) else {
                    return Err(CphError::InvalidCursorHz(hz));
                };
                self.state.default_vrr_cursor_hz.set(hz)
            }
        }
        Ok(())
    }

    fn handle_set_tearing_mode(
        &self,
        connector: Option<Connector>,
        mode: ConfigTearingMode,
    ) -> Result<(), CphError> {
        let Some(mode) = TearingMode::from_config(mode) else {
            return Err(CphError::UnknownTearingMode(mode));
        };
        match connector {
            Some(c) => {
                let connector = self.get_output_node(c)?;
                connector.global.persistent.tearing_mode.set(mode);
                connector.update_presentation_type();
            }
            _ => self.state.default_tearing_mode.set(mode),
        }
        Ok(())
    }

    fn handle_connector_set_transform(
        &self,
        connector: Connector,
        transform: Transform,
    ) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        connector.update_transform(transform);
        Ok(())
    }

    fn handle_connector_set_position(
        &self,
        connector: Connector,
        x: i32,
        y: i32,
    ) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        if x < 0 || y < 0 || x > MAX_EXTENTS || y > MAX_EXTENTS {
            return Err(CphError::InvalidConnectorPosition(x, y));
        }
        connector.set_position(x, y);
        Ok(())
    }

    fn handle_connector_get_position(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output_node(connector)?;
        let (x, y) = connector.global.pos.get().position();
        self.respond(Response::ConnectorGetPosition { x, y });
        Ok(())
    }

    fn handle_connector_set_enabled(
        &self,
        connector: Connector,
        enabled: bool,
    ) -> Result<(), CphError> {
        let connector = self.get_connector(connector)?;
        connector.connector.set_enabled(enabled);
        Ok(())
    }

    fn handle_get_connector(
        &self,
        ty: jay_config::video::connector_type::ConnectorType,
        idx: u32,
    ) -> Result<(), CphError> {
        let connectors = self.state.connectors.lock();
        let connector = 'get_connector: {
            for connector in connectors.values() {
                let kid = connector.connector.kernel_id();
                if ty == kid.ty.to_config() && idx == kid.idx {
                    break 'get_connector Connector(connector.connector.id().raw() as _);
                }
            }
            Connector(0)
        };
        self.respond(Response::GetConnector { connector });
        Ok(())
    }

    fn handle_get_connector_active_workspace(&self, connector: Connector) -> Result<(), CphError> {
        let output = self.get_output_node(connector)?;
        let workspace = output
            .workspace
            .get()
            .map_or(Workspace(0), |ws| self.get_workspace_by_name(&ws.name));
        self.respond(Response::GetConnectorActiveWorkspace { workspace });
        Ok(())
    }

    fn handle_get_connector_workspaces(&self, connector: Connector) -> Result<(), CphError> {
        let output = self.get_output_node(connector)?;
        let workspaces = output
            .workspaces
            .iter()
            .map(|ws| self.get_workspace_by_name(&ws.name))
            .collect::<Vec<_>>();
        self.respond(Response::GetConnectorWorkspaces { workspaces });
        Ok(())
    }

    fn handle_has_capability(&self, device: InputDevice, cap: Capability) -> Result<(), CphError> {
        let dev = self.get_device_handler_data(device)?;
        let mut is_unknown = false;
        let has_cap = 'has_cap: {
            let cap = match cap {
                CAP_KEYBOARD => InputDeviceCapability::Keyboard,
                CAP_POINTER => InputDeviceCapability::Pointer,
                CAP_TOUCH => InputDeviceCapability::Touch,
                CAP_TABLET_TOOL => InputDeviceCapability::TabletTool,
                CAP_TABLET_PAD => InputDeviceCapability::TabletPad,
                CAP_GESTURE => InputDeviceCapability::Gesture,
                CAP_SWITCH => InputDeviceCapability::Switch,
                _ => {
                    is_unknown = true;
                    break 'has_cap false;
                }
            };
            dev.device.has_capability(cap)
        };
        self.respond(Response::HasCapability { has: has_cap });
        if is_unknown {
            Err(CphError::UnknownCapability(cap))
        } else {
            Ok(())
        }
    }

    fn handle_get_seat_mono(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetMono {
            mono: seat.get_mono().unwrap_or(false),
        });
        Ok(())
    }

    fn handle_set_seat_mono(&self, seat: Seat, mono: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_mono(mono);
        Ok(())
    }

    fn handle_get_window_mono(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        self.respond(Response::GetWindowMono {
            mono: toplevel_parent_container(&*window)
                .map(|c| c.mono_child.is_some())
                .unwrap_or(false),
        });
        Ok(())
    }

    fn handle_set_window_mono(&self, window: Window, mono: bool) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        if let Some(c) = toplevel_parent_container(&*window) {
            c.set_mono(mono.then_some(window.as_ref()));
        }
        Ok(())
    }

    fn handle_get_seat_split(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetSplit {
            axis: seat
                .get_split()
                .unwrap_or(ContainerSplit::Horizontal)
                .into(),
        });
        Ok(())
    }

    fn handle_set_seat_split(&self, seat: Seat, axis: Axis) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_split(axis.into());
        Ok(())
    }

    fn handle_get_window_split(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        self.respond(Response::GetWindowSplit {
            axis: toplevel_parent_container(&*window)
                .map(|c| c.split.get())
                .unwrap_or(ContainerSplit::Horizontal)
                .into(),
        });
        Ok(())
    }

    fn handle_set_window_split(&self, window: Window, axis: Axis) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        if let Some(c) = toplevel_parent_container(&*window) {
            c.set_split(axis.into());
        }
        Ok(())
    }

    fn handle_add_shortcut(
        &self,
        seat: Seat,
        mod_mask: Modifiers,
        mods: Modifiers,
        sym: KeySym,
    ) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.add_shortcut(mod_mask, mods, sym);
        Ok(())
    }

    fn handle_remove_shortcut(
        &self,
        seat: Seat,
        mods: Modifiers,
        sym: KeySym,
    ) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.remove_shortcut(mods, sym);
        Ok(())
    }

    fn handle_get_input_devices(&self, seat: Option<Seat>) {
        let id = seat.map(|s| SeatId::from_raw(s.0 as _));
        let matches = |dhd: &DeviceHandlerData| {
            let id = match id {
                Some(id) => id,
                _ => return true,
            };
            if let Some(seat) = dhd.seat.get() {
                return seat.id() == id;
            }
            false
        };
        let mut res = vec![];
        {
            let devs = self.state.input_device_handlers.borrow_mut();
            for dev in devs.values() {
                if matches(&dev.data) {
                    res.push(InputDevice(dev.id.raw() as _));
                }
            }
        }
        self.respond(Response::GetInputDevices { devices: res });
    }

    fn handle_get_seats(&self) {
        let seats = {
            let seats = self.state.globals.seats.lock();
            seats
                .values()
                .map(|seat| Seat::from_raw(seat.id().raw() as _))
                .collect()
        };
        self.respond(Response::GetSeats { seats });
    }

    fn handle_run(
        &self,
        prog: &str,
        args: Vec<String>,
        env: Vec<(String, String)>,
        fds: Vec<(i32, i32)>,
    ) -> Result<(), CphError> {
        let fds: Vec<_> = fds
            .into_iter()
            .map(|(a, b)| (a, Rc::new(OwnedFd::new(b))))
            .collect();
        let forker = match self.state.forker.get() {
            Some(f) => f,
            _ => return Err(CphError::NoForker),
        };
        let env = env.into_iter().map(|(k, v)| (k, Some(v))).collect();
        forker.spawn(prog.to_string(), args, env, fds);
        Ok(())
    }

    fn handle_set_log_level(&self, level: LogLevel) {
        let level = match level {
            LogLevel::Error => Level::Error,
            LogLevel::Warn => Level::Warn,
            LogLevel::Info => Level::Info,
            LogLevel::Debug => Level::Debug,
            LogLevel::Trace => Level::Trace,
        };
        if let Some(logger) = &self.state.logger {
            logger.set_level(level);
        }
    }

    fn handle_grab(&self, kb: InputDevice, grab: bool) -> Result<(), CphError> {
        let kb = self.get_kb(kb)?;
        kb.grab(grab);
        Ok(())
    }

    fn handle_create_seat_split(&self, seat: Seat, axis: Axis) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.create_split(axis.into());
        Ok(())
    }

    fn handle_create_window_split(&self, window: Window, axis: Axis) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        toplevel_create_split(&self.state, window, axis.into());
        Ok(())
    }

    fn handle_focus_seat_parent(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.focus_parent();
        Ok(())
    }

    fn handle_quit(&self) {
        log::info!("Quitting");
        self.state.ring.stop();
    }

    fn handle_switch_to(&self, vtnr: u32) {
        self.state.backend.get().switch_to(vtnr);
    }

    fn handle_get_seat_floating(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetFloating {
            floating: seat.get_floating().unwrap_or(false),
        });
        Ok(())
    }

    fn handle_set_seat_floating(&self, seat: Seat, floating: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_floating(floating);
        Ok(())
    }

    fn handle_get_window_floating(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        self.respond(Response::GetWindowFloating {
            floating: window.tl_data().is_floating.get(),
        });
        Ok(())
    }

    fn handle_set_window_floating(&self, window: Window, floating: bool) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        toplevel_set_floating(&self.state, window, floating);
        Ok(())
    }

    fn handle_add_pollable(self: &Rc<Self>, fd: i32) -> Result<(), CphError> {
        let fd = match fcntl_dupfd_cloexec(fd, 0) {
            Ok(fd) => Rc::new(fd),
            Err(e) => {
                let err = format!(
                    "Could not invoke F_DUPFD_CLOEXEC: {}",
                    ErrorFmt(OsError::from(e))
                );
                log::error!("{}", err);
                self.respond(Response::AddPollable { id: Err(err) });
                return Ok(());
            }
        };
        let id = self.pollable_id.fetch_add(1);
        let id = PollableId(id);
        let create = |writable: bool, events: c::c_short| {
            let event = Rc::new(AsyncEvent::default());
            let slf = self.clone();
            let trigger = event.clone();
            let fd = fd.clone();
            let future = self.state.eng.spawn("config fd poller", async move {
                loop {
                    trigger.triggered().await;
                    let res = slf.state.ring.poll(&fd, events).await.merge();
                    if let Err(e) = &res {
                        log::warn!("Could not poll fd: {}", ErrorFmt(e));
                    }
                    let res = res.map_err(|e| ErrorFmt(e).to_string()).map(drop);
                    slf.send(&ServerMessage::InterestReady { id, writable, res });
                }
            });
            (event, future)
        };
        let (read_trigger, _read_future) = create(false, c::POLLIN);
        let (write_trigger, _write_future) = create(true, c::POLLOUT);
        self.pollables.set(
            id,
            Rc::new(Pollable {
                write_trigger,
                _write_future,
                read_trigger,
                _read_future,
            }),
        );
        self.respond(Response::AddPollable { id: Ok(id) });
        Ok(())
    }

    fn handle_remove_pollable(self: &Rc<Self>, id: PollableId) {
        self.pollables.remove(&id);
    }

    fn handle_add_interest(
        self: &Rc<Self>,
        id: PollableId,
        writable: bool,
    ) -> Result<(), CphError> {
        let Some(pollable) = self.pollables.get(&id) else {
            return Err(CphError::PollableDoesNotExist);
        };
        let trigger = match writable {
            true => &pollable.write_trigger,
            false => &pollable.read_trigger,
        };
        trigger.trigger();
        Ok(())
    }

    fn tl_to_window(&self, tl: &dyn ToplevelNode) -> Window {
        self.tl_id_to_window(tl.tl_data().identifier.get())
    }

    fn tl_id_to_window(&self, tl: ToplevelIdentifier) -> Window {
        if let Some(win) = self.windows_from_tl_id.get(&tl) {
            return win;
        }
        let id = Window(self.window_ids.fetch_add(1));
        self.windows_from_tl_id.set(tl, id);
        self.windows_to_tl_id.set(id, tl);
        id
    }

    fn get_window(&self, window: Window) -> Result<Rc<dyn ToplevelNode>, CphError> {
        self.windows_to_tl_id
            .get(&window)
            .and_then(|id| self.state.toplevels.get(&id))
            .and_then(|tl| tl.upgrade())
            .ok_or(CphError::WindowDoesNotExist(window))
    }

    fn spaces_change(&self) {
        struct V;
        impl NodeVisitorBase for V {
            fn visit_output(&mut self, node: &Rc<OutputNode>) {
                node.on_spaces_changed();
                node.node_visit_children(self);
            }
            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.on_spaces_changed();
                node.node_visit_children(self);
            }
            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                node.on_spaces_changed();
                node.node_visit_children(self);
            }
        }
        self.state.root.clone().node_visit(&mut V);
        self.state.damage(self.state.root.extents.get());
        self.state.icons.update_sizes(&self.state);
    }

    fn colors_changed(&self) {
        struct V;
        impl NodeVisitorBase for V {
            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.on_colors_changed();
                node.node_visit_children(self);
            }
            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                node.on_colors_changed();
                node.node_visit_children(self);
            }
        }
        self.state.root.clone().node_visit(&mut V);
        self.state.damage(self.state.root.extents.get());
        self.state.icons.clear();
    }

    fn get_sized(&self, sized: Resizable) -> Result<ThemeSized, CphError> {
        use jay_config::theme::sized::*;
        let sized = match sized {
            TITLE_HEIGHT => ThemeSized::title_height,
            BORDER_WIDTH => ThemeSized::border_width,
            _ => return Err(CphError::UnknownSized(sized.0)),
        };
        Ok(sized)
    }

    fn handle_get_size(&self, sized: Resizable) -> Result<(), CphError> {
        let sized = self.get_sized(sized)?;
        let size = sized.field(&self.state.theme).get();
        self.respond(Response::GetSize { size });
        Ok(())
    }

    fn handle_set_size(&self, sized: Resizable, size: i32) -> Result<(), CphError> {
        let sized = self.get_sized(sized)?;
        if size < sized.min() {
            return Err(CphError::InvalidSize(size, sized));
        }
        if size > sized.max() {
            return Err(CphError::InvalidSize(size, sized));
        }
        sized.field(&self.state.theme).set(size);
        self.spaces_change();
        Ok(())
    }

    fn handle_reset_colors(&self) {
        self.state.theme.colors.reset();
        self.colors_changed();
    }

    fn handle_reset_sizes(&self) {
        self.state.theme.sizes.reset();
        self.spaces_change();
    }

    fn handle_reset_font(&self) {
        self.state
            .theme
            .font
            .set(self.state.theme.default_font.clone());
    }

    fn handle_set_font(&self, font: &str) {
        self.state.theme.font.set(Arc::new(font.to_string()));
    }

    fn handle_get_font(&self) {
        let font = self.state.theme.font.get().to_string();
        self.respond(Response::GetFont { font });
    }

    fn get_color(&self, colorable: Colorable) -> Result<&Cell<Color>, CphError> {
        let colors = &self.state.theme.colors;
        use jay_config::theme::colors::*;
        let colorable = match colorable {
            UNFOCUSED_TITLE_BACKGROUND_COLOR => &colors.unfocused_title_background,
            FOCUSED_TITLE_BACKGROUND_COLOR => &colors.focused_title_background,
            CAPTURED_UNFOCUSED_TITLE_BACKGROUND_COLOR => {
                &colors.captured_unfocused_title_background
            }
            CAPTURED_FOCUSED_TITLE_BACKGROUND_COLOR => &colors.captured_focused_title_background,
            FOCUSED_INACTIVE_TITLE_BACKGROUND_COLOR => &colors.focused_inactive_title_background,
            BACKGROUND_COLOR => &colors.background,
            BAR_BACKGROUND_COLOR => &colors.bar_background,
            SEPARATOR_COLOR => &colors.separator,
            BORDER_COLOR => &colors.border,
            UNFOCUSED_TITLE_TEXT_COLOR => &colors.unfocused_title_text,
            FOCUSED_TITLE_TEXT_COLOR => &colors.focused_title_text,
            FOCUSED_INACTIVE_TITLE_TEXT_COLOR => &colors.focused_inactive_title_text,
            BAR_STATUS_TEXT_COLOR => &colors.bar_text,
            ATTENTION_REQUESTED_BACKGROUND_COLOR => &colors.attention_requested_background,
            HIGHLIGHT_COLOR => &colors.highlight,
            _ => return Err(CphError::UnknownColor(colorable.0)),
        };
        Ok(colorable)
    }

    fn handle_get_color(&self, colorable: Colorable) -> Result<(), CphError> {
        let color = self.get_color(colorable)?.get();
        let [r, g, b, a] = color.to_array(TransferFunction::Srgb);
        let color = jay_config::theme::Color::new_f32_premultiplied(r, g, b, a);
        self.respond(Response::GetColor { color });
        Ok(())
    }

    fn handle_set_color(
        &self,
        colorable: Colorable,
        color: jay_config::theme::Color,
    ) -> Result<(), CphError> {
        self.get_color(colorable)?.set(color.into());
        self.colors_changed();
        Ok(())
    }

    fn handle_destroy_keymap(&self, keymap: Keymap) {
        self.keymaps.remove(&keymap);
    }

    fn get_client(&self, client: ConfigClient) -> Result<Rc<Client>, CphError> {
        self.state
            .clients
            .get(ClientId::from_raw(client.0))
            .ok()
            .ok_or(CphError::ClientDoesNotExist(client))
    }

    fn handle_get_clients(&self) {
        let mut clients = vec![];
        for client in self.state.clients.clients.borrow().values() {
            clients.push(ConfigClient(client.data.id.raw()));
        }
        self.respond(Response::GetClients { clients });
    }

    fn handle_client_exists(&self, client: ConfigClient) {
        self.respond(Response::ClientExists {
            exists: self.get_client(client).is_ok(),
        });
    }

    fn handle_client_is_xwayland(&self, client: ConfigClient) -> Result<(), CphError> {
        self.respond(Response::ClientIsXwayland {
            is_xwayland: self.get_client(client)?.is_xwayland,
        });
        Ok(())
    }

    fn handle_client_kill(&self, client: ConfigClient) {
        self.state.clients.kill(ClientId::from_raw(client.0));
    }

    fn handle_get_workspace_window(&self, ws: Workspace) -> Result<(), CphError> {
        let window = self
            .get_existing_workspace(ws)?
            .and_then(|ws| ws.container.get())
            .map(|c| self.tl_to_window(&*c))
            .unwrap_or(Window(0));
        self.respond(Response::GetWorkspaceWindow { window });
        Ok(())
    }

    fn handle_get_seat_keyboard_window(&self, seat: Seat) -> Result<(), CphError> {
        let window = self
            .get_seat(seat)?
            .get_keyboard_node()
            .node_toplevel()
            .map(|tl| self.tl_to_window(&*tl))
            .unwrap_or(Window(0));
        self.respond(Response::GetSeatKeyboardWindow { window });
        Ok(())
    }

    fn handle_seat_focus_window(&self, seat: Seat, window_id: Window) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let window = self.get_window(window_id)?;
        if !window.node_visible() {
            return Err(CphError::WindowNotVisible(window_id));
        }
        seat.focus_toplevel(window);
        Ok(())
    }

    fn handle_get_window_title(&self, window: Window) -> Result<(), CphError> {
        let title = self.get_window(window)?.tl_data().title.borrow().clone();
        self.respond(Response::GetWindowTitle { title });
        Ok(())
    }

    fn handle_get_window_type(&self, window: Window) -> Result<(), CphError> {
        let kind = self.get_window(window)?.tl_data().kind.to_window_type();
        self.respond(Response::GetWindowType { kind });
        Ok(())
    }

    fn handle_window_exists(&self, window: Window) {
        self.respond(Response::WindowExists {
            exists: self.get_window(window).is_ok(),
        });
    }

    fn handle_get_window_id(&self, window: Window) -> Result<(), CphError> {
        let id = self
            .get_window(window)?
            .tl_data()
            .identifier
            .get()
            .to_string();
        self.respond(Response::GetWindowId { id: id.to_string() });
        Ok(())
    }

    fn handle_get_window_is_visible(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        self.respond(Response::GetWindowIsVisible {
            visible: window.node_visible(),
        });
        Ok(())
    }

    fn handle_get_window_client(&self, window: Window) -> Result<(), CphError> {
        let window = self.get_window(window)?;
        self.respond(Response::GetWindowClient {
            client: window
                .tl_data()
                .client
                .as_ref()
                .map(|c| ConfigClient(c.id.raw()))
                .unwrap_or(ConfigClient(0)),
        });
        Ok(())
    }

    fn handle_get_window_parent(&self, window: Window) -> Result<(), CphError> {
        let window = self
            .get_window(window)?
            .tl_data()
            .parent
            .get()
            .and_then(|tl| tl.node_into_toplevel())
            .map(|tl| self.tl_to_window(&*tl))
            .unwrap_or(Window(0));
        self.respond(Response::GetWindowParent { window });
        Ok(())
    }

    fn handle_get_window_workspace(&self, window: Window) -> Result<(), CphError> {
        let workspace = self
            .get_window(window)?
            .tl_data()
            .workspace
            .get()
            .map(|ws| self.get_workspace_by_name(&ws.name))
            .unwrap_or(Workspace(0));
        self.respond(Response::GetWindowWorkspace { workspace });
        Ok(())
    }

    fn handle_get_window_children(&self, window: Window) -> Result<(), CphError> {
        let mut windows = vec![];
        if let Some(c) = self.get_window(window)?.node_into_container() {
            for c in c.children.iter() {
                windows.push(self.tl_to_window(&*c.node));
            }
        }
        self.respond(Response::GetWindowChildren { windows });
        Ok(())
    }

    pub fn handle_request(self: &Rc<Self>, msg: &[u8]) {
        if let Err(e) = self.handle_request_(msg) {
            log::error!("Could not handle client request: {}", ErrorFmt(e));
        }
    }

    fn handle_request_(self: &Rc<Self>, msg: &[u8]) -> Result<(), CphError> {
        let request = match bincode_ops().deserialize::<ClientMessage>(msg) {
            Ok(msg) => msg,
            Err(e) => return Err(CphError::ParsingFailed(e)),
        };
        match request {
            ClientMessage::Log {
                level,
                msg,
                file,
                line,
            } => self.handle_log_request(level, msg, file, line),
            ClientMessage::GetSeat { name } => self.handle_get_seat(name),
            ClientMessage::ParseKeymap { keymap } => {
                self.handle_parse_keymap(keymap).wrn("parse_keymap")?
            }
            ClientMessage::SeatSetKeymap { seat, keymap } => {
                self.handle_set_keymap(seat, keymap).wrn("set_keymap")?
            }
            ClientMessage::SeatGetRepeatRate { seat } => {
                self.handle_get_repeat_rate(seat).wrn("get_repeat_rate")?
            }
            ClientMessage::SeatSetRepeatRate { seat, rate, delay } => self
                .handle_set_repeat_rate(seat, rate, delay)
                .wrn("set_repeat_rate")?,
            ClientMessage::SetSeat { device, seat } => {
                self.handle_set_seat(device, seat).wrn("set_seat")?
            }
            ClientMessage::GetSeatMono { seat } => {
                self.handle_get_seat_mono(seat).wrn("get_seat_mono")?
            }
            ClientMessage::SetSeatMono { seat, mono } => {
                self.handle_set_seat_mono(seat, mono).wrn("set_seat_mono")?
            }
            ClientMessage::GetSeatSplit { seat } => {
                self.handle_get_seat_split(seat).wrn("get_seat_split")?
            }
            ClientMessage::SetSeatSplit { seat, axis } => self
                .handle_set_seat_split(seat, axis)
                .wrn("set_seat_split")?,
            ClientMessage::AddShortcut { seat, mods, sym } => self
                .handle_add_shortcut(seat, Modifiers(!0), mods, sym)
                .wrn("add_shortcut")?,
            ClientMessage::RemoveShortcut { seat, mods, sym } => self
                .handle_remove_shortcut(seat, mods, sym)
                .wrn("remove_shortcut")?,
            ClientMessage::SeatFocus { seat, direction } => {
                self.handle_seat_focus(seat, direction).wrn("seat_focus")?
            }
            ClientMessage::SeatMove { seat, direction } => {
                self.handle_seat_move(seat, direction).wrn("seat_move")?
            }
            ClientMessage::GetInputDevices { seat } => self.handle_get_input_devices(seat),
            ClientMessage::GetSeats => self.handle_get_seats(),
            ClientMessage::RemoveSeat { .. } => {}
            ClientMessage::Run { prog, args, env } => {
                self.handle_run(prog, args, env, vec![]).wrn("run")?
            }
            ClientMessage::GrabKb { kb, grab } => self.handle_grab(kb, grab).wrn("grab")?,
            ClientMessage::SetColor { colorable, color } => {
                self.handle_set_color(colorable, color).wrn("set_color")?
            }
            ClientMessage::GetColor { colorable } => {
                self.handle_get_color(colorable).wrn("get_color")?
            }
            ClientMessage::CreateSeatSplit { seat, axis } => self
                .handle_create_seat_split(seat, axis)
                .wrn("create_seat_split")?,
            ClientMessage::FocusSeatParent { seat } => self
                .handle_focus_seat_parent(seat)
                .wrn("focus_seat_parent")?,
            ClientMessage::GetSeatFloating { seat } => self
                .handle_get_seat_floating(seat)
                .wrn("get_seat_floating")?,
            ClientMessage::SetSeatFloating { seat, floating } => self
                .handle_set_seat_floating(seat, floating)
                .wrn("set_seat_floating")?,
            ClientMessage::Quit => self.handle_quit(),
            ClientMessage::SwitchTo { vtnr } => self.handle_switch_to(vtnr),
            ClientMessage::HasCapability { device, cap } => self
                .handle_has_capability(device, cap)
                .wrn("has_capability")?,
            ClientMessage::SetLeftHanded {
                device,
                left_handed,
            } => self
                .handle_set_left_handed(device, left_handed)
                .wrn("set_left_handed")?,
            ClientMessage::SetAccelProfile { device, profile } => self
                .handle_set_accel_profile(device, profile)
                .wrn("set_accel_profile")?,
            ClientMessage::SetAccelSpeed { device, speed } => self
                .handle_set_accel_speed(device, speed)
                .wrn("set_accel_speed")?,
            ClientMessage::SetTransformMatrix { device, matrix } => self
                .handle_set_transform_matrix(device, matrix)
                .wrn("set_transform_matrix")?,
            ClientMessage::GetDeviceName { device } => {
                self.handle_get_device_name(device).wrn("get_device_name")?
            }
            ClientMessage::GetWorkspace { name } => self.handle_get_workspace(name),
            ClientMessage::ShowWorkspace { seat, workspace } => self
                .handle_show_workspace(seat, workspace)
                .wrn("show_workspace")?,
            ClientMessage::SetSeatWorkspace { seat, workspace } => self
                .handle_set_seat_workspace(seat, workspace)
                .wrn("set_seat_workspace")?,
            ClientMessage::GetConnector { ty, idx } => {
                self.handle_get_connector(ty, idx).wrn("get_connector")?
            }
            ClientMessage::ConnectorConnected { connector } => self
                .handle_connector_connected(connector)
                .wrn("connector_connected")?,
            ClientMessage::ConnectorType { connector } => self
                .handle_connector_type(connector)
                .wrn("connector_type")?,
            ClientMessage::ConnectorMode { connector } => self
                .handle_connector_mode(connector)
                .wrn("connector_mode")?,
            ClientMessage::ConnectorSetPosition { connector, x, y } => self
                .handle_connector_set_position(connector, x, y)
                .wrn("connector_set_position")?,
            ClientMessage::ConnectorSetEnabled { connector, enabled } => self
                .handle_connector_set_enabled(connector, enabled)
                .wrn("connector_set_enabled")?,
            ClientMessage::SeatClose { seat } => self.handle_seat_close(seat).wrn("seat_close")?,
            ClientMessage::SetStatus { status } => self.handle_set_status(status),
            ClientMessage::GetTimer { name } => self.handle_get_timer(name).wrn("get_timer")?,
            ClientMessage::RemoveTimer { timer } => {
                self.handle_remove_timer(timer).wrn("remove_timer")?
            }
            ClientMessage::ProgramTimer {
                timer,
                initial,
                periodic,
            } => self
                .handle_program_timer(timer, initial, periodic)
                .wrn("program_timer")?,
            ClientMessage::SetEnv { key, val } => self.handle_set_env(key, val),
            ClientMessage::SetSeatFullscreen { seat, fullscreen } => self
                .handle_set_seat_fullscreen(seat, fullscreen)
                .wrn("set_seat_fullscreen")?,
            ClientMessage::GetSeatFullscreen { seat } => self
                .handle_get_seat_fullscreen(seat)
                .wrn("get_seat_fullscreen")?,
            ClientMessage::Reload => self.handle_reload(),
            ClientMessage::GetDeviceConnectors { device } => self
                .handle_get_connectors(Some(device), false)
                .wrn("get_device_connectors")?,
            ClientMessage::GetDrmDeviceSyspath { device } => self
                .handle_get_drm_device_syspath(device)
                .wrn("get_drm_device_syspath")?,
            ClientMessage::GetDrmDeviceVendor { device } => self
                .handle_get_drm_device_vendor(device)
                .wrn("get_drm_device_vendor")?,
            ClientMessage::GetDrmDeviceModel { device } => self
                .handle_get_drm_device_model(device)
                .wrn("get_drm_device_model")?,
            ClientMessage::GetDrmDevices => self.handle_get_drm_devices(),
            ClientMessage::GetDrmDevicePciId { device } => self
                .handle_get_drm_device_pci_id(device)
                .wrn("get_drm_device_pci_id")?,
            ClientMessage::ResetColors => self.handle_reset_colors(),
            ClientMessage::ResetSizes => self.handle_reset_sizes(),
            ClientMessage::GetSize { sized } => self.handle_get_size(sized).wrn("get_size")?,
            ClientMessage::SetSize { sized, size } => {
                self.handle_set_size(sized, size).wrn("set_size")?
            }
            ClientMessage::ResetFont => self.handle_reset_font(),
            ClientMessage::GetFont => self.handle_get_font(),
            ClientMessage::SetFont { font } => self.handle_set_font(font),
            ClientMessage::SetPxPerWheelScroll { device, px } => self
                .handle_set_px_per_wheel_scroll(device, px)
                .wrn("set_px_per_wheel_scroll")?,
            ClientMessage::ConnectorSetScale { connector, scale } => self
                .handle_connector_set_scale(connector, scale)
                .wrn("connector_set_scale")?,
            ClientMessage::ConnectorGetScale { connector } => self
                .handle_connector_get_scale(connector)
                .wrn("connector_get_scale")?,
            ClientMessage::ConnectorSize { connector } => self
                .handle_connector_size(connector)
                .wrn("connector_size")?,
            ClientMessage::SetCursorSize { seat, size } => self
                .handle_set_cursor_size(seat, size)
                .wrn("set_cursor_size")?,
            ClientMessage::SetTapEnabled { device, enabled } => self
                .handle_set_tap_enabled(device, enabled)
                .wrn("set_tap_enabled")?,
            ClientMessage::SetDragEnabled { device, enabled } => self
                .handle_set_drag_enabled(device, enabled)
                .wrn("set_drag_enabled")?,
            ClientMessage::SetDragLockEnabled { device, enabled } => self
                .handle_set_drag_lock_enabled(device, enabled)
                .wrn("set_drag_lock_enabled")?,
            ClientMessage::SetUseHardwareCursor {
                seat,
                use_hardware_cursor,
            } => self
                .handle_set_use_hardware_cursor(seat, use_hardware_cursor)
                .wrn("set_use_hardware_cursor")?,
            ClientMessage::DisablePointerConstraint { seat } => self
                .handle_disable_pointer_constraint(seat)
                .wrn("disable_pointer_constraint")?,
            ClientMessage::MakeRenderDevice { device } => self
                .handle_make_render_device(device)
                .wrn("make_render_device")?,
            ClientMessage::GetSeatWorkspace { seat } => self
                .handle_get_seat_workspace(seat)
                .wrn("get_seat_workspace")?,
            ClientMessage::GetSeatKeyboardWorkspace { seat } => self
                .handle_get_seat_keyboard_workspace(seat)
                .wrn("get_seat_keyboard_workspace")?,
            ClientMessage::SetDefaultWorkspaceCapture { capture } => {
                self.handle_set_default_workspace_capture(capture)
            }
            ClientMessage::GetDefaultWorkspaceCapture => {
                self.handle_get_default_workspace_capture()
            }
            ClientMessage::SetWorkspaceCapture { workspace, capture } => self
                .handle_set_workspace_capture(workspace, capture)
                .wrn("set_workspace_capture")?,
            ClientMessage::GetWorkspaceCapture { workspace } => self
                .handle_get_workspace_capture(workspace)
                .wrn("get_workspace_capture")?,
            ClientMessage::SetNaturalScrollingEnabled { device, enabled } => self
                .handle_set_natural_scrolling_enabled(device, enabled)
                .wrn("set_natural_scrolling_enabled")?,
            ClientMessage::SetGfxApi { device, api } => {
                self.handle_set_gfx_api(device, api).wrn("set_gfx_api")?
            }
            ClientMessage::SetDirectScanoutEnabled { device, enabled } => self
                .handle_set_direct_scanout_enabled(device, enabled)
                .wrn("set_direct_scanout_enabled")?,
            ClientMessage::ConnectorSetTransform {
                connector,
                transform,
            } => self
                .handle_connector_set_transform(connector, transform)
                .wrn("connector_set_transform")?,
            ClientMessage::SetDoubleClickIntervalUsec { usec } => {
                self.handle_set_double_click_interval_usec(usec)
            }
            ClientMessage::SetDoubleClickDistance { dist } => {
                self.handle_set_double_click_distance(dist)
            }
            ClientMessage::ConnectorModes { connector } => self
                .handle_connector_modes(connector)
                .wrn("connector_modes")?,
            ClientMessage::ConnectorSetMode { connector, mode } => self
                .handle_connector_set_mode(connector, mode)
                .wrn("connector_set_mode")?,
            ClientMessage::AddPollable { fd } => {
                self.handle_add_pollable(fd).wrn("add_pollable")?
            }
            ClientMessage::RemovePollable { id } => self.handle_remove_pollable(id),
            ClientMessage::AddInterest { pollable, writable } => self
                .handle_add_interest(pollable, writable)
                .wrn("add_interest")?,
            ClientMessage::Run2 {
                prog,
                args,
                env,
                fds,
            } => self.handle_run(prog, args, env, fds).wrn("run")?,
            ClientMessage::DisableDefaultSeat => self.state.create_default_seat.set(false),
            ClientMessage::DestroyKeymap { keymap } => self.handle_destroy_keymap(keymap),
            ClientMessage::GetConnectorName { connector } => self
                .handle_connector_name(connector)
                .wrn("connector_name")?,
            ClientMessage::GetConnectorModel { connector } => self
                .handle_connector_model(connector)
                .wrn("connector_model")?,
            ClientMessage::GetConnectorManufacturer { connector } => self
                .handle_connector_manufacturer(connector)
                .wrn("connector_manufacturer")?,
            ClientMessage::GetConnectorSerialNumber { connector } => self
                .handle_connector_serial_number(connector)
                .wrn("connector_serial_number")?,
            ClientMessage::GetConnectors {
                device,
                connected_only,
            } => self
                .handle_get_connectors(device, connected_only)
                .wrn("get_connectors")?,
            ClientMessage::ConnectorGetPosition { connector } => self
                .handle_connector_get_position(connector)
                .wrn("connector_get_position")?,
            ClientMessage::GetConfigDir => self.handle_get_config_dir(),
            ClientMessage::GetWorkspaces => self.handle_get_workspaces(),
            ClientMessage::UnsetEnv { key } => self.handle_unset_env(key),
            ClientMessage::SetLogLevel { level } => self.handle_set_log_level(level),
            ClientMessage::GetDrmDeviceDevnode { device } => self
                .handle_get_drm_device_devnode(device)
                .wrn("get_drm_device_devnode")?,
            ClientMessage::GetInputDeviceSyspath { device } => self
                .handle_get_input_device_syspath(device)
                .wrn("get_input_device_syspath")?,
            ClientMessage::GetInputDeviceDevnode { device } => self
                .handle_get_input_device_devnode(device)
                .wrn("get_input_device_devnode")?,
            ClientMessage::SetIdle { timeout } => self.handle_set_idle(timeout),
            ClientMessage::MoveToOutput {
                workspace,
                connector,
            } => self
                .handle_move_to_output(workspace, connector)
                .wrn("move_to_output")?,
            ClientMessage::SetExplicitSyncEnabled { enabled } => {
                self.handle_set_explicit_sync_enabled(enabled)
            }
            ClientMessage::GetSocketPath => self.handle_get_socket_path(),
            ClientMessage::DeviceSetKeymap { device, keymap } => self
                .handle_set_device_keymap(device, keymap)
                .wrn("set_device_keymap")?,
            ClientMessage::SetForward { seat, forward } => {
                self.handle_set_forward(seat, forward).wrn("set_forward")?
            }
            ClientMessage::AddShortcut2 {
                seat,
                mod_mask,
                mods,
                sym,
            } => self
                .handle_add_shortcut(seat, mod_mask, mods, sym)
                .wrn("add_shortcut")?,
            ClientMessage::SetFocusFollowsMouseMode { seat, mode } => self
                .handle_set_focus_follows_mouse_mode(seat, mode)
                .wrn("set_focus_follows_mouse_mode")?,
            ClientMessage::SetInputDeviceConnector {
                input_device,
                connector,
            } => self
                .handle_set_input_device_connector(input_device, connector)
                .wrn("set_input_device_connector")?,
            ClientMessage::RemoveInputMapping { input_device } => self
                .handle_remove_input_mapping(input_device)
                .wrn("remove_input_mapping")?,
            ClientMessage::SetWindowManagementEnabled { seat, enabled } => self
                .handle_set_window_management_enabled(seat, enabled)
                .wrn("set_window_management_enabled")?,
            ClientMessage::SetVrrMode { connector, mode } => self
                .handle_set_vrr_mode(connector, mode)
                .wrn("set_vrr_mode")?,
            ClientMessage::SetVrrCursorHz { connector, hz } => self
                .handle_set_vrr_cursor_hz(connector, hz)
                .wrn("set_vrr_cursor_hz")?,
            ClientMessage::SetTearingMode { connector, mode } => self
                .handle_set_tearing_mode(connector, mode)
                .wrn("set_tearing_mode")?,
            ClientMessage::SetCalibrationMatrix { device, matrix } => self
                .handle_set_calibration_matrix(device, matrix)
                .wrn("set_calibration_matrix")?,
            ClientMessage::SetEiSocketEnabled { enabled } => {
                self.handle_set_ei_socket_enabled(enabled)
            }
            ClientMessage::ConnectorSetFormat { connector, format } => self
                .handle_connector_set_format(connector, format)
                .wrn("connector_set_format")?,
            ClientMessage::SetFlipMargin { device, margin } => self
                .handle_set_flip_margin(device, margin)
                .wrn("set_flip_margin")?,
            ClientMessage::SetUiDragEnabled { enabled } => self.handle_set_ui_drag_enabled(enabled),
            ClientMessage::SetUiDragThreshold { threshold } => {
                self.handle_set_ui_drag_threshold(threshold)
            }
            ClientMessage::SetXScalingMode { mode } => self
                .handle_set_x_scaling_mode(mode)
                .wrn("set_x_scaling_mode")?,
            ClientMessage::SetIdleGracePeriod { period } => {
                self.handle_set_idle_grace_period(period)
            }
            ClientMessage::SetColorManagementEnabled { enabled } => {
                self.handle_set_color_management_enabled(enabled)
            }
            ClientMessage::ConnectorSetColors {
                connector,
                color_space,
                transfer_function,
            } => self
                .handle_connector_set_colors(connector, color_space, transfer_function)
                .wrn("connector_set_colors")?,
            ClientMessage::ConnectorSetBrightness {
                connector,
                brightness,
            } => self
                .handle_connector_set_brightness(connector, brightness)
                .wrn("connector_set_brightness")?,
            ClientMessage::SetFloatAboveFullscreen { above } => {
                self.handle_set_float_above_fullscreen(above)
            }
            ClientMessage::GetFloatAboveFullscreen => self.handle_get_float_above_fullscreen(),
            ClientMessage::GetSeatFloatPinned { seat } => self
                .handle_get_seat_float_pinned(seat)
                .wrn("get_seat_float_pinned")?,
            ClientMessage::SetSeatFloatPinned { seat, pinned } => self
                .handle_set_seat_float_pinned(seat, pinned)
                .wrn("set_seat_float_pinned")?,
            ClientMessage::SetShowFloatPinIcon { show } => {
                self.handle_set_show_float_pin_icon(show)
            }
            ClientMessage::GetConnectorActiveWorkspace { connector } => self
                .handle_get_connector_active_workspace(connector)
                .wrn("get_connector_active_workspace")?,
            ClientMessage::GetConnectorWorkspaces { connector } => self
                .handle_get_connector_workspaces(connector)
                .wrn("get_connector_workspaces")?,
            ClientMessage::GetClients => self.handle_get_clients(),
            ClientMessage::ClientExists { client } => self.handle_client_exists(client),
            ClientMessage::ClientIsXwayland { client } => self
                .handle_client_is_xwayland(client)
                .wrn("client_is_xwayland")?,
            ClientMessage::ClientKill { client } => self.handle_client_kill(client),
            ClientMessage::WindowExists { window } => self.handle_window_exists(window),
            ClientMessage::GetWorkspaceWindow { workspace } => self
                .handle_get_workspace_window(workspace)
                .wrn("get_workspace_window")?,
            ClientMessage::GetSeatKeyboardWindow { seat } => self
                .handle_get_seat_keyboard_window(seat)
                .wrn("get_seat_keyboard_window")?,
            ClientMessage::SeatFocusWindow { seat, window } => self
                .handle_seat_focus_window(seat, window)
                .wrn("seat_focus_window")?,
            ClientMessage::GetWindowTitle { window } => self
                .handle_get_window_title(window)
                .wrn("get_window_title")?,
            ClientMessage::GetWindowType { window } => {
                self.handle_get_window_type(window).wrn("get_window_type")?
            }
            ClientMessage::GetWindowId { window } => {
                self.handle_get_window_id(window).wrn("get_window_id")?
            }
            ClientMessage::GetWindowParent { window } => self
                .handle_get_window_parent(window)
                .wrn("get_window_parent")?,
            ClientMessage::GetWindowWorkspace { window } => self
                .handle_get_window_workspace(window)
                .wrn("get_window_workspace")?,
            ClientMessage::GetWindowChildren { window } => self
                .handle_get_window_children(window)
                .wrn("get_window_children")?,
            ClientMessage::GetWindowSplit { window } => self
                .handle_get_window_split(window)
                .wrn("get_window_split")?,
            ClientMessage::SetWindowSplit { window, axis } => self
                .handle_set_window_split(window, axis)
                .wrn("set_window_split")?,
            ClientMessage::GetWindowMono { window } => {
                self.handle_get_window_mono(window).wrn("get_window_mono")?
            }
            ClientMessage::SetWindowMono { window, mono } => self
                .handle_set_window_mono(window, mono)
                .wrn("set_window_mono")?,
            ClientMessage::WindowMove { window, direction } => self
                .handle_window_move(window, direction)
                .wrn("window_move")?,
            ClientMessage::CreateWindowSplit { window, axis } => self
                .handle_create_window_split(window, axis)
                .wrn("create_window_split")?,
            ClientMessage::WindowClose { window } => {
                self.handle_window_close(window).wrn("close_window")?
            }
            ClientMessage::GetWindowFloating { window } => self
                .handle_get_window_floating(window)
                .wrn("get_window_floating")?,
            ClientMessage::SetWindowFloating { window, floating } => self
                .handle_set_window_floating(window, floating)
                .wrn("set_window_floating")?,
            ClientMessage::SetWindowWorkspace { window, workspace } => self
                .handle_set_window_workspace(window, workspace)
                .wrn("set_window_workspace")?,
            ClientMessage::SetWindowFullscreen { window, fullscreen } => self
                .handle_set_window_fullscreen(window, fullscreen)
                .wrn("set_window_fullscreen")?,
            ClientMessage::GetWindowFullscreen { window } => self
                .handle_get_window_fullscreen(window)
                .wrn("get_window_fullscreen")?,
            ClientMessage::GetWindowFloatPinned { window } => self
                .handle_get_window_float_pinned(window)
                .wrn("get_window_float_pinned")?,
            ClientMessage::SetWindowFloatPinned { window, pinned } => self
                .handle_set_window_float_pinned(window, pinned)
                .wrn("set_window_float_pinned")?,
            ClientMessage::GetWindowIsVisible { window } => self
                .handle_get_window_is_visible(window)
                .wrn("get_window_is_visible")?,
            ClientMessage::GetWindowClient { window } => self
                .handle_get_window_client(window)
                .wrn("get_window_client")?,
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
enum CphError {
    #[error("Tried to set an unknown accel profile: {}", (.0).0)]
    UnknownAccelProfile(AccelProfile),
    #[error("Queried unknown capability: {}", (.0).0)]
    UnknownCapability(Capability),
    #[error("The sized {} is outside the valid range [{}, {}] for component {}", .0, .1.min(), .1.max(), .1.name())]
    InvalidSize(i32, ThemeSized),
    #[error("The ol' forker is not available")]
    NoForker,
    #[error("Repeat rate is negative")]
    NegativeRepeatRate,
    #[error("Repeat delay is negative")]
    NegativeRepeatDelay,
    #[error("Parsing failed")]
    ParseKeymapError(#[from] KbvmError),
    #[error("Device {0:?} does not exist")]
    DeviceDoesNotExist(InputDevice),
    #[error("Connector {0:?} does not exist")]
    ConnectorDoesNotExist(Connector),
    #[error("Timer {0:?} does not exist")]
    TimerDoesNotExist(JayTimer),
    #[error("Connector {0:?} does not exist or is not connected")]
    OutputDoesNotExist(Connector),
    #[error("Output {0:?} is not a desktop output")]
    OutputIsNotDesktop(Connector),
    #[error("{0}x{1} is not a valid connector position")]
    InvalidConnectorPosition(i32, i32),
    #[error("Keymap {0:?} does not exist")]
    KeymapDoesNotExist(Keymap),
    #[error("Seat {0:?} does not exist")]
    SeatDoesNotExist(Seat),
    #[error("DRM device {0:?} does not exist")]
    DrmDeviceDoesNotExist(DrmDevice),
    #[error("Workspace {0:?} does not exist")]
    WorkspaceDoesNotExist(Workspace),
    #[error("Keyboard {0:?} does not exist")]
    KeyboardDoesNotExist(InputDevice),
    #[error("Colorable element {0} is not known")]
    UnknownColor(u32),
    #[error("Sized element {0} is not known")]
    UnknownSized(u32),
    #[error("Could not parse the message")]
    ParsingFailed(#[source] bincode::Error),
    #[error("Could not process a `{0}` request")]
    FailedRequest(&'static str, #[source] Box<Self>),
    #[error(transparent)]
    TimerError(#[from] TimerError),
    #[error("The requested monitor scale {0} is too small")]
    ScaleTooSmall(f64),
    #[error("The requested monitor scale {0} is too large")]
    ScaleTooLarge(f64),
    #[error("Tried to set a negative cursor size")]
    NegativeCursorSize,
    #[error("Config referred to a pollable that does not exist")]
    PollableDoesNotExist,
    #[error("Unknown VRR mode {0:?}")]
    UnknownVrrMode(ConfigVrrMode),
    #[error("Invalid cursor hz {0}")]
    InvalidCursorHz(f64),
    #[error("Unknown tearing mode {0:?}")]
    UnknownTearingMode(ConfigTearingMode),
    #[error("The format {0:?} is unknown")]
    UnknownFormat(ConfigFormat),
    #[error("Unknown x scaling mode {0:?}")]
    UnknownXScalingMode(XScalingMode),
    #[error("Unknown color space {0:?}")]
    UnknownColorSpace(ColorSpace),
    #[error("Unknown transfer function {0:?}")]
    UnknownTransferFunction(ConfigTransferFunction),
    #[error("Client {0:?} does not exist")]
    ClientDoesNotExist(ConfigClient),
    #[error("Window {0:?} does not exist")]
    WindowDoesNotExist(Window),
    #[error("Window {0:?} is not visible")]
    WindowNotVisible(Window),
}

trait WithRequestName {
    fn wrn(self, request: &'static str) -> Result<(), CphError>;
}

impl WithRequestName for Result<(), CphError> {
    fn wrn(self, request: &'static str) -> Result<(), CphError> {
        self.map_err(move |e| CphError::FailedRequest(request, Box::new(e)))
    }
}
