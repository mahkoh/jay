#![allow(
    clippy::len_zero,
    clippy::single_char_pattern,
    clippy::collapsible_if,
    clippy::collapsible_else_if
)]

mod config;
mod rules;
mod toml;

use {
    crate::{
        config::{
            Action, ClientRule, Config, ConfigConnector, ConfigDrmDevice, ConfigKeymap,
            ConnectorMatch, DrmDeviceMatch, Exec, Input, InputMatch, Output, OutputMatch, Shortcut,
            SimpleCommand, Status, Theme, WindowRule, parse_config,
        },
        rules::{MatcherTemp, RuleMapper},
    },
    ahash::{AHashMap, AHashSet},
    error_reporter::Report,
    jay_config::{
        client::Client,
        config, config_dir,
        exec::{Command, set_env, unset_env},
        get_workspace,
        input::{
            FocusFollowsMouseMode, InputDevice, Seat, SwitchEvent, capability::CAP_SWITCH,
            get_seat, input_devices, on_input_device_removed, on_new_input_device,
            set_libei_socket_enabled,
        },
        is_reload,
        keyboard::{Keymap, ModifiedKeySym},
        logging::set_log_level,
        on_devices_enumerated, on_idle, on_unload, quit, reload, set_color_management_enabled,
        set_default_workspace_capture, set_explicit_sync_enabled, set_float_above_fullscreen,
        set_idle, set_idle_grace_period, set_show_float_pin_icon, set_ui_drag_enabled,
        set_ui_drag_threshold,
        status::{set_i3bar_separator, set_status, set_status_command, unset_status_command},
        switch_to_vt,
        theme::{reset_colors, reset_font, reset_sizes, set_font},
        toggle_float_above_fullscreen,
        video::{
            ColorSpace, Connector, DrmDevice, TransferFunction, connectors, drm_devices,
            on_connector_connected, on_connector_disconnected, on_graphics_initialized,
            on_new_connector, on_new_drm_device, set_direct_scanout_enabled, set_gfx_api,
            set_tearing_mode, set_vrr_cursor_hz, set_vrr_mode,
        },
        window::Window,
        xwayland::set_x_scaling_mode,
    },
    run_on_drop::on_drop,
    std::{
        cell::{Cell, RefCell},
        io::ErrorKind,
        path::PathBuf,
        rc::Rc,
        time::Duration,
    },
};

fn default_seat() -> Seat {
    get_seat("default")
}

trait FnBuilder: Sized {
    fn new<F: Fn() + 'static>(f: F) -> Self;
}

impl FnBuilder for Box<dyn Fn()> {
    fn new<F: Fn() + 'static>(f: F) -> Self {
        Box::new(f)
    }
}

impl FnBuilder for Rc<dyn Fn()> {
    fn new<F: Fn() + 'static>(f: F) -> Self {
        Rc::new(f)
    }
}

impl Action {
    fn into_fn(self, state: &Rc<State>) -> Box<dyn Fn()> {
        self.into_fn_impl(state)
    }

    fn into_rc_fn(self, state: &Rc<State>) -> Rc<dyn Fn()> {
        self.into_fn_impl(state)
    }

    fn into_fn_impl<B: FnBuilder>(self, state: &Rc<State>) -> B {
        macro_rules! client_action {
            ($name:ident, $opt:expr) => {{
                let state = state.clone();
                B::new(move || {
                    if let Some($name) = state.client.get() {
                        $opt
                    }
                })
            }};
        }
        let s = state.persistent.seat;
        macro_rules! window_or_seat {
            ($name:ident, $expr:expr) => {{
                let state = state.clone();
                B::new(move || {
                    if let Some($name) = state.window.get() {
                        if let Some($name) = $name {
                            $expr;
                        }
                    } else {
                        let $name = s;
                        $expr;
                    }
                })
            }};
        }
        match self {
            Action::SimpleCommand { cmd } => match cmd {
                SimpleCommand::Focus(dir) => B::new(move || s.focus(dir)),
                SimpleCommand::Move(dir) => window_or_seat!(s, s.move_(dir)),
                SimpleCommand::Split(axis) => window_or_seat!(s, s.create_split(axis)),
                SimpleCommand::ToggleSplit => window_or_seat!(s, s.toggle_split()),
                SimpleCommand::SetSplit(b) => window_or_seat!(s, s.set_split(b)),
                SimpleCommand::ToggleMono => window_or_seat!(s, s.toggle_mono()),
                SimpleCommand::SetMono(b) => window_or_seat!(s, s.set_mono(b)),
                SimpleCommand::ToggleFullscreen => window_or_seat!(s, s.toggle_fullscreen()),
                SimpleCommand::SetFullscreen(b) => window_or_seat!(s, s.set_fullscreen(b)),
                SimpleCommand::FocusParent => B::new(move || s.focus_parent()),
                SimpleCommand::Close => window_or_seat!(s, s.close()),
                SimpleCommand::DisablePointerConstraint => {
                    B::new(move || s.disable_pointer_constraint())
                }
                SimpleCommand::ToggleFloating => window_or_seat!(s, s.toggle_floating()),
                SimpleCommand::SetFloating(b) => window_or_seat!(s, s.set_floating(b)),
                SimpleCommand::Quit => B::new(quit),
                SimpleCommand::ReloadConfigToml => {
                    let persistent = state.persistent.clone();
                    B::new(move || load_config(false, &persistent))
                }
                SimpleCommand::ReloadConfigSo => B::new(reload),
                SimpleCommand::None => B::new(|| ()),
                SimpleCommand::Forward(bool) => B::new(move || s.set_forward(bool)),
                SimpleCommand::EnableWindowManagement(bool) => {
                    B::new(move || s.set_window_management_enabled(bool))
                }
                SimpleCommand::SetFloatAboveFullscreen(bool) => {
                    B::new(move || set_float_above_fullscreen(bool))
                }
                SimpleCommand::ToggleFloatAboveFullscreen => B::new(toggle_float_above_fullscreen),
                SimpleCommand::SetFloatPinned(pinned) => {
                    window_or_seat!(s, s.set_float_pinned(pinned))
                }
                SimpleCommand::ToggleFloatPinned => window_or_seat!(s, s.toggle_float_pinned()),
                SimpleCommand::KillClient => client_action!(c, c.kill()),
            },
            Action::Multi { actions } => {
                let actions: Vec<_> = actions.into_iter().map(|a| a.into_fn(state)).collect();
                B::new(move || {
                    for action in &actions {
                        action();
                    }
                })
            }
            Action::Exec { exec } => B::new(move || create_command(&exec).spawn()),
            Action::SwitchToVt { num } => B::new(move || switch_to_vt(num)),
            Action::ShowWorkspace { name } => {
                let workspace = get_workspace(&name);
                B::new(move || s.show_workspace(workspace))
            }
            Action::MoveToWorkspace { name } => {
                let workspace = get_workspace(&name);
                window_or_seat!(s, s.set_workspace(workspace))
            }
            Action::ConfigureConnector { con } => B::new(move || {
                for c in connectors() {
                    if con.match_.matches(c) {
                        con.apply(c);
                    }
                }
            }),
            Action::ConfigureInput { input } => {
                let state = state.clone();
                B::new(move || {
                    for c in input_devices() {
                        if input.match_.matches(c, &state) {
                            input.apply(c, &state);
                        }
                    }
                })
            }
            Action::ConfigureOutput { out } => {
                let state = state.clone();
                B::new(move || {
                    for c in connectors() {
                        if out.match_.matches(c, &state) {
                            out.apply(c);
                        }
                    }
                })
            }
            Action::SetEnv { env } => B::new(move || {
                for (k, v) in &env {
                    set_env(k, v);
                }
            }),
            Action::UnsetEnv { env } => B::new(move || {
                for k in &env {
                    unset_env(k);
                }
            }),
            Action::SetKeymap { map } => {
                let state = state.clone();
                B::new(move || state.set_keymap(&map))
            }
            Action::SetStatus { status } => {
                let state = state.clone();
                B::new(move || state.set_status(&status))
            }
            Action::SetTheme { theme } => {
                let state = state.clone();
                B::new(move || state.apply_theme(&theme))
            }
            Action::SetLogLevel { level } => B::new(move || set_log_level(level)),
            Action::SetGfxApi { api } => B::new(move || set_gfx_api(api)),
            Action::ConfigureDirectScanout { enabled } => {
                B::new(move || set_direct_scanout_enabled(enabled))
            }
            Action::ConfigureDrmDevice { dev } => {
                let state = state.clone();
                B::new(move || {
                    for d in drm_devices() {
                        if dev.match_.matches(d, &state) {
                            dev.apply(d);
                        }
                    }
                })
            }
            Action::SetRenderDevice { dev } => {
                let state = state.clone();
                B::new(move || {
                    for d in drm_devices() {
                        if dev.matches(d, &state) {
                            d.make_render_device();
                        }
                    }
                })
            }
            Action::ConfigureIdle { idle, grace_period } => B::new(move || {
                if let Some(idle) = idle {
                    set_idle(Some(idle))
                }
                if let Some(period) = grace_period {
                    set_idle_grace_period(period)
                }
            }),
            Action::MoveToOutput { output, workspace } => {
                let state = state.clone();
                B::new(move || {
                    let output = 'get_output: {
                        for connector in connectors() {
                            if connector.connected() && output.matches(connector, &state) {
                                break 'get_output connector;
                            }
                        }
                        return;
                    };
                    match workspace {
                        Some(ws) => ws.move_to_output(output),
                        None => s.move_to_output(output),
                    }
                })
            }
            Action::SetRepeatRate { rate } => {
                B::new(move || s.set_repeat_rate(rate.rate, rate.delay))
            }
            Action::DefineAction { name, action } => {
                let state = state.clone();
                let action = action.into_rc_fn(&state);
                let name = Rc::new(name);
                B::new(move || {
                    state
                        .persistent
                        .actions
                        .borrow_mut()
                        .insert(name.clone(), action.clone());
                })
            }
            Action::UndefineAction { name } => {
                let state = state.clone();
                B::new(move || {
                    state.persistent.actions.borrow_mut().remove(&name);
                })
            }
            Action::NamedAction { name } => {
                let state = state.clone();
                B::new(move || {
                    let depth = state.action_depth.get();
                    if depth >= state.action_depth_max {
                        log::error!("Maximum action depth reached");
                        return;
                    }
                    state.action_depth.set(depth + 1);
                    let _reset = on_drop(|| state.action_depth.set(depth));
                    let Some(action) = state.persistent.actions.borrow().get(&name).cloned() else {
                        log::error!("There is no action named {name}");
                        return;
                    };
                    action();
                })
            }
        }
    }
}

fn apply_recursive_match<'a, U>(
    type_name: &str,
    list: &'a AHashMap<String, U>,
    active: &mut AHashSet<&'a str>,
    name: &'a str,
    matches: impl FnOnce(&'a U, &mut AHashSet<&'a str>) -> bool,
) -> bool {
    match list.get(name) {
        None => {
            log::warn!("{type_name} with name {name} does not exist");
            false
        }
        Some(m) => {
            if active.insert(name) {
                let matches = matches(m, active);
                active.remove(name);
                matches
            } else {
                log::warn!("Recursion while evaluating match for {type_name} {name}");
                false
            }
        }
    }
}

impl ConfigDrmDevice {
    fn apply(&self, d: DrmDevice) {
        if let Some(api) = self.gfx_api {
            d.set_gfx_api(api);
        }
        if let Some(dse) = self.direct_scanout_enabled {
            d.set_direct_scanout_enabled(dse);
        }
        if let Some(fm) = self.flip_margin_ms {
            d.set_flip_margin(Duration::from_nanos((fm * 1_000_000.0) as _));
        }
    }
}

impl DrmDeviceMatch {
    fn matches(&self, d: DrmDevice, state: &State) -> bool {
        self.matches_(d, state, &mut AHashSet::new())
    }

    fn matches_<'a>(
        &'a self,
        d: DrmDevice,
        state: &'a State,
        active: &mut AHashSet<&'a str>,
    ) -> bool {
        match self {
            DrmDeviceMatch::Any(m) => m.iter().any(|m| m.matches_(d, state, active)),
            DrmDeviceMatch::All {
                name,
                syspath,
                vendor,
                vendor_name,
                model,
                model_name,
                devnode,
            } => {
                if let Some(name) = name {
                    let matches = apply_recursive_match(
                        "drm device",
                        &state.drm_devices,
                        active,
                        name,
                        |m, active| m.matches_(d, state, active),
                    );
                    if !matches {
                        return false;
                    }
                }
                if let Some(syspath) = syspath {
                    if d.syspath() != *syspath {
                        return false;
                    }
                }
                if let Some(devnode) = devnode {
                    if d.devnode() != *devnode {
                        return false;
                    }
                }
                if let Some(model) = model_name {
                    if d.model() != *model {
                        return false;
                    }
                }
                if let Some(vendor) = vendor_name {
                    if d.vendor() != *vendor {
                        return false;
                    }
                }
                if let Some(vendor) = vendor {
                    if d.pci_id().vendor != *vendor {
                        return false;
                    }
                }
                if let Some(model) = model {
                    if d.pci_id().model != *model {
                        return false;
                    }
                }
                true
            }
        }
    }
}

impl InputMatch {
    fn matches(&self, d: InputDevice, state: &State) -> bool {
        self.matches_(d, state, &mut AHashSet::new())
    }

    fn matches_<'a>(
        &'a self,
        d: InputDevice,
        state: &'a State,
        active: &mut AHashSet<&'a str>,
    ) -> bool {
        match self {
            InputMatch::Any(m) => m.iter().any(|m| m.matches_(d, state, active)),
            InputMatch::All {
                tag,
                name,
                syspath,
                devnode,
                is_keyboard,
                is_pointer,
                is_touch,
                is_tablet_tool,
                is_tablet_pad,
                is_gesture,
                is_switch,
            } => {
                if let Some(name) = name {
                    if d.name() != *name {
                        return false;
                    }
                }
                if let Some(tag) = tag {
                    let matches = apply_recursive_match(
                        "input device",
                        &state.input_devices,
                        active,
                        tag,
                        |m, active| m.matches_(d, state, active),
                    );
                    if !matches {
                        return false;
                    }
                }
                if let Some(syspath) = syspath {
                    if d.syspath() != *syspath {
                        return false;
                    }
                }
                if let Some(devnode) = devnode {
                    if d.devnode() != *devnode {
                        return false;
                    }
                }
                macro_rules! check_cap {
                    ($is:expr, $cap:ident) => {
                        if let Some(is) = *$is {
                            if d.has_capability(jay_config::input::capability::$cap) != is {
                                return false;
                            }
                        }
                    };
                }
                check_cap!(is_keyboard, CAP_KEYBOARD);
                check_cap!(is_pointer, CAP_POINTER);
                check_cap!(is_touch, CAP_TOUCH);
                check_cap!(is_tablet_tool, CAP_TABLET_TOOL);
                check_cap!(is_tablet_pad, CAP_TABLET_PAD);
                check_cap!(is_gesture, CAP_GESTURE);
                check_cap!(is_switch, CAP_SWITCH);
                true
            }
        }
    }
}

impl Input {
    fn apply(&self, c: InputDevice, state: &State) {
        if let Some(v) = self.accel_profile {
            c.set_accel_profile(v);
        }
        if let Some(v) = self.accel_speed {
            c.set_accel_speed(v);
        }
        if let Some(v) = self.tap_enabled {
            c.set_tap_enabled(v);
        }
        if let Some(v) = self.tap_drag_enabled {
            c.set_drag_enabled(v);
        }
        if let Some(v) = self.tap_drag_lock_enabled {
            c.set_drag_lock_enabled(v);
        }
        if let Some(v) = self.left_handed {
            c.set_left_handed(v);
        }
        if let Some(v) = self.natural_scrolling {
            c.set_natural_scrolling_enabled(v);
        }
        if let Some(v) = self.px_per_wheel_scroll {
            c.set_px_per_wheel_scroll(v);
        }
        if let Some(v) = self.transform_matrix {
            c.set_transform_matrix(v);
        }
        if let Some(v) = &self.keymap {
            if let Some(km) = state.get_keymap(v) {
                c.set_keymap(km);
            }
        }
        if let Some(output) = &self.output {
            if let Some(output) = output {
                for connector in connectors() {
                    if output.matches(connector, state) {
                        c.set_connector(connector);
                    }
                }
            } else {
                c.remove_mapping();
            }
        }
        if let Some(v) = self.calibration_matrix {
            c.set_calibration_matrix(v);
        }
    }
}

impl OutputMatch {
    fn matches(&self, c: Connector, state: &State) -> bool {
        if !c.connected() {
            return false;
        }
        self.matches_(c, state, &mut AHashSet::new())
    }

    fn matches_<'a>(
        &'a self,
        c: Connector,
        state: &'a State,
        active: &mut AHashSet<&'a str>,
    ) -> bool {
        match self {
            OutputMatch::Any(m) => m.iter().any(|m| m.matches_(c, state, active)),
            OutputMatch::All {
                name,
                connector,
                serial_number,
                manufacturer,
                model,
            } => {
                if let Some(name) = name {
                    let matches = apply_recursive_match(
                        "output",
                        &state.outputs,
                        active,
                        name,
                        |m, active| m.matches_(c, state, active),
                    );
                    if !matches {
                        return false;
                    }
                }
                if let Some(connector) = &connector {
                    if c.name() != *connector {
                        return false;
                    }
                }
                if let Some(serial_number) = &serial_number {
                    if c.serial_number() != *serial_number {
                        return false;
                    }
                }
                if let Some(manufacturer) = &manufacturer {
                    if c.manufacturer() != *manufacturer {
                        return false;
                    }
                }
                if let Some(model) = &model {
                    if c.model() != *model {
                        return false;
                    }
                }
                true
            }
        }
    }
}

impl ConnectorMatch {
    fn matches(&self, c: Connector) -> bool {
        if !c.exists() {
            return false;
        }
        match self {
            ConnectorMatch::Any(m) => m.iter().any(|m| m.matches(c)),
            ConnectorMatch::All { connector } => {
                if let Some(connector) = &connector {
                    if c.name() != *connector {
                        return false;
                    }
                }
                true
            }
        }
    }
}

impl ConfigConnector {
    fn apply(&self, c: Connector) {
        c.set_enabled(self.enabled);
    }
}

impl Output {
    fn apply(&self, c: Connector) {
        if self.x.is_some() || self.y.is_some() {
            let (old_x, old_y) = c.position();
            c.set_position(self.x.unwrap_or(old_x), self.y.unwrap_or(old_y));
        }
        if let Some(scale) = self.scale {
            c.set_scale(scale);
        }
        if let Some(transform) = self.transform {
            c.set_transform(transform);
        }
        if let Some(mode) = &self.mode {
            let modes = c.modes();
            let m = modes.iter().find(|m| {
                if m.width() != mode.width || m.height() != mode.height {
                    return false;
                }
                match mode.refresh_rate {
                    None => true,
                    Some(rr) => m.refresh_rate() as f64 / 1000.0 == rr,
                }
            });
            match m {
                None => {
                    log::warn!("Output {} does not support mode {mode}", c.name());
                }
                Some(m) => c.set_mode(m.width(), m.height(), Some(m.refresh_rate())),
            }
        }
        if let Some(vrr) = &self.vrr {
            if let Some(mode) = vrr.mode {
                c.set_vrr_mode(mode);
            }
            if let Some(hz) = vrr.cursor_hz {
                c.set_vrr_cursor_hz(hz);
            }
        }
        if let Some(tearing) = &self.tearing {
            if let Some(mode) = tearing.mode {
                c.set_tearing_mode(mode);
            }
        }
        if let Some(format) = self.format {
            c.set_format(format);
        }
        if self.color_space.is_some() || self.transfer_function.is_some() {
            let cs = self.color_space.unwrap_or(ColorSpace::DEFAULT);
            let tf = self.transfer_function.unwrap_or(TransferFunction::DEFAULT);
            c.set_colors(cs, tf);
        }
        if let Some(brightness) = self.brightness {
            c.set_brightness(brightness);
        }
    }
}

struct State {
    outputs: AHashMap<String, OutputMatch>,
    drm_devices: AHashMap<String, DrmDeviceMatch>,
    input_devices: AHashMap<String, InputMatch>,
    persistent: Rc<PersistentState>,
    keymaps: AHashMap<String, Keymap>,

    io_maps: Vec<(InputMatch, OutputMatch)>,
    io_inputs: RefCell<AHashMap<InputDevice, Vec<bool>>>,
    io_outputs: RefCell<AHashMap<Connector, Vec<bool>>>,

    action_depth_max: u64,
    action_depth: Cell<u64>,

    client: Cell<Option<Client>>,

    window: Cell<Option<Option<Window>>>,
}

impl Drop for State {
    fn drop(&mut self) {
        for keymap in self.keymaps.values() {
            keymap.destroy();
        }
    }
}

type SwitchActions = Vec<(InputMatch, AHashMap<SwitchEvent, Box<dyn Fn()>>)>;

impl State {
    fn unbind_all(&self) {
        let mut binds = self.persistent.binds.borrow_mut();
        for bind in binds.drain() {
            self.persistent.seat.unbind(bind);
        }
    }

    fn apply_shortcuts(self: &Rc<Self>, shortcuts: impl IntoIterator<Item = Shortcut>) {
        let mut binds = self.persistent.binds.borrow_mut();
        for shortcut in shortcuts {
            if let Action::SimpleCommand {
                cmd: SimpleCommand::None,
            } = shortcut.action
            {
                if shortcut.latch.is_none() {
                    self.persistent.seat.unbind(shortcut.keysym);
                    binds.remove(&shortcut.keysym);
                    continue;
                }
            }
            let mut f = shortcut.action.into_fn(self);
            if let Some(l) = shortcut.latch {
                let l = l.into_rc_fn(self);
                let s = self.persistent.seat;
                f = Box::new(move || {
                    f();
                    let l = l.clone();
                    s.latch(move || l());
                });
            }
            self.persistent
                .seat
                .bind_masked(shortcut.mask, shortcut.keysym, f);
            binds.insert(shortcut.keysym);
        }
    }

    fn get_keymap(&self, map: &ConfigKeymap) -> Option<Keymap> {
        let map = match map {
            ConfigKeymap::Named(n) => match self.keymaps.get(n) {
                None => {
                    log::warn!("Unknown keymap {n}");
                    return None;
                }
                Some(m) => *m,
            },
            ConfigKeymap::Defined { map, .. } => *map,
            ConfigKeymap::Literal(map) => *map,
        };
        Some(map)
    }

    fn set_keymap(&self, map: &ConfigKeymap) {
        if let Some(map) = self.get_keymap(map) {
            self.persistent.seat.set_keymap(map);
        }
    }

    fn set_status(&self, status: &Option<Status>) {
        set_status("");
        match status {
            None => unset_status_command(),
            Some(s) => {
                set_i3bar_separator(s.separator.as_deref().unwrap_or(" | "));
                set_status_command(s.format, create_command(&s.exec))
            }
        }
    }

    fn apply_theme(&self, theme: &Theme) {
        use jay_config::theme::{colors::*, sized::*};
        macro_rules! color {
            ($colorable:ident, $field:ident) => {
                if let Some(color) = theme.$field {
                    $colorable.set_color(color)
                }
            };
        }
        color!(
            ATTENTION_REQUESTED_BACKGROUND_COLOR,
            attention_requested_bg_color
        );
        color!(BACKGROUND_COLOR, bg_color);
        color!(BAR_BACKGROUND_COLOR, bar_bg_color);
        color!(BAR_STATUS_TEXT_COLOR, bar_status_text_color);
        color!(BORDER_COLOR, border_color);
        color!(
            CAPTURED_FOCUSED_TITLE_BACKGROUND_COLOR,
            captured_focused_title_bg_color
        );
        color!(
            CAPTURED_UNFOCUSED_TITLE_BACKGROUND_COLOR,
            captured_unfocused_title_bg_color
        );
        color!(
            FOCUSED_INACTIVE_TITLE_BACKGROUND_COLOR,
            focused_inactive_title_bg_color
        );
        color!(
            FOCUSED_INACTIVE_TITLE_TEXT_COLOR,
            focused_inactive_title_text_color
        );
        color!(FOCUSED_TITLE_BACKGROUND_COLOR, focused_title_bg_color);
        color!(FOCUSED_TITLE_TEXT_COLOR, focused_title_text_color);
        color!(SEPARATOR_COLOR, separator_color);
        color!(UNFOCUSED_TITLE_BACKGROUND_COLOR, unfocused_title_bg_color);
        color!(UNFOCUSED_TITLE_TEXT_COLOR, unfocused_title_text_color);
        color!(HIGHLIGHT_COLOR, highlight_color);
        macro_rules! size {
            ($sized:ident, $field:ident) => {
                if let Some(size) = theme.$field {
                    $sized.set(size);
                }
            };
        }
        size!(BORDER_WIDTH, border_width);
        size!(TITLE_HEIGHT, title_height);
        if let Some(font) = &theme.font {
            set_font(font);
        }
    }

    fn handle_switch_device(self: &Rc<Self>, dev: InputDevice, actions: &Rc<SwitchActions>) {
        if !dev.has_capability(CAP_SWITCH) {
            return;
        }
        let state = self.clone();
        let actions = actions.clone();
        dev.on_switch_event(move |ev| {
            for (match_, actions) in &*actions {
                if match_.matches(dev, &state) {
                    if let Some(action) = actions.get(&ev) {
                        action();
                    }
                }
            }
        });
    }

    fn add_io_output(&self, c: Connector) {
        let mappings: Vec<_> = self
            .io_maps
            .iter()
            .map(|(_, output)| output.matches(c, self))
            .collect();
        if mappings.len() > 0 {
            self.io_outputs.borrow_mut().insert(c, mappings);
        }
    }

    fn add_io_input(&self, d: InputDevice) {
        let mappings: Vec<_> = self
            .io_maps
            .iter()
            .map(|(input, _)| input.matches(d, self))
            .collect();
        if mappings.len() > 0 {
            self.io_inputs.borrow_mut().insert(d, mappings);
        }
    }

    fn map_input_to_output(&self, d: InputDevice) {
        let input_mappings = &*self.io_inputs.borrow();
        let Some(input_matches) = input_mappings.get(&d) else {
            return;
        };
        for (idx, &input_is_match) in input_matches.iter().enumerate() {
            if input_is_match {
                for (&c, output_maps) in &*self.io_outputs.borrow() {
                    if output_maps.get(idx) == Some(&true) {
                        d.set_connector(c);
                    }
                }
            }
        }
    }

    fn map_output_to_input(&self, c: Connector) {
        let output_mappings = &*self.io_outputs.borrow();
        let Some(output_matches) = output_mappings.get(&c) else {
            return;
        };
        for (idx, &output_is_match) in output_matches.iter().enumerate() {
            if output_is_match {
                for (&d, input_matches) in &*self.io_inputs.borrow() {
                    if input_matches.get(idx) == Some(&true) {
                        d.set_connector(c);
                    }
                }
            }
        }
    }

    fn with_client(&self, client: Client, check: bool, f: impl FnOnce()) {
        let mut opt = Some(client);
        if client.0 == 0 || (check && client.does_not_exist()) {
            opt = None;
        }
        self.client.set(opt);
        f();
        self.client.set(None);
    }

    fn with_window(&self, window: Window, check: bool, f: impl FnOnce()) {
        let mut w = Some(window);
        if check && !window.exists() {
            w = None;
        }
        self.window.set(Some(w));
        f();
        self.window.set(None);
    }
}

#[derive(Eq, PartialEq, Hash)]
struct OutputId {
    manufacturer: String,
    model: String,
    serial_number: String,
}

struct PersistentState {
    seen_outputs: RefCell<AHashSet<OutputId>>,
    default: Config,
    seat: Seat,
    binds: RefCell<AHashSet<ModifiedKeySym>>,
    #[expect(clippy::type_complexity)]
    actions: RefCell<AHashMap<Rc<String>, Rc<dyn Fn()>>>,
    client_rules: Cell<Vec<MatcherTemp<ClientRule>>>,
    client_rule_mapper: RefCell<Option<RuleMapper<ClientRule>>>,
    window_rules: Cell<Vec<MatcherTemp<WindowRule>>>,
}

fn load_config(initial_load: bool, persistent: &Rc<PersistentState>) {
    let mut path = PathBuf::from(config_dir());
    path.push("config.toml");
    let mut config = match std::fs::read(&path) {
        Ok(input) => match parse_config(&input, |e| {
            log::warn!("Error while parsing {}: {}", path.display(), Report::new(e))
        }) {
            None if initial_load => {
                log::warn!("Using default config instead");
                persistent.default.clone()
            }
            None => {
                log::warn!("Ignoring config reload");
                return;
            }
            Some(c) => c,
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            log::info!("{} does not exist. Using default config.", path.display());
            persistent.default.clone()
        }
        Err(e) => {
            log::warn!("Could not load {}: {}", path.display(), Report::new(e));
            log::warn!("Ignoring config reload");
            return;
        }
    };
    let mut outputs = AHashMap::new();
    for output in &config.outputs {
        if let Some(name) = &output.name {
            let prev = outputs.insert(name.clone(), output.match_.clone());
            if prev.is_some() {
                log::warn!("Duplicate output name {name}");
            }
        }
    }
    let mut keymaps = AHashMap::new();
    for keymap in config.keymaps {
        match keymap {
            ConfigKeymap::Defined { name, map } => {
                keymaps.insert(name, map);
            }
            _ => log::warn!("Keymap is not in defined form in top-level context"),
        }
    }
    let mut input_devices = AHashMap::new();
    let mut io_maps = vec![];
    for input in &config.inputs {
        if let Some(tag) = &input.tag {
            let prev = input_devices.insert(tag.clone(), input.match_.clone());
            if prev.is_some() {
                log::warn!("Duplicate input tag {tag}");
            }
        }
        if let Some(Some(output)) = &input.output {
            io_maps.push((input.match_.clone(), output.clone()));
        }
    }
    let mut named_drm_device = AHashMap::new();
    for drm_device in &config.drm_devices {
        if let Some(name) = &drm_device.name {
            let prev = named_drm_device.insert(name.clone(), drm_device.match_.clone());
            if prev.is_some() {
                log::warn!("Duplicate drm device name {name}");
            }
        }
    }
    let state = Rc::new(State {
        outputs,
        drm_devices: named_drm_device,
        input_devices,
        persistent: persistent.clone(),
        keymaps,
        io_maps,
        io_inputs: Default::default(),
        io_outputs: Default::default(),
        action_depth_max: config.max_action_depth,
        action_depth: Cell::new(0),
        client: Default::default(),
        window: Default::default(),
    });
    let (client_rules, client_rule_mapper) = state.create_rules(&config.client_rules);
    persistent.client_rules.set(client_rules);
    *state.persistent.client_rule_mapper.borrow_mut() = Some(client_rule_mapper);
    let (window_rules, _) = state.create_rules(&config.window_rules);
    persistent.window_rules.set(window_rules);
    state.set_status(&config.status);
    persistent.actions.borrow_mut().clear();
    for a in config.named_actions {
        let action = a.action.into_rc_fn(&state);
        persistent.actions.borrow_mut().insert(a.name, action);
    }
    let mut switch_actions = vec![];
    for input in &mut config.inputs {
        let mut actions = AHashMap::new();
        for (event, action) in input.switch_actions.drain() {
            actions.insert(event, action.into_fn(&state));
        }
        if actions.len() > 0 {
            switch_actions.push((input.match_.clone(), actions));
        }
    }
    let switch_actions = Rc::new(switch_actions);
    match config.on_graphics_initialized {
        None => on_graphics_initialized(|| ()),
        Some(a) => on_graphics_initialized(a.into_fn(&state)),
    }
    match config.on_idle {
        None => on_idle(|| ()),
        Some(a) => on_idle(a.into_fn(&state)),
    }
    state.unbind_all();
    state.apply_shortcuts(config.shortcuts);
    if let Some(keymap) = config.keymap {
        state.set_keymap(&keymap);
    }
    if let Some(repeat_rate) = config.repeat_rate {
        persistent
            .seat
            .set_repeat_rate(repeat_rate.rate, repeat_rate.delay);
    }
    on_new_connector(move |c| {
        for connector in &config.connectors {
            if connector.match_.matches(c) {
                connector.apply(c);
            }
        }
    });
    on_connector_connected({
        let state = state.clone();
        move |c| {
            state.add_io_output(c);
            state.map_output_to_input(c);
            let id = OutputId {
                manufacturer: c.manufacturer(),
                model: c.model(),
                serial_number: c.serial_number(),
            };
            if state.persistent.seen_outputs.borrow_mut().insert(id) {
                for output in &config.outputs {
                    if output.match_.matches(c, &state) {
                        output.apply(c);
                    }
                }
            }
        }
    });
    on_connector_disconnected({
        let state = state.clone();
        move |c| {
            state.io_outputs.borrow_mut().remove(&c);
        }
    });
    set_default_workspace_capture(config.workspace_capture);
    for (k, v) in config.env {
        set_env(&k, &v);
    }
    if initial_load && !is_reload() {
        if let Some(on_startup) = config.on_startup {
            on_startup.into_fn(&state)();
        }
        if let Some(level) = config.log_level {
            set_log_level(level);
        }
        if let Some(idle) = config.idle {
            set_idle(Some(idle));
        }
        if let Some(period) = config.grace_period {
            set_idle_grace_period(period);
        }
    }
    on_devices_enumerated({
        let state = state.clone();
        move || {
            if let Some(dev) = config.render_device {
                for d in drm_devices() {
                    if dev.matches(d, &state) {
                        d.make_render_device();
                        return;
                    }
                }
            }
        }
    });
    reset_colors();
    reset_font();
    reset_sizes();
    state.apply_theme(&config.theme);
    if let Some(api) = config.gfx_api {
        set_gfx_api(api);
    }
    if let Some(dse) = config.direct_scanout_enabled {
        set_direct_scanout_enabled(dse);
    }
    if let Some(ese) = config.explicit_sync_enabled {
        set_explicit_sync_enabled(ese);
    }
    on_new_drm_device({
        let state = state.clone();
        move |d| {
            for dev in &config.drm_devices {
                if dev.match_.matches(d, &state) {
                    dev.apply(d);
                }
            }
        }
    });
    on_new_input_device({
        let state = state.clone();
        let switch_actions = switch_actions.clone();
        move |c| {
            state.add_io_input(c);
            for input in &config.inputs {
                if input.match_.matches(c, &state) {
                    input.apply(c, &state);
                }
            }
            state.handle_switch_device(c, &switch_actions);
        }
    });
    on_input_device_removed({
        let state = state.clone();
        move |c| {
            state.io_inputs.borrow_mut().remove(&c);
        }
    });
    for c in connectors() {
        state.add_io_output(c);
    }
    for c in jay_config::input::input_devices() {
        state.add_io_input(c);
        state.map_input_to_output(c);
        state.handle_switch_device(c, &switch_actions);
    }
    persistent
        .seat
        .set_focus_follows_mouse_mode(match config.focus_follows_mouse {
            true => FocusFollowsMouseMode::True,
            false => FocusFollowsMouseMode::False,
        });
    if let Some(window_management_key) = config.window_management_key {
        persistent
            .seat
            .set_window_management_key(window_management_key);
    }
    if let Some(vrr) = config.vrr {
        if let Some(mode) = vrr.mode {
            set_vrr_mode(mode);
        }
        if let Some(hz) = vrr.cursor_hz {
            set_vrr_cursor_hz(hz);
        }
    }
    if let Some(tearing) = config.tearing {
        if let Some(mode) = tearing.mode {
            set_tearing_mode(mode);
        }
    }
    set_libei_socket_enabled(config.libei.enable_socket.unwrap_or(false));
    if let Some(enabled) = config.ui_drag.enabled {
        set_ui_drag_enabled(enabled);
    }
    if let Some(threshold) = config.ui_drag.threshold {
        set_ui_drag_threshold(threshold);
    }
    if let Some(xwayland) = config.xwayland {
        if let Some(mode) = xwayland.scaling_mode {
            set_x_scaling_mode(mode);
        }
    }
    if let Some(cm) = config.color_management {
        if let Some(enabled) = cm.enabled {
            set_color_management_enabled(enabled);
        }
    }
    if let Some(float) = config.float {
        if let Some(show) = float.show_pin_icon {
            set_show_float_pin_icon(show);
        }
    }
}

fn create_command(exec: &Exec) -> Command {
    let mut command = Command::new(&exec.prog);
    for arg in &exec.args {
        command.arg(arg);
    }
    for (k, v) in &exec.envs {
        command.env(k, v);
    }
    if exec.privileged {
        command.privileged();
    }
    command
}

const DEFAULT: &[u8] = include_bytes!("default-config.toml");

pub fn configure() {
    let default = parse_config(DEFAULT, |e| {
        panic!("Could not parse the default config: {}", Report::new(e))
    });
    let persistent = Rc::new(PersistentState {
        seen_outputs: Default::default(),
        default: default.unwrap(),
        seat: default_seat(),
        binds: Default::default(),
        actions: Default::default(),
        client_rules: Default::default(),
        client_rule_mapper: Default::default(),
        window_rules: Default::default(),
    });
    {
        let p = persistent.clone();
        on_unload(move || {
            p.actions.borrow_mut().clear();
            p.client_rule_mapper.borrow_mut().take();
        });
    }
    load_config(true, &persistent);
}

config!(configure);
