#![allow(
    clippy::len_zero,
    clippy::single_char_pattern,
    clippy::collapsible_if,
    clippy::collapsible_else_if
)]

mod config;
mod rules;
mod shortcuts;
mod toml;

use {
    crate::{
        config::{
            Action, ClientRule, Config, ConfigConnector, ConfigDrmDevice, ConfigKeymap,
            ConnectorMatch, DrmDeviceMatch, Exec, Input, InputMatch, Output, OutputMatch,
            SimpleCommand, Status, Theme, WindowRule, parse_config,
        },
        rules::{MatcherTemp, RuleMapper},
        shortcuts::ModeState,
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
        io::Async,
        is_reload,
        keyboard::Keymap,
        logging::set_log_level,
        on_devices_enumerated, on_idle, on_unload, quit, reload, set_color_management_enabled,
        set_default_workspace_capture, set_explicit_sync_enabled, set_float_above_fullscreen,
        set_idle, set_idle_grace_period, set_middle_click_paste_enabled, set_show_bar,
        set_show_float_pin_icon, set_show_titles, set_ui_drag_enabled, set_ui_drag_threshold,
        status::{set_i3bar_separator, set_status, set_status_command, unset_status_command},
        switch_to_vt,
        tasks::{self, JoinHandle},
        theme::{reset_colors, reset_font, reset_sizes, set_bar_font, set_font, set_title_font},
        toggle_float_above_fullscreen, toggle_show_bar, toggle_show_titles,
        video::{
            ColorSpace, Connector, DrmDevice, Eotf, connectors, drm_devices,
            on_connector_connected, on_connector_disconnected, on_graphics_initialized,
            on_new_connector, on_new_drm_device, set_direct_scanout_enabled, set_gfx_api,
            set_tearing_mode, set_vrr_cursor_hz, set_vrr_mode,
        },
        window::Window,
        workspace::set_workspace_display_order,
        xwayland::set_x_scaling_mode,
    },
    run_on_drop::on_drop,
    std::{
        cell::{Cell, RefCell},
        ffi::OsStr,
        io::ErrorKind,
        os::{fd::AsRawFd, unix::ffi::OsStrExt},
        path::{Path, PathBuf},
        rc::Rc,
        time::Duration,
    },
    uapi::{
        Errno,
        c::{
            self, CLOCK_MONOTONIC, IN_ATTRIB, IN_CLOEXEC, IN_CLOSE_WRITE, IN_CREATE, IN_DELETE,
            IN_EXCL_UNLINK, IN_MOVED_FROM, IN_MOVED_TO, IN_NONBLOCK, IN_ONLYDIR, TFD_CLOEXEC,
            TFD_NONBLOCK, timespec,
        },
    },
};

fn default_seat() -> Seat {
    get_seat("default")
}

trait FnBuilder: Sized {
    type Output;

    #[expect(clippy::wrong_self_convention)]
    fn new<F: Fn() + 'static>(&self, f: F) -> Self::Output;
}

struct BoxFnBuilder;

impl FnBuilder for BoxFnBuilder {
    type Output = Box<dyn Fn()>;

    fn new<F: Fn() + 'static>(&self, f: F) -> Self::Output {
        Box::new(f)
    }
}

struct RcFnBuilder;

impl FnBuilder for RcFnBuilder {
    type Output = Rc<dyn Fn()>;

    fn new<F: Fn() + 'static>(&self, f: F) -> Self::Output {
        Rc::new(f)
    }
}

struct ShortcutFnBuilder<'a>(&'a Rc<State>);

impl FnBuilder for ShortcutFnBuilder<'_> {
    type Output = Rc<dyn Fn()>;

    fn new<F: Fn() + 'static>(&self, f: F) -> Self::Output {
        let state = self.0.clone();
        Rc::new(move || {
            state.cancel_mode_latch();
            f();
        })
    }
}

impl Action {
    fn into_fn(self, state: &Rc<State>) -> Box<dyn Fn()> {
        self.into_fn_impl(&BoxFnBuilder, state)
    }

    fn into_rc_fn(self, state: &Rc<State>) -> Rc<dyn Fn()> {
        self.into_fn_impl(&RcFnBuilder, state)
    }

    fn into_shortcut_fn(self, state: &Rc<State>) -> Rc<dyn Fn()> {
        self.into_fn_impl(&ShortcutFnBuilder(state), state)
    }

    fn into_fn_impl<B: FnBuilder>(self, b: &B, state: &Rc<State>) -> B::Output {
        macro_rules! client_action {
            ($name:ident, $opt:expr) => {{
                let state = state.clone();
                b.new(move || {
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
                b.new(move || {
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
                SimpleCommand::Focus(dir) => b.new(move || s.focus(dir)),
                SimpleCommand::Move(dir) => window_or_seat!(s, s.move_(dir)),
                SimpleCommand::Split(axis) => window_or_seat!(s, s.create_split(axis)),
                SimpleCommand::ToggleSplit => window_or_seat!(s, s.toggle_split()),
                SimpleCommand::SetSplit(b) => window_or_seat!(s, s.set_split(b)),
                SimpleCommand::ToggleMono => window_or_seat!(s, s.toggle_mono()),
                SimpleCommand::SetMono(b) => window_or_seat!(s, s.set_mono(b)),
                SimpleCommand::ToggleFullscreen => window_or_seat!(s, s.toggle_fullscreen()),
                SimpleCommand::SetFullscreen(b) => window_or_seat!(s, s.set_fullscreen(b)),
                SimpleCommand::FocusParent => b.new(move || s.focus_parent()),
                SimpleCommand::Close => window_or_seat!(s, s.close()),
                SimpleCommand::DisablePointerConstraint => {
                    b.new(move || s.disable_pointer_constraint())
                }
                SimpleCommand::ToggleFloating => window_or_seat!(s, s.toggle_floating()),
                SimpleCommand::SetFloating(b) => window_or_seat!(s, s.set_floating(b)),
                SimpleCommand::Quit => b.new(quit),
                SimpleCommand::ReloadConfigToml => {
                    let persistent = state.persistent.clone();
                    b.new(move || load_config(false, false, &persistent))
                }
                SimpleCommand::ReloadConfigSo => b.new(reload),
                SimpleCommand::None => b.new(|| ()),
                SimpleCommand::Forward(bool) => b.new(move || s.set_forward(bool)),
                SimpleCommand::EnableWindowManagement(bool) => {
                    b.new(move || s.set_window_management_enabled(bool))
                }
                SimpleCommand::SetFloatAboveFullscreen(bool) => {
                    b.new(move || set_float_above_fullscreen(bool))
                }
                SimpleCommand::ToggleFloatAboveFullscreen => b.new(toggle_float_above_fullscreen),
                SimpleCommand::SetFloatPinned(pinned) => {
                    window_or_seat!(s, s.set_float_pinned(pinned))
                }
                SimpleCommand::ToggleFloatPinned => window_or_seat!(s, s.toggle_float_pinned()),
                SimpleCommand::KillClient => client_action!(c, c.kill()),
                SimpleCommand::ShowBar(show) => b.new(move || set_show_bar(show)),
                SimpleCommand::ToggleBar => b.new(toggle_show_bar),
                SimpleCommand::ShowTitles(show) => b.new(move || set_show_titles(show)),
                SimpleCommand::ToggleTitles => b.new(toggle_show_titles),
                SimpleCommand::FocusHistory(timeline) => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.focus_history(timeline))
                }
                SimpleCommand::FocusLayerRel(direction) => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.focus_layer_rel(direction))
                }
                SimpleCommand::FocusTiles => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.focus_tiles())
                }
                SimpleCommand::CreateMark => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.create_mark(None))
                }
                SimpleCommand::JumpToMark => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.jump_to_mark(None))
                }
                SimpleCommand::PopMode(pop) => {
                    let state = state.clone();
                    b.new(move || state.pop_mode(pop))
                }
                SimpleCommand::EnableSimpleIm(v) => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.set_simple_im_enabled(v))
                }
                SimpleCommand::ToggleSimpleImEnabled => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.toggle_simple_im_enabled())
                }
                SimpleCommand::ReloadSimpleIm => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.reload_simple_im())
                }
                SimpleCommand::EnableUnicodeInput => {
                    let persistent = state.persistent.clone();
                    b.new(move || persistent.seat.enable_unicode_input())
                }
            },
            Action::Multi { actions } => {
                let actions: Vec<_> = actions.into_iter().map(|a| a.into_fn(state)).collect();
                b.new(move || {
                    for action in &actions {
                        action();
                    }
                })
            }
            Action::Exec { exec } => b.new(move || create_command(&exec).spawn()),
            Action::SwitchToVt { num } => b.new(move || switch_to_vt(num)),
            Action::ShowWorkspace { name, output } => {
                let workspace = get_workspace(&name);
                let state = state.clone();
                b.new(move || {
                    let output = 'get_output: {
                        let Some(output) = &output else {
                            break 'get_output None;
                        };
                        for connector in connectors() {
                            if connector.connected() && output.matches(connector, &state) {
                                break 'get_output Some(connector);
                            }
                        }
                        None
                    };
                    match output {
                        Some(o) => s.show_workspace_on(workspace, o),
                        _ => s.show_workspace(workspace),
                    }
                })
            }
            Action::MoveToWorkspace { name } => {
                let workspace = get_workspace(&name);
                window_or_seat!(s, s.set_workspace(workspace))
            }
            Action::ConfigureConnector { con } => b.new(move || {
                for c in connectors() {
                    if con.match_.matches(c) {
                        con.apply(c);
                    }
                }
            }),
            Action::ConfigureInput { input } => {
                let state = state.clone();
                b.new(move || {
                    for c in input_devices() {
                        if input.match_.matches(c, &state) {
                            input.apply(c, &state);
                        }
                    }
                })
            }
            Action::ConfigureOutput { out } => {
                let state = state.clone();
                b.new(move || {
                    for c in connectors() {
                        if out.match_.matches(c, &state) {
                            out.apply(c);
                        }
                    }
                })
            }
            Action::SetEnv { env } => b.new(move || {
                for (k, v) in &env {
                    set_env(k, v);
                }
            }),
            Action::UnsetEnv { env } => b.new(move || {
                for k in &env {
                    unset_env(k);
                }
            }),
            Action::SetKeymap { map } => {
                let state = state.clone();
                b.new(move || state.set_keymap(&map))
            }
            Action::SetStatus { status } => {
                let state = state.clone();
                b.new(move || state.set_status(&status))
            }
            Action::SetTheme { theme } => {
                let state = state.clone();
                b.new(move || state.apply_theme(&theme))
            }
            Action::SetLogLevel { level } => b.new(move || set_log_level(level)),
            Action::SetGfxApi { api } => b.new(move || set_gfx_api(api)),
            Action::ConfigureDirectScanout { enabled } => {
                b.new(move || set_direct_scanout_enabled(enabled))
            }
            Action::ConfigureDrmDevice { dev } => {
                let state = state.clone();
                b.new(move || {
                    for d in drm_devices() {
                        if dev.match_.matches(d, &state) {
                            dev.apply(d);
                        }
                    }
                })
            }
            Action::SetRenderDevice { dev } => {
                let state = state.clone();
                b.new(move || {
                    for d in drm_devices() {
                        if dev.matches(d, &state) {
                            d.make_render_device();
                        }
                    }
                })
            }
            Action::ConfigureIdle { idle, grace_period } => b.new(move || {
                if let Some(idle) = idle {
                    set_idle(Some(idle))
                }
                if let Some(period) = grace_period {
                    set_idle_grace_period(period)
                }
            }),
            Action::MoveToOutput {
                output,
                workspace,
                direction,
            } => {
                let state = state.clone();
                b.new(move || {
                    let target_output = {
                        // Handle directional output selection
                        if let Some(direction) = direction {
                            // Get the current workspace to determine the source output
                            let current_ws = match workspace {
                                Some(ws) => ws,
                                None => s.get_workspace(),
                            };
                            if !current_ws.exists() {
                                return;
                            }
                            // Get the connector that currently has this workspace
                            let source_connector = current_ws.connector();
                            if !source_connector.exists() {
                                return;
                            }
                            // Find the connector in the given direction
                            let target = source_connector.connector_in_direction(direction);
                            if !target.exists() {
                                return;
                            }
                            target
                        } else if let Some(output) = &output {
                            // Handle normal output matching
                            'match_output: {
                                for connector in connectors() {
                                    if connector.connected() && output.matches(connector, &state) {
                                        break 'match_output connector;
                                    }
                                }
                                return;
                            }
                        } else {
                            return;
                        }
                    };
                    match workspace {
                        Some(ws) => ws.move_to_output(target_output),
                        None => s.move_to_output(target_output),
                    }
                })
            }
            Action::SetRepeatRate { rate } => {
                b.new(move || s.set_repeat_rate(rate.rate, rate.delay))
            }
            Action::DefineAction { name, action } => {
                let state = state.clone();
                let action = action.into_rc_fn(&state);
                let name = Rc::new(name);
                b.new(move || {
                    state
                        .persistent
                        .actions
                        .borrow_mut()
                        .insert(name.clone(), action.clone());
                })
            }
            Action::UndefineAction { name } => {
                let state = state.clone();
                b.new(move || {
                    state.persistent.actions.borrow_mut().remove(&name);
                })
            }
            Action::NamedAction { name } => {
                let state = state.clone();
                b.new(move || {
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
            Action::CreateMark(m) => {
                let persistent = state.persistent.clone();
                b.new(move || persistent.seat.create_mark(Some(m)))
            }
            Action::JumpToMark(m) => {
                let persistent = state.persistent.clone();
                b.new(move || persistent.seat.jump_to_mark(Some(m)))
            }
            Action::CopyMark(s, d) => {
                let persistent = state.persistent.clone();
                b.new(move || persistent.seat.copy_mark(s, d))
            }
            Action::SetMode { name, latch } => {
                let state = state.clone();
                let new = state.get_mode_slot(&name);
                b.new(move || {
                    let new = new.mode.borrow();
                    let Some(new) = new.as_ref() else {
                        log::warn!("Input mode {name} does not exist");
                        return;
                    };
                    state.set_mode(new, latch);
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
                if let Some(syspath) = syspath
                    && d.syspath() != *syspath
                {
                    return false;
                }
                if let Some(devnode) = devnode
                    && d.devnode() != *devnode
                {
                    return false;
                }
                if let Some(model) = model_name
                    && d.model() != *model
                {
                    return false;
                }
                if let Some(vendor) = vendor_name
                    && d.vendor() != *vendor
                {
                    return false;
                }
                if let Some(vendor) = vendor
                    && d.pci_id().vendor != *vendor
                {
                    return false;
                }
                if let Some(model) = model
                    && d.pci_id().model != *model
                {
                    return false;
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
                if let Some(name) = name
                    && d.name() != *name
                {
                    return false;
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
                if let Some(syspath) = syspath
                    && d.syspath() != *syspath
                {
                    return false;
                }
                if let Some(devnode) = devnode
                    && d.devnode() != *devnode
                {
                    return false;
                }
                macro_rules! check_cap {
                    ($is:expr, $cap:ident) => {
                        if let Some(is) = *$is
                            && d.has_capability(jay_config::input::capability::$cap) != is
                        {
                            return false;
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
        if let Some(v) = &self.keymap
            && let Some(km) = state.get_keymap(v)
        {
            c.set_keymap(km);
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
        if let Some(v) = self.click_method {
            c.set_click_method(v);
        }
        if let Some(v) = self.middle_button_emulation {
            c.set_middle_button_emulation_enabled(v);
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
                if let Some(connector) = &connector
                    && c.name() != *connector
                {
                    return false;
                }
                if let Some(serial_number) = &serial_number
                    && c.serial_number() != *serial_number
                {
                    return false;
                }
                if let Some(manufacturer) = &manufacturer
                    && c.manufacturer() != *manufacturer
                {
                    return false;
                }
                if let Some(model) = &model
                    && c.model() != *model
                {
                    return false;
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
                if let Some(connector) = &connector
                    && c.name() != *connector
                {
                    return false;
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
        if let Some(tearing) = &self.tearing
            && let Some(mode) = tearing.mode
        {
            c.set_tearing_mode(mode);
        }
        if let Some(format) = self.format {
            c.set_format(format);
        }
        if self.color_space.is_some() || self.eotf.is_some() {
            let cs = self.color_space.unwrap_or(ColorSpace::DEFAULT);
            let tf = self.eotf.unwrap_or(Eotf::DEFAULT);
            c.set_colors(cs, tf);
        }
        if let Some(brightness) = self.brightness {
            c.set_brightness(brightness);
        }
        if let Some(bs) = self.blend_space {
            c.set_blend_space(bs);
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
        size!(BAR_HEIGHT, bar_height);
        macro_rules! font {
            ($fun:ident, $field:ident) => {
                if let Some(font) = &theme.$field {
                    $fun(font);
                }
            };
        }
        font!(set_font, font);
        font!(set_title_font, title_font);
        font!(set_bar_font, bar_font);
    }

    fn handle_switch_device(self: &Rc<Self>, dev: InputDevice, actions: &Rc<SwitchActions>) {
        if !dev.has_capability(CAP_SWITCH) {
            return;
        }
        let state = self.clone();
        let actions = actions.clone();
        dev.on_switch_event(move |ev| {
            for (match_, actions) in &*actions {
                if match_.matches(dev, &state)
                    && let Some(action) = actions.get(&ev)
                {
                    action();
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
    #[expect(clippy::type_complexity)]
    actions: RefCell<AHashMap<Rc<String>, Rc<dyn Fn()>>>,
    client_rules: Cell<Vec<MatcherTemp<ClientRule>>>,
    client_rule_mapper: RefCell<Option<RuleMapper<ClientRule>>>,
    window_rules: Cell<Vec<MatcherTemp<WindowRule>>>,
    mark_names: RefCell<AHashMap<String, u32>>,
    mode_state: ModeState,
    watcher_handle: RefCell<Option<JoinHandle<()>>>,
    last_config: RefCell<Option<Vec<u8>>>,
}

async fn watch_config(persistent: Rc<PersistentState>) {
    let inotify = match uapi::inotify_init1(IN_NONBLOCK | IN_CLOEXEC) {
        Ok(i) => i,
        Err(e) => {
            log::error!("Could not create inotify fd: {}", Report::new(e));
            return;
        }
    };
    let inotify_async = match Async::new(&inotify) {
        Ok(i) => i,
        Err(e) => {
            log::error!(
                "Could not create Async object for inotify fd: {}",
                Report::new(e)
            );
            return;
        }
    };

    let timer = match uapi::timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK | TFD_CLOEXEC) {
        Ok(t) => Rc::new(t),
        Err(e) => {
            log::error!("Could not create timer fd: {}", Report::new(e));
            return;
        }
    };
    let timer_async = match Async::new(timer.clone()) {
        Ok(i) => i,
        Err(e) => {
            log::error!(
                "Could not create Async object for timer fd: {}",
                Report::new(e)
            );
            return;
        }
    };

    let timer_task = tasks::spawn(async move {
        loop {
            if let Err(e) = timer_async.readable().await {
                log::error!(
                    "Could not wait for timer to become readable: {}",
                    Report::new(e),
                );
                return;
            }
            let mut buf = 0u64;
            if let Err(e) = uapi::read(timer_async.as_ref().raw(), &mut buf) {
                log::error!("Could not read from timer fd: {}", Report::new(e));
                return;
            }
            load_config(false, true, &persistent);
        }
    });
    let _cancel_task = on_drop(|| timer_task.abort());

    let program_timer = || {
        let new_value = c::itimerspec {
            it_interval: timespec {
                tv_nsec: 0,
                tv_sec: 0,
            },
            it_value: timespec {
                tv_nsec: 400_000_000,
                tv_sec: 0,
            },
        };
        if let Err(e) = uapi::timerfd_settime(timer.raw(), 0, &new_value) {
            log::error!("Could not set timer: {}", Report::new(e));
        }
    };

    let config_dir = config_dir();
    let config_dir = Path::new(&config_dir);
    let mut dirs = vec![];
    for component in config_dir.components() {
        dirs.push(component.as_os_str());
    }

    let mut dir_watches = vec![];
    let mut file_watch = None;

    let mut path_buf = PathBuf::new();
    let mut create_watches = |dir_watches: &mut Vec<c::c_int>,
                              file_watch: &mut Option<c::c_int>| {
        path_buf.clear();
        for (i, dir) in dirs.iter().enumerate() {
            path_buf.push(dir);
            if dir_watches.len() > i {
                continue;
            }
            let res = uapi::inotify_add_watch(
                inotify.raw(),
                &*path_buf,
                IN_ONLYDIR
                    | IN_CREATE
                    | IN_DELETE
                    | IN_MOVED_FROM
                    | IN_MOVED_TO
                    | IN_ATTRIB
                    | IN_EXCL_UNLINK,
            );
            let Ok(n) = res else {
                return;
            };
            dir_watches.push(n);
        }
        if file_watch.is_none() {
            path_buf.push(CONFIG_TOML);
            let res =
                uapi::inotify_add_watch(inotify.raw(), &*path_buf, IN_CLOSE_WRITE | IN_EXCL_UNLINK);
            *file_watch = res.ok();
        }
    };
    macro_rules! create_watches {
        () => {
            create_watches(&mut dir_watches, &mut file_watch);
            program_timer();
        };
    }
    create_watches!();

    let mut buffer = vec![0; 1024];
    loop {
        let res = uapi::inotify_read(inotify_async.as_ref().as_raw_fd(), &mut *buffer);
        let events = match res {
            Ok(e) => e,
            Err(Errno(c::EAGAIN)) => {
                inotify_async.readable().await.unwrap();
                continue;
            }
            Err(e) => {
                log::error!("Could not read from inotify fd: {}", e);
                return;
            }
        };
        for event in events {
            if Some(event.wd) == file_watch {
                program_timer();
            } else {
                for i in 0..dir_watches.len() {
                    if event.wd != dir_watches[i] {
                        continue;
                    }
                    let next = if i + 1 == dirs.len() {
                        OsStr::new(CONFIG_TOML)
                    } else {
                        dirs[i + 1]
                    };
                    if event.name().to_bytes() != next.as_bytes() {
                        break;
                    }
                    if event.mask & (IN_DELETE | IN_MOVED_FROM) != 0 {
                        for wd in dir_watches.drain(i + 1..) {
                            let _ = uapi::inotify_rm_watch(inotify.raw(), wd);
                        }
                        if let Some(wd) = file_watch.take() {
                            let _ = uapi::inotify_rm_watch(inotify.raw(), wd);
                        }
                        program_timer();
                    }
                    if (event.mask & IN_ATTRIB != 0
                        && i + 1 == dir_watches.len()
                        && file_watch.is_none())
                        || event.mask & (IN_CREATE | IN_MOVED_TO) != 0
                    {
                        create_watches!();
                    }
                    break;
                }
            }
        }
    }
}

const CONFIG_TOML: &str = "config.toml";

fn load_config(initial_load: bool, auto_reload: bool, persistent: &Rc<PersistentState>) {
    let mut path = PathBuf::from(config_dir());
    path.push(CONFIG_TOML);
    let mut last_config = persistent.last_config.borrow_mut();
    let mut config = match std::fs::read(&path) {
        Ok(input) => {
            if auto_reload {
                if Some(&input) == last_config.as_ref() {
                    return;
                }
                log::info!("Auto reloading config")
            }
            let parsed = parse_config(&input, &persistent.mark_names, |e| {
                log::warn!("Error while parsing {}: {}", path.display(), Report::new(e))
            });
            *last_config = Some(input);
            match parsed {
                None if initial_load => {
                    log::warn!("Using default config instead");
                    persistent.default.clone()
                }
                None => {
                    log::warn!("Ignoring config reload");
                    return;
                }
                Some(c) => c,
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            if auto_reload {
                if last_config.take().is_none() {
                    return;
                }
                log::info!("Auto reloading config")
            }
            log::info!("{} does not exist. Using default config.", path.display());
            persistent.default.clone()
        }
        Err(e) => {
            log::warn!("Could not load {}: {}", path.display(), Report::new(e));
            log::warn!("Ignoring config reload");
            return;
        }
    };
    drop(last_config);
    if let Some(auto_reload) = config.auto_reload {
        if auto_reload {
            let handle = &mut *persistent.watcher_handle.borrow_mut();
            if handle.is_none() {
                *handle = Some(tasks::spawn(watch_config(persistent.clone())));
            }
        } else {
            if let Some(handle) = persistent.watcher_handle.take() {
                handle.abort();
            }
        }
    }
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
    state.clear_modes_after_reload();
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
    state.init_modes(&config.shortcuts, &config.input_modes);
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
    if let Some(tearing) = config.tearing
        && let Some(mode) = tearing.mode
    {
        set_tearing_mode(mode);
    }
    set_libei_socket_enabled(config.libei.enable_socket.unwrap_or(false));
    if let Some(enabled) = config.ui_drag.enabled {
        set_ui_drag_enabled(enabled);
    }
    if let Some(threshold) = config.ui_drag.threshold {
        set_ui_drag_threshold(threshold);
    }
    if let Some(xwayland) = config.xwayland
        && let Some(mode) = xwayland.scaling_mode
    {
        set_x_scaling_mode(mode);
    }
    if let Some(cm) = config.color_management
        && let Some(enabled) = cm.enabled
    {
        set_color_management_enabled(enabled);
    }
    if let Some(float) = config.float
        && let Some(show) = float.show_pin_icon
    {
        set_show_float_pin_icon(show);
    }
    if let Some(key) = config.pointer_revert_key {
        persistent.seat.set_pointer_revert_key(key);
    }
    if let Some(v) = config.use_hardware_cursor {
        persistent.seat.use_hardware_cursor(v);
    }
    if let Some(v) = config.show_bar {
        set_show_bar(v);
    }
    if let Some(v) = config.show_titles {
        set_show_titles(v);
    }
    if let Some(v) = config.focus_history {
        if let Some(v) = v.only_visible {
            persistent.seat.focus_history_set_only_visible(v);
        }
        if let Some(v) = v.same_workspace {
            persistent.seat.focus_history_set_same_workspace(v);
        }
    }
    if let Some(v) = config.middle_click_paste {
        set_middle_click_paste_enabled(v);
    }
    if let Some(v) = config.workspace_display_order {
        set_workspace_display_order(v);
    }
    if let Some(simple_im) = config.simple_im {
        if let Some(enabled) = simple_im.enabled {
            persistent.seat.set_simple_im_enabled(enabled);
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
    let mark_names = Default::default();
    let default = parse_config(DEFAULT, &mark_names, |e| {
        panic!("Could not parse the default config: {}", Report::new(e))
    });
    let persistent = Rc::new(PersistentState {
        seen_outputs: Default::default(),
        default: default.unwrap(),
        seat: default_seat(),
        actions: Default::default(),
        client_rules: Default::default(),
        client_rule_mapper: Default::default(),
        window_rules: Default::default(),
        mark_names,
        mode_state: Default::default(),
        watcher_handle: Default::default(),
        last_config: Default::default(),
    });
    {
        let p = persistent.clone();
        on_unload(move || {
            p.actions.borrow_mut().clear();
            p.client_rule_mapper.borrow_mut().take();
            p.mode_state.clear();
        });
    }
    load_config(true, false, &persistent);
}

config!(configure);
