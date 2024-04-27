#![allow(clippy::len_zero, clippy::single_char_pattern, clippy::collapsible_if)]

mod config;
mod toml;

use {
    crate::config::{
        parse_config, Action, Config, ConfigConnector, ConfigDrmDevice, ConfigKeymap,
        ConnectorMatch, DrmDeviceMatch, Exec, Input, InputMatch, Output, OutputMatch, Shortcut,
        SimpleCommand, Status, Theme,
    },
    ahash::{AHashMap, AHashSet},
    error_reporter::Report,
    jay_config::{
        config, config_dir,
        exec::{set_env, unset_env, Command},
        get_workspace,
        input::{
            get_seat, input_devices, on_new_input_device, FocusFollowsMouseMode, InputDevice, Seat,
        },
        is_reload,
        keyboard::{Keymap, ModifiedKeySym},
        logging::set_log_level,
        on_devices_enumerated, on_idle, quit, reload, set_default_workspace_capture,
        set_explicit_sync_enabled, set_idle,
        status::{set_i3bar_separator, set_status, set_status_command, unset_status_command},
        switch_to_vt,
        theme::{reset_colors, reset_font, reset_sizes, set_font},
        video::{
            connectors, drm_devices, on_connector_connected, on_graphics_initialized,
            on_new_connector, on_new_drm_device, set_direct_scanout_enabled, set_gfx_api,
            Connector, DrmDevice,
        },
    },
    std::{cell::RefCell, io::ErrorKind, path::PathBuf, rc::Rc},
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
        let s = state.persistent.seat;
        match self {
            Action::SimpleCommand { cmd } => match cmd {
                SimpleCommand::Focus(dir) => B::new(move || s.focus(dir)),
                SimpleCommand::Move(dir) => B::new(move || s.move_(dir)),
                SimpleCommand::Split(axis) => B::new(move || s.create_split(axis)),
                SimpleCommand::ToggleSplit => B::new(move || s.toggle_split()),
                SimpleCommand::ToggleMono => B::new(move || s.toggle_mono()),
                SimpleCommand::ToggleFullscreen => B::new(move || s.toggle_fullscreen()),
                SimpleCommand::FocusParent => B::new(move || s.focus_parent()),
                SimpleCommand::Close => B::new(move || s.close()),
                SimpleCommand::DisablePointerConstraint => {
                    B::new(move || s.disable_pointer_constraint())
                }
                SimpleCommand::ToggleFloating => B::new(move || s.toggle_floating()),
                SimpleCommand::Quit => B::new(quit),
                SimpleCommand::ReloadConfigToml => {
                    let persistent = state.persistent.clone();
                    B::new(move || load_config(false, &persistent))
                }
                SimpleCommand::ReloadConfigSo => B::new(reload),
                SimpleCommand::None => B::new(|| ()),
                SimpleCommand::Forward(bool) => B::new(move || s.set_forward(bool)),
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
                B::new(move || s.set_workspace(workspace))
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
            Action::ConfigureIdle { idle } => B::new(move || set_idle(Some(idle))),
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
    }
}

struct State {
    outputs: AHashMap<String, OutputMatch>,
    drm_devices: AHashMap<String, DrmDeviceMatch>,
    input_devices: AHashMap<String, InputMatch>,
    persistent: Rc<PersistentState>,
    keymaps: AHashMap<String, Keymap>,
}

impl Drop for State {
    fn drop(&mut self) {
        for keymap in self.keymaps.values() {
            keymap.destroy();
        }
    }
}

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
}

fn load_config(initial_load: bool, persistent: &Rc<PersistentState>) {
    let mut path = PathBuf::from(config_dir());
    path.push("config.toml");
    let config = match std::fs::read(&path) {
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
    for input in &config.inputs {
        if let Some(tag) = &input.tag {
            let prev = input_devices.insert(tag.clone(), input.match_.clone());
            if prev.is_some() {
                log::warn!("Duplicate input tag {tag}");
            }
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
    });
    state.set_status(&config.status);
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
        move |c| {
            for input in &config.inputs {
                if input.match_.matches(c, &state) {
                    input.apply(c, &state);
                }
            }
        }
    });
    persistent
        .seat
        .set_focus_follows_mouse_mode(match config.focus_follows_mouse {
            true => FocusFollowsMouseMode::True,
            false => FocusFollowsMouseMode::False,
        });
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
    });
    load_config(true, &persistent);
}

config!(configure);
