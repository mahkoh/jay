use {
    crate::{
        backend::{BackendColorSpace, BackendEotfs, ConnectorId, Mode, MonitorInfo},
        format::Format,
        globals::GlobalName,
        ifs::wl_output::{BlendSpace, PersistentOutputState},
        scale::Scale,
        state::{OutputData, State},
        tree::{OutputNode, TearingMode, Transform, VrrMode},
        utils::rc_eq::RcEq,
    },
    std::{
        cell::{Ref, RefCell},
        collections::hash_map::Entry,
        rc::Rc,
    },
};

linear_ids!(HeadNames, HeadName, u64);

#[derive(Clone, PartialEq)]
pub struct HeadState {
    pub connector_id: ConnectorId,
    pub name: RcEq<String>,
    pub wl_output: Option<GlobalName>,
    pub connector_enabled: bool,
    pub active: bool,
    pub connected: bool,
    pub in_compositor_space: bool,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub mode: Mode,
    pub transform: Transform,
    pub scale: Scale,
    pub monitor_info: Option<RcEq<MonitorInfo>>,
    pub inherent_non_desktop: bool,
    pub override_non_desktop: Option<bool>,
    pub vrr: bool,
    pub vrr_mode: VrrMode,
    pub tearing_enabled: bool,
    pub tearing_active: bool,
    pub tearing_mode: TearingMode,
    pub format: &'static Format,
    pub color_space: BackendColorSpace,
    pub eotf: BackendEotfs,
    pub supported_formats: RcEq<Vec<&'static Format>>,
    pub brightness: Option<f64>,
    pub blend_space: BlendSpace,
    pub use_native_gamut: bool,
    pub vrr_cursor_hz: Option<f64>,
    pub persistent_state: Option<RcEq<PersistentOutputState>>,
}

pub struct ReadOnlyHeadState {
    state: Rc<RefCell<HeadState>>,
}

impl ReadOnlyHeadState {
    pub fn borrow(&self) -> Ref<'_, HeadState> {
        self.state.borrow()
    }
}

impl HeadState {
    pub fn update_in_compositor_space(&mut self, state: &State, wl_output: Option<GlobalName>) {
        self.in_compositor_space = false;
        self.wl_output = None;
        if !self.connector_enabled {
            return;
        }
        let Some(mi) = &self.monitor_info else {
            return;
        };
        if mi.non_desktop {
            return;
        }
        if self.override_non_desktop == Some(true) {
            return;
        }
        self.in_compositor_space = true;
        self.wl_output = wl_output;
        if self.persistent_state.is_none() {
            let ds = state
                .persistent_output_states
                .get(&mi.output_id)
                .unwrap_or_else(|| state.new_persistent_output_state());
            self.position = ds.pos.get();
            self.transform = ds.transform.get();
            self.vrr_mode = ds.vrr_mode.get();
            self.tearing_mode = ds.tearing_mode.get();
            self.brightness = ds.brightness.get();
            self.blend_space = ds.blend_space.get();
            self.use_native_gamut = ds.use_native_gamut.get();
            self.vrr_cursor_hz = ds.vrr_cursor_hz.get();
            self.scale = ds.scale.get();
            self.persistent_state = Some(RcEq(ds));
            if let Some(c) = state.connectors.get(&self.connector_id) {
                self.mode = c.state.borrow().mode;
            }
            self.update_size();
        }
    }

    pub fn update_size(&mut self) {
        self.size =
            OutputNode::calculate_extents_(self.mode, self.transform, self.scale, self.position)
                .size();
    }

    pub fn flush_persistent_state(&self, state: &State) {
        if let Some(mi) = &self.monitor_info
            && let Some(ds) = &self.persistent_state
            && let Entry::Vacant(v) = state
                .persistent_output_states
                .lock()
                .entry(mi.output_id.clone())
        {
            v.insert(ds.0.clone());
        }
    }
}

pub struct HeadManagers {
    pub name: HeadName,
    state: Rc<RefCell<HeadState>>,
}

impl HeadManagers {
    pub fn new(name: HeadName, state: HeadState) -> Self {
        Self {
            name,
            state: Rc::new(RefCell::new(state)),
        }
    }

    pub fn state(&self) -> ReadOnlyHeadState {
        ReadOnlyHeadState {
            state: self.state.clone(),
        }
    }

    pub fn handle_output_connected(&self, s: &State, output: &OutputData) {
        let state = &mut *self.state.borrow_mut();
        state.connected = true;
        state.monitor_info = Some(RcEq(output.monitor_info.clone()));
        state.persistent_state = None;
        state.inherent_non_desktop = output.monitor_info.non_desktop;
        state.update_in_compositor_space(s, output.node.as_ref().map(|n| n.global.name));
    }

    pub fn handle_output_disconnected(&self, s: &State) {
        let state = &mut *self.state.borrow_mut();
        state.connected = false;
        state.monitor_info = None;
        state.persistent_state = None;
        state.update_in_compositor_space(s, None);
    }

    pub fn handle_position_size_change(&self, node: &OutputNode) {
        let state = &mut *self.state.borrow_mut();
        let pos = node.global.pos.get();
        state.position = pos.position();
        state.size = pos.size();
    }

    pub fn handle_mode_change(&self, mode: Mode) {
        let state = &mut *self.state.borrow_mut();
        state.mode = mode;
    }

    pub fn handle_transform_change(&self, transform: Transform) {
        let state = &mut *self.state.borrow_mut();
        state.transform = transform;
    }

    pub fn handle_scale_change(&self, scale: Scale) {
        let state = &mut *self.state.borrow_mut();
        state.scale = scale;
    }

    pub fn handle_enabled_change(&self, s: &State, enabled: bool) {
        let state = &mut *self.state.borrow_mut();
        state.connector_enabled = enabled;
        state.update_in_compositor_space(s, state.wl_output);
    }

    pub fn handle_active_change(&self, active: bool) {
        let state = &mut *self.state.borrow_mut();
        state.active = active;
    }

    pub fn handle_non_desktop_override_changed(&self, overrd: Option<bool>) {
        let state = &mut *self.state.borrow_mut();
        state.override_non_desktop = overrd;
    }

    pub fn handle_vrr_change(&self, vrr: bool) {
        let state = &mut *self.state.borrow_mut();
        state.vrr = vrr;
    }

    pub fn handle_vrr_mode_change(&self, vrr_mode: &VrrMode) {
        let state = &mut *self.state.borrow_mut();
        state.vrr_mode = *vrr_mode;
    }

    pub fn handle_tearing_enabled_change(&self, enabled: bool) {
        let state = &mut *self.state.borrow_mut();
        state.tearing_enabled = enabled;
    }

    pub fn handle_tearing_active_change(&self, active: bool) {
        let state = &mut *self.state.borrow_mut();
        state.tearing_active = active;
    }

    pub fn handle_tearing_mode_change(&self, tearing_mode: &TearingMode) {
        let state = &mut *self.state.borrow_mut();
        state.tearing_mode = *tearing_mode;
    }

    pub fn handle_format_change(&self, format: &'static Format) {
        let state = &mut *self.state.borrow_mut();
        state.format = format;
    }

    pub fn handle_colors_change(&self, color_space: BackendColorSpace, eotf: BackendEotfs) {
        let state = &mut *self.state.borrow_mut();
        state.color_space = color_space;
        state.eotf = eotf;
    }

    pub fn handle_formats_change(&self, formats: &Rc<Vec<&'static Format>>) {
        let state = &mut *self.state.borrow_mut();
        state.supported_formats.0 = formats.clone();
    }

    pub fn handle_brightness_change(&self, brightness: Option<f64>) {
        let state = &mut *self.state.borrow_mut();
        state.brightness = brightness;
    }

    pub fn handle_blend_space_change(&self, blend_space: BlendSpace) {
        let state = &mut *self.state.borrow_mut();
        state.blend_space = blend_space;
    }

    pub fn handle_use_native_gamut_change(&self, use_native_gamut: bool) {
        let state = &mut *self.state.borrow_mut();
        state.use_native_gamut = use_native_gamut;
    }

    pub fn handle_cursor_hz_change(&self, cursor_hz: Option<f64>) {
        let state = &mut *self.state.borrow_mut();
        state.vrr_cursor_hz = cursor_hz;
    }
}
