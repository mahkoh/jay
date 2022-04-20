use {
    crate::{
        async_engine::{AsyncError, SpawnedFuture, Timer},
        backend::{
            self, ConnectorId, InputDeviceAccelProfile, InputDeviceCapability, InputDeviceId,
        },
        compositor::MAX_EXTENTS,
        ifs::wl_seat::{SeatId, WlSeatGlobal},
        state::{ConnectorData, DeviceHandlerData, OutputData, State},
        tree::{ContainerNode, ContainerSplit, FloatNode, NodeVisitorBase, SizedNode},
        utils::{
            copyhashmap::CopyHashMap, debug_fn::debug_fn, errorfmt::ErrorFmt, numcell::NumCell,
            stack::Stack,
        },
        xkbcommon::{XkbCommonError, XkbKeymap},
    },
    bincode::error::DecodeError,
    jay_config::{
        _private::{
            bincode_ops,
            ipc::{ClientMessage, Response, ServerMessage},
        },
        drm::Connector,
        input::{
            acceleration::{AccelProfile, ACCEL_PROFILE_ADAPTIVE, ACCEL_PROFILE_FLAT},
            capability::{
                Capability, CAP_GESTURE, CAP_KEYBOARD, CAP_POINTER, CAP_SWITCH, CAP_TABLET_PAD,
                CAP_TABLET_TOOL, CAP_TOUCH,
            },
            InputDevice, Seat,
        },
        keyboard::{keymap::Keymap, mods::Modifiers, syms::KeySym},
        Axis, Direction, LogLevel, Workspace,
    },
    libloading::Library,
    log::Level,
    std::{cell::Cell, rc::Rc, time::Duration},
    thiserror::Error,
    uapi::c,
};

pub(super) struct ConfigProxyHandler {
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
}

pub(super) struct TimerData {
    timer: Timer,
    id: u64,
    name: Rc<String>,
    _handler: SpawnedFuture<()>,
}

impl ConfigProxyHandler {
    pub fn do_drop(&self) {
        self.dropped.set(true);

        self.timers_by_name.clear();
        self.timers_by_id.clear();
    }

    pub fn send(&self, msg: &ServerMessage) {
        let mut buf = self.bufs.pop().unwrap_or_default();
        buf.clear();
        bincode::encode_into_std_write(msg, &mut buf, bincode_ops()).unwrap();
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

    fn handle_create_seat(&self, name: &str) {
        let global_name = self.state.globals.name();
        let seat = WlSeatGlobal::new(global_name, name, &self.state);
        self.state.globals.add_global(&self.state, &seat);
        self.respond(Response::CreateSeat {
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
        seat.set_keymap(&keymap);
        Ok(())
    }

    fn handle_set_status(&self, status: &str) {
        self.state.set_status(status);
    }

    fn get_timer(&self, timer: jay_config::Timer) -> Result<Rc<TimerData>, CphError> {
        match self.timers_by_id.get(&timer.0) {
            Some(t) => Ok(t),
            _ => Err(CphError::TimerDoesNotExist(timer)),
        }
    }

    fn handle_remove_timer(&self, timer: jay_config::Timer) -> Result<(), CphError> {
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

    fn handle_program_timer(
        &self,
        timer: jay_config::Timer,
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
                timer: jay_config::Timer(t.id),
            });
            return Ok(());
        }
        let id = self.timer_ids.fetch_add(1);
        let timer = self.state.eng.timer(c::CLOCK_BOOTTIME)?;
        let handler = {
            let timer = timer.clone();
            let slf = self.clone();
            self.state.eng.spawn(async move {
                loop {
                    match timer.expired().await {
                        Ok(_) => slf.send(&ServerMessage::TimerExpired {
                            timer: jay_config::Timer(id),
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
            timer: jay_config::Timer(id),
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
        seat.move_focus(direction);
        Ok(())
    }

    fn handle_move(&self, seat: Seat, direction: Direction) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.move_focused(direction);
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

    fn get_seat(&self, seat: Seat) -> Result<Rc<WlSeatGlobal>, CphError> {
        let seats = self.state.globals.seats.lock();
        for seat_global in seats.values() {
            if seat_global.id().raw() == seat.0 as _ {
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
        dev.seat.set(seat);
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

    fn handle_show_workspace(&self, seat: Seat, ws: Workspace) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        let name = self.get_workspace(ws)?;
        self.state.show_workspace(&seat, &name);
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

    fn handle_get_connector(
        &self,
        ty: jay_config::drm::connector_type::ConnectorType,
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
    ) -> Result<(), CphError> {
        let forker = match self.state.forker.get() {
            Some(f) => f,
            _ => return Err(CphError::NoForker),
        };
        forker.spawn(prog.to_string(), args, env, None);
        Ok(())
    }

    fn handle_grab(&self, kb: InputDevice, grab: bool) -> Result<(), CphError> {
        let kb = self.get_kb(kb)?;
        kb.grab(grab);
        Ok(())
    }

    fn handle_get_title_height(&self) {
        self.respond(Response::GetTitleHeight {
            height: self.state.theme.title_height.get(),
        });
    }

    fn handle_get_border_width(&self) {
        self.respond(Response::GetBorderWidth {
            width: self.state.theme.border_width.get(),
        });
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
        self.state.el.stop();
    }

    fn handle_switch_to(&self, vtnr: u32) {
        self.state.backend.get().switch_to(vtnr);
    }

    fn handle_toggle_floating(&self, seat: Seat) -> Result<(), CphError> {
        let seat = self.get_seat(seat)?;
        seat.toggle_floating();
        Ok(())
    }

    fn spaces_change(&self) {
        struct V;
        impl NodeVisitorBase for V {
            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.on_spaces_changed();
                node.visit_children(self);
            }
            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                node.on_spaces_changed();
                node.visit_children(self);
            }
        }
        self.state.root.clone().visit(&mut V);
    }

    fn colors_change(&self) {
        struct V;
        impl NodeVisitorBase for V {
            fn visit_container(&mut self, node: &Rc<ContainerNode>) {
                node.on_colors_changed();
                node.visit_children(self);
            }
            fn visit_float(&mut self, node: &Rc<FloatNode>) {
                node.on_colors_changed();
                node.visit_children(self);
            }
        }
        self.state.root.clone().visit(&mut V);
    }

    fn handle_set_title_height(&self, height: i32) -> Result<(), CphError> {
        if height < 0 {
            return Err(CphError::NegativeTitleHeight(height));
        }
        if height > 1000 {
            return Err(CphError::ExcessiveTitleHeight(height));
        }
        self.state.theme.title_height.set(height);
        self.spaces_change();
        Ok(())
    }

    fn handle_set_border_width(&self, width: i32) -> Result<(), CphError> {
        if width < 0 {
            return Err(CphError::NegativeBorderWidth(width));
        }
        if width > 1000 {
            return Err(CphError::ExcessiveBorderWidth(width));
        }
        self.state.theme.border_width.set(width);
        self.spaces_change();
        Ok(())
    }

    fn handle_set_title_color(&self, color: jay_config::theme::Color) {
        self.state.theme.title_color.set(color.into());
        self.colors_change();
    }

    fn handle_set_border_color(&self, color: jay_config::theme::Color) {
        self.state.theme.border_color.set(color.into());
    }

    fn handle_set_background_color(&self, color: jay_config::theme::Color) {
        self.state.theme.background_color.set(color.into());
    }

    fn handle_set_title_underline_color(&self, color: jay_config::theme::Color) {
        self.state.theme.underline_color.set(color.into());
    }

    pub fn handle_request(self: &Rc<Self>, msg: &[u8]) {
        if let Err(e) = self.handle_request_(msg) {
            log::error!("Could not handle client request: {}", ErrorFmt(e));
        }
    }

    fn handle_request_(self: &Rc<Self>, msg: &[u8]) -> Result<(), CphError> {
        let (request, _) = match bincode::decode_from_slice::<ClientMessage, _>(msg, bincode_ops())
        {
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
            ClientMessage::CreateSeat { name } => self.handle_create_seat(name),
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
                self.handle_run(prog, args, env).wrn("run")?
            }
            ClientMessage::GrabKb { kb, grab } => self.handle_grab(kb, grab).wrn("grab")?,
            ClientMessage::SetTitleHeight { height } => self
                .handle_set_title_height(height)
                .wrn("set_title_height")?,
            ClientMessage::SetBorderWidth { width } => self
                .handle_set_border_width(width)
                .wrn("set_bordre_width")?,
            ClientMessage::SetTitleColor { color } => self.handle_set_title_color(color),
            ClientMessage::SetTitleUnderlineColor { color } => {
                self.handle_set_title_underline_color(color)
            }
            ClientMessage::SetBorderColor { color } => self.handle_set_border_color(color),
            ClientMessage::SetBackgroundColor { color } => self.handle_set_background_color(color),
            ClientMessage::GetTitleHeight => self.handle_get_title_height(),
            ClientMessage::GetBorderWidth => self.handle_get_border_width(),
            ClientMessage::CreateSplit { seat, axis } => {
                self.handle_create_split(seat, axis).wrn("create_split")?
            }
            ClientMessage::FocusParent { seat } => {
                self.handle_focus_parent(seat).wrn("focus_parent")?
            }
            ClientMessage::ToggleFloating { seat } => {
                self.handle_toggle_floating(seat).wrn("toggle_floating")?
            }
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
    #[error("The height {0} is negative")]
    NegativeTitleHeight(i32),
    #[error("The height {0} is larger than the maximum 1000")]
    ExcessiveTitleHeight(i32),
    #[error("The width {0} is negative")]
    NegativeBorderWidth(i32),
    #[error("The width {0} is larger than the maximum 1000")]
    ExcessiveBorderWidth(i32),
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
    TimerDoesNotExist(jay_config::Timer),
    #[error("Connector {0:?} does not exist or is not connected")]
    OutputDoesNotExist(Connector),
    #[error("{0}x{1} is not a valid connector position")]
    InvalidConnectorPosition(i32, i32),
    #[error("Keymap {0:?} does not exist")]
    KeymapDoesNotExist(Keymap),
    #[error("Seat {0:?} does not exist")]
    SeatDoesNotExist(Seat),
    #[error("Workspace {0:?} does not exist")]
    WorkspaceDoesNotExist(Workspace),
    #[error("Keyboard {0:?} does not exist")]
    KeyboardDoesNotExist(InputDevice),
    #[error("Could not parse the message")]
    ParsingFailed(#[source] DecodeError),
    #[error("Could not process a `{0}` request")]
    FailedRequest(&'static str, #[source] Box<Self>),
    #[error(transparent)]
    AsyncError(#[from] AsyncError),
}

trait WithRequestName {
    fn wrn(self, request: &'static str) -> Result<(), CphError>;
}

impl WithRequestName for Result<(), CphError> {
    fn wrn(self, request: &'static str) -> Result<(), CphError> {
        self.map_err(move |e| CphError::FailedRequest(request, Box::new(e)))
    }
}
