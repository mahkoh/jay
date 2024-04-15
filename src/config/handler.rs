use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            self, ConnectorId, DrmDeviceId, InputDeviceAccelProfile, InputDeviceCapability,
            InputDeviceId,
        },
        compositor::MAX_EXTENTS,
        config::ConfigProxy,
        ifs::wl_seat::{SeatId, WlSeatGlobal},
        io_uring::TaskResultExt,
        scale::Scale,
        state::{ConnectorData, DeviceHandlerData, DrmDevData, OutputData, State},
        theme::{Color, ThemeSized, DEFAULT_FONT},
        tree::{
            move_ws_to_output, ContainerNode, ContainerSplit, FloatNode, Node, NodeVisitorBase,
            OutputNode, WsMoveConfig,
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
        },
        xkbcommon::{XkbCommonError, XkbKeymap},
    },
    bincode::Options,
    jay_config::{
        _private::{
            bincode_ops,
            ipc::{ClientMessage, Response, ServerMessage, WorkspaceSource},
            PollableId, WireMode,
        },
        input::{
            acceleration::{AccelProfile, ACCEL_PROFILE_ADAPTIVE, ACCEL_PROFILE_FLAT},
            capability::{
                Capability, CAP_GESTURE, CAP_KEYBOARD, CAP_POINTER, CAP_SWITCH, CAP_TABLET_PAD,
                CAP_TABLET_TOOL, CAP_TOUCH,
            },
            InputDevice, Seat,
        },
        keyboard::{mods::Modifiers, syms::KeySym, Keymap},
        logging::LogLevel,
        theme::{colors::Colorable, sized::Resizable},
        timer::Timer as JayTimer,
        video::{Connector, DrmDevice, GfxApi, Transform},
        Axis, Direction, Workspace,
    },
    libloading::Library,
    log::Level,
    std::{cell::Cell, ops::Deref, rc::Rc, time::Duration},
    thiserror::Error,
    uapi::{c, fcntl_dupfd_cloexec, OwnedFd},
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
    pub keymaps: CopyHashMap<Keymap, Rc<XkbKeymap>>,
    pub bufs: Stack<Vec<u8>>,

    pub workspace_ids: NumCell<u64>,
    pub workspaces_by_name: CopyHashMap<Rc<String>, u64>,
    pub workspaces_by_id: CopyHashMap<u64, Rc<String>>,

    pub timer_ids: NumCell<u64>,
    pub timers_by_name: CopyHashMap<Rc<String>, Rc<TimerData>>,
    pub timers_by_id: CopyHashMap<u64, Rc<TimerData>>,

    pub pollable_id: NumCell<u64>,
    pub pollables: CopyHashMap<PollableId, Rc<Pollable>>,
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
        let (keymap, res) = match self.state.xkb_ctx.keymap_from_str(keymap) {
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

    fn handle_get_fullscreen(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetFullscreen {
            fullscreen: seat.get_fullscreen(),
        });
        Ok(())
    }

    fn handle_set_fullscreen(&self, seat: Seat, fullscreen: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_fullscreen(fullscreen);
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
            let id = match self.workspaces_by_name.get(&ws.name) {
                None => {
                    let id = self.workspace_ids.fetch_add(1);
                    let name = Rc::new(ws.name.clone());
                    self.workspaces_by_name.set(name.clone(), id);
                    self.workspaces_by_id.set(id, name);
                    id
                }
                Some(id) => id,
            };
            workspaces.push(Workspace(id));
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
            self.state.eng.spawn(async move {
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

    fn handle_close(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.close();
        Ok(())
    }

    fn handle_focus(&self, seat: Seat, direction: Direction) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.move_focus(direction.into());
        Ok(())
    }

    fn handle_move(&self, seat: Seat, direction: Direction) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.move_focused(direction.into());
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

    fn get_keymap(&self, keymap: Keymap) -> Result<Rc<XkbKeymap>, CphError> {
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

    fn handle_get_workspace(&self, name: &str) {
        let name = Rc::new(name.to_owned());
        let ws = match self.workspaces_by_name.get(&name) {
            Some(w) => w,
            _ => {
                let ws = self.workspace_ids.fetch_add(1);
                self.workspaces_by_name.set(name.clone(), ws);
                self.workspaces_by_id.set(ws, name);
                ws
            }
        };
        self.respond(Response::GetWorkspace {
            workspace: Workspace(ws),
        });
    }

    fn handle_get_workspace_capture(&self, workspace: Workspace) -> Result<(), CphError> {
        let name = self.get_workspace(workspace)?;
        let capture = match self.state.workspaces.get(name.as_str()) {
            Some(ws) => ws.capture.get(),
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
        let name = self.get_workspace(workspace)?;
        if let Some(ws) = self.state.workspaces.get(name.as_str()) {
            ws.capture.set(capture);
            ws.output.get().schedule_update_render_data();
            self.state.damage();
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
        let mut workspace = 0;
        if !output.is_dummy {
            if let Some(ws) = output.workspace.get() {
                if let Some(ws) = self.workspaces_by_name.get(&ws.name) {
                    workspace = ws;
                }
            }
        }
        self.respond(Response::GetSeatWorkspace {
            workspace: Workspace(workspace),
        });
        Ok(())
    }

    fn handle_show_workspace(&self, seat: Seat, ws: Workspace) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let name = self.get_workspace(ws)?;
        self.state.show_workspace(&seat, &name);
        Ok(())
    }

    fn handle_set_workspace(&self, seat: Seat, ws: Workspace) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let name = self.get_workspace(ws)?;
        let workspace = match self.state.workspaces.get(name.deref()) {
            Some(ws) => ws,
            _ => seat.get_output().create_workspace(name.deref()),
        };
        seat.set_workspace(&workspace);
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
        let output = self.get_output(connector)?;
        let ws = match workspace {
            WorkspaceSource::Explicit(ws) => {
                let name = self.get_workspace(ws)?;
                match self.state.workspaces.get(name.as_str()) {
                    Some(ws) => ws,
                    _ => return Ok(()),
                }
            }
            WorkspaceSource::Seat(s) => match self.get_seat(s)?.get_output().workspace.get() {
                Some(ws) => ws,
                _ => return Ok(()),
            },
        };
        if ws.is_dummy || output.node.is_dummy {
            return Ok(());
        }
        if ws.output.get().id == output.node.id {
            return Ok(());
        }
        let link = match &*ws.output_link.borrow() {
            None => return Ok(()),
            Some(l) => l.to_ref(),
        };
        let config = WsMoveConfig {
            make_visible_if_empty: true,
            source_is_destroyed: false,
        };
        move_ws_to_output(&link, &output.node, config);
        ws.desired_output.set(output.node.global.output_id.clone());
        self.state.tree_changed();
        self.state.damage();
        Ok(())
    }

    fn handle_set_idle(&self, timeout: Duration) {
        self.state.idle.set_timeout(timeout);
    }

    fn handle_set_explicit_sync_enabled(&self, enabled: bool) {
        self.state.explicit_sync_enabled.set(enabled);
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
        let connector = self.get_output(connector)?;
        let mode = connector.node.global.mode.get();
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
        let connector = self.get_output(connector)?;
        self.respond(Response::ConnectorModes {
            modes: connector
                .node
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
            model: connector.monitor_info.product.clone(),
        });
        Ok(())
    }

    fn handle_connector_manufacturer(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        self.respond(Response::GetConnectorManufacturer {
            manufacturer: connector.monitor_info.manufacturer.clone(),
        });
        Ok(())
    }

    fn handle_connector_serial_number(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        self.respond(Response::GetConnectorSerialNumber {
            serial_number: connector.monitor_info.serial_number.clone(),
        });
        Ok(())
    }

    fn handle_set_cursor_size(&self, seat: Seat, size: i32) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        if size < 0 {
            return Err(CphError::NegativeCursorSize);
        }
        seat.set_cursor_size(size as _);
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
        if use_hardware_cursor {
            for other in self.state.globals.seats.lock().values() {
                if other.id() != seat.id() {
                    other.set_hardware_cursor(false);
                }
            }
        }
        seat.set_hardware_cursor(use_hardware_cursor);
        self.state.refresh_hardware_cursors();
        Ok(())
    }

    fn handle_connector_size(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        let pos = connector.node.global.pos.get();
        self.respond(Response::ConnectorSize {
            width: pos.width(),
            height: pos.height(),
        });
        Ok(())
    }

    fn handle_connector_get_scale(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        self.respond(Response::ConnectorGetScale {
            scale: connector.node.global.persistent.scale.get().to_f64(),
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
        let connector = self.get_output(connector)?;
        connector.node.set_preferred_scale(scale);
        self.state.damage();
        Ok(())
    }

    fn handle_connector_set_transform(
        &self,
        connector: Connector,
        transform: Transform,
    ) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        connector.node.update_transform(transform);
        self.state.damage();
        Ok(())
    }

    fn handle_connector_set_position(
        &self,
        connector: Connector,
        x: i32,
        y: i32,
    ) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        if x < 0 || y < 0 || x > MAX_EXTENTS || y > MAX_EXTENTS {
            return Err(CphError::InvalidConnectorPosition(x, y));
        }
        let old_pos = connector.node.global.pos.get();
        connector.node.set_position(x, y);
        let seats = self.state.globals.seats.lock();
        for seat in seats.values() {
            if seat.get_output().id == connector.node.id {
                let seat_pos = seat.position();
                seat.set_position(
                    seat_pos.0.round_down() + x - old_pos.x1(),
                    seat_pos.1.round_down() + y - old_pos.y1(),
                );
            }
        }
        Ok(())
    }

    fn handle_connector_get_position(&self, connector: Connector) -> Result<(), CphError> {
        let connector = self.get_output(connector)?;
        let (x, y) = connector.node.global.pos.get().position();
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

    fn handle_get_mono(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetMono {
            mono: seat.get_mono().unwrap_or(false),
        });
        Ok(())
    }

    fn handle_set_mono(&self, seat: Seat, mono: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_mono(mono);
        Ok(())
    }

    fn handle_get_split(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetSplit {
            axis: seat
                .get_split()
                .unwrap_or(ContainerSplit::Horizontal)
                .into(),
        });
        Ok(())
    }

    fn handle_set_split(&self, seat: Seat, axis: Axis) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_split(axis.into());
        Ok(())
    }

    fn handle_add_shortcut(
        &self,
        seat: Seat,
        mods: Modifiers,
        sym: KeySym,
    ) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.add_shortcut(mods, sym);
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

    fn handle_create_split(&self, seat: Seat, axis: Axis) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.create_split(axis.into());
        Ok(())
    }

    fn handle_focus_parent(&self, seat: Seat) -> Result<(), CphError> {
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

    fn handle_get_floating(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        self.respond(Response::GetFloating {
            floating: seat.get_floating().unwrap_or(false),
        });
        Ok(())
    }

    fn handle_set_floating(&self, seat: Seat, floating: bool) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.set_floating(floating);
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
            let future = self.state.eng.spawn(async move {
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
        *self.state.theme.font.borrow_mut() = DEFAULT_FONT.to_string();
    }

    fn handle_set_font(&self, font: &str) {
        *self.state.theme.font.borrow_mut() = font.to_string();
    }

    fn handle_get_font(&self) {
        let font = self.state.theme.font.borrow_mut().clone();
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
            _ => return Err(CphError::UnknownColor(colorable.0)),
        };
        Ok(colorable)
    }

    fn handle_get_color(&self, colorable: Colorable) -> Result<(), CphError> {
        let color = self.get_color(colorable)?.get();
        let color =
            jay_config::theme::Color::new_f32_premultiplied(color.r, color.g, color.b, color.a);
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
            ClientMessage::GetMono { seat } => self.handle_get_mono(seat).wrn("get_mono")?,
            ClientMessage::SetMono { seat, mono } => {
                self.handle_set_mono(seat, mono).wrn("set_mono")?
            }
            ClientMessage::GetSplit { seat } => self.handle_get_split(seat).wrn("get_split")?,
            ClientMessage::SetSplit { seat, axis } => {
                self.handle_set_split(seat, axis).wrn("set_split")?
            }
            ClientMessage::AddShortcut { seat, mods, sym } => self
                .handle_add_shortcut(seat, mods, sym)
                .wrn("add_shortcut")?,
            ClientMessage::RemoveShortcut { seat, mods, sym } => self
                .handle_remove_shortcut(seat, mods, sym)
                .wrn("remove_shortcut")?,
            ClientMessage::Focus { seat, direction } => {
                self.handle_focus(seat, direction).wrn("focus")?
            }
            ClientMessage::Move { seat, direction } => {
                self.handle_move(seat, direction).wrn("move")?
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
            ClientMessage::CreateSplit { seat, axis } => {
                self.handle_create_split(seat, axis).wrn("create_split")?
            }
            ClientMessage::FocusParent { seat } => {
                self.handle_focus_parent(seat).wrn("focus_parent")?
            }
            ClientMessage::GetFloating { seat } => {
                self.handle_get_floating(seat).wrn("get_floating")?
            }
            ClientMessage::SetFloating { seat, floating } => self
                .handle_set_floating(seat, floating)
                .wrn("set_floating")?,
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
            ClientMessage::SetWorkspace { seat, workspace } => self
                .handle_set_workspace(seat, workspace)
                .wrn("set_workspace")?,
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
            ClientMessage::Close { seat } => self.handle_close(seat).wrn("close")?,
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
            ClientMessage::SetFullscreen { seat, fullscreen } => self
                .handle_set_fullscreen(seat, fullscreen)
                .wrn("set_fullscreen")?,
            ClientMessage::GetFullscreen { seat } => {
                self.handle_get_fullscreen(seat).wrn("get_fullscreen")?
            }
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
    #[error("The sized {0} is outside the valid range [{}, {}] for component {}", .1.min(), .1.max(), .1.name())]
    InvalidSize(i32, ThemeSized),
    #[error("The ol' forker is not available")]
    NoForker,
    #[error("Repeat rate is negative")]
    NegativeRepeatRate,
    #[error("Repeat delay is negative")]
    NegativeRepeatDelay,
    #[error("Parsing failed")]
    ParseKeymapError(#[from] XkbCommonError),
    #[error("Device {0:?} does not exist")]
    DeviceDoesNotExist(InputDevice),
    #[error("Connector {0:?} does not exist")]
    ConnectorDoesNotExist(Connector),
    #[error("Timer {0:?} does not exist")]
    TimerDoesNotExist(JayTimer),
    #[error("Connector {0:?} does not exist or is not connected")]
    OutputDoesNotExist(Connector),
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
}

trait WithRequestName {
    fn wrn(self, request: &'static str) -> Result<(), CphError>;
}

impl WithRequestName for Result<(), CphError> {
    fn wrn(self, request: &'static str) -> Result<(), CphError> {
        self.map_err(move |e| CphError::FailedRequest(request, Box::new(e)))
    }
}
