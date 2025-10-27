use {
    crate::{
        backend::Backend,
        client::{Client, ClientCaps},
        ifs::{
            color_management::wp_color_manager_v1::WpColorManagerV1Global,
            ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1Global,
            ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1Global,
            ext_idle_notifier_v1::ExtIdleNotifierV1Global,
            ext_image_copy::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1Global,
            ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1Global,
            ext_session_lock_manager_v1::ExtSessionLockManagerV1Global,
            head_management::jay_head_manager_v1::JayHeadManagerV1Global,
            ipc::{
                data_control::{
                    ext_data_control_manager_v1::ExtDataControlManagerV1Global,
                    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1Global,
                },
                wl_data_device_manager::WlDataDeviceManagerGlobal,
                zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1Global,
            },
            jay_compositor::JayCompositorGlobal,
            jay_damage_tracking::JayDamageTrackingGlobal,
            org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManagerGlobal,
            wl_compositor::WlCompositorGlobal,
            wl_fixes::WlFixesGlobal,
            wl_output::WlOutputGlobal,
            wl_registry::WlRegistry,
            wl_seat::{
                WlSeatGlobal,
                ext_transient_seat_manager_v1::ExtTransientSeatManagerV1Global,
                tablet::zwp_tablet_manager_v2::ZwpTabletManagerV2Global,
                text_input::{
                    zwp_input_method_manager_v2::ZwpInputMethodManagerV2Global,
                    zwp_text_input_manager_v3::ZwpTextInputManagerV3Global,
                },
                wp_pointer_warp_v1::WpPointerWarpV1Global,
                zwp_pointer_constraints_v1::ZwpPointerConstraintsV1Global,
                zwp_pointer_gestures_v1::ZwpPointerGesturesV1Global,
                zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1Global,
                zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1Global,
            },
            wl_shm::WlShmGlobal,
            wl_subcompositor::WlSubcompositorGlobal,
            wl_surface::xwayland_shell_v1::XwaylandShellV1Global,
            wl_upgrade::WlUpgradeGlobal,
            wlr_output_manager::zwlr_output_manager_v1::ZwlrOutputManagerV1Global,
            workspace_manager::ext_workspace_manager_v1::ExtWorkspaceManagerV1Global,
            wp_alpha_modifier_v1::WpAlphaModifierV1Global,
            wp_commit_timing_manager_v1::WpCommitTimingManagerV1Global,
            wp_content_type_manager_v1::WpContentTypeManagerV1Global,
            wp_cursor_shape_manager_v1::WpCursorShapeManagerV1Global,
            wp_fifo_manager_v1::WpFifoManagerV1Global,
            wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1Global,
            wp_presentation::WpPresentationGlobal,
            wp_security_context_manager_v1::WpSecurityContextManagerV1Global,
            wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1Global,
            wp_tearing_control_manager_v1::WpTearingControlManagerV1Global,
            wp_viewporter::WpViewporterGlobal,
            xdg_activation_v1::XdgActivationV1Global,
            xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1Global,
            xdg_toplevel_tag_manager_v1::XdgToplevelTagManagerV1Global,
            xdg_wm_base::XdgWmBaseGlobal,
            xdg_wm_dialog_v1::XdgWmDialogV1Global,
            zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1Global,
            zwlr_layer_shell_v1::ZwlrLayerShellV1Global,
            zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1Global,
            zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1Global,
            zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Global,
            zxdg_output_manager_v1::ZxdgOutputManagerV1Global,
        },
        object::{Interface, ObjectId, Version},
        state::State,
        utils::{
            copyhashmap::{CopyHashMap, Locked},
            numcell::NumCell,
        },
    },
    std::{
        error::Error,
        fmt::{Display, Formatter},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum GlobalsError {
    #[error("The requested global {0} does not exist")]
    GlobalDoesNotExist(GlobalName),
    #[error("The output with id {0} does not exist")]
    OutputDoesNotExist(GlobalName),
    #[error(transparent)]
    GlobalError(GlobalError),
}

#[derive(Debug, Error)]
#[error("An error occurred in a `{}` global", .interface.name())]
pub struct GlobalError {
    pub interface: Interface,
    #[source]
    pub error: Box<dyn Error>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct GlobalName(u32);

impl GlobalName {
    pub fn from_raw(id: u32) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl Display for GlobalName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub trait GlobalBase {
    fn name(&self) -> GlobalName;
    fn bind<'a>(
        self: Rc<Self>,
        client: &'a Rc<Client>,
        id: ObjectId,
        version: Version,
    ) -> Result<(), GlobalsError>;
    fn interface(&self) -> Interface;
}

pub trait Global: GlobalBase {
    fn singleton(&self) -> bool;
    fn version(&self) -> u32;
    fn required_caps(&self) -> ClientCaps {
        ClientCaps::none()
    }
    fn xwayland_only(&self) -> bool {
        false
    }
    fn exposed(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

pub struct Globals {
    next_name: NumCell<u32>,
    registry: CopyHashMap<GlobalName, Rc<dyn Global>>,
    removed: CopyHashMap<GlobalName, Rc<dyn Global>>,
    pub outputs: CopyHashMap<GlobalName, Rc<WlOutputGlobal>>,
    pub seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
}

impl Globals {
    pub fn new() -> Self {
        let slf = Self {
            next_name: NumCell::new(1),
            registry: CopyHashMap::new(),
            removed: CopyHashMap::new(),
            outputs: Default::default(),
            seats: Default::default(),
        };
        slf.add_singletons();
        slf
    }

    pub fn clear(&self) {
        self.registry.clear();
        self.outputs.clear();
        self.seats.clear();
    }

    fn add_singletons(&self) {
        macro_rules! add_singleton {
            ($name:ident) => {
                self.add_global_no_broadcast(&Rc::new($name::new(self.name())));
            };
        }
        add_singleton!(WlCompositorGlobal);
        add_singleton!(WlShmGlobal);
        add_singleton!(WlSubcompositorGlobal);
        add_singleton!(XdgWmBaseGlobal);
        add_singleton!(WlDataDeviceManagerGlobal);
        add_singleton!(ZxdgDecorationManagerV1Global);
        add_singleton!(OrgKdeKwinServerDecorationManagerGlobal);
        add_singleton!(ZwpPrimarySelectionDeviceManagerV1Global);
        add_singleton!(ZwlrLayerShellV1Global);
        add_singleton!(ZwlrOutputManagerV1Global);
        add_singleton!(ZxdgOutputManagerV1Global);
        add_singleton!(JayCompositorGlobal);
        add_singleton!(ZwlrScreencopyManagerV1Global);
        add_singleton!(ZwpRelativePointerManagerV1Global);
        add_singleton!(ExtSessionLockManagerV1Global);
        add_singleton!(WpViewporterGlobal);
        add_singleton!(WpFractionalScaleManagerV1Global);
        add_singleton!(ZwpPointerConstraintsV1Global);
        add_singleton!(XwaylandShellV1Global);
        add_singleton!(WpTearingControlManagerV1Global);
        add_singleton!(WpSinglePixelBufferManagerV1Global);
        add_singleton!(WpCursorShapeManagerV1Global);
        add_singleton!(WpContentTypeManagerV1Global);
        add_singleton!(XdgActivationV1Global);
        add_singleton!(ExtForeignToplevelListV1Global);
        add_singleton!(ZwpIdleInhibitManagerV1Global);
        add_singleton!(ExtIdleNotifierV1Global);
        add_singleton!(XdgToplevelDragManagerV1Global);
        add_singleton!(ZwlrForeignToplevelManagerV1Global);
        add_singleton!(ZwlrDataControlManagerV1Global);
        add_singleton!(WpAlphaModifierV1Global);
        add_singleton!(ZwpVirtualKeyboardManagerV1Global);
        add_singleton!(ZwpInputMethodManagerV2Global);
        add_singleton!(ZwpTextInputManagerV3Global);
        add_singleton!(WpSecurityContextManagerV1Global);
        add_singleton!(XdgWmDialogV1Global);
        add_singleton!(ExtTransientSeatManagerV1Global);
        add_singleton!(ZwpPointerGesturesV1Global);
        add_singleton!(ZwpTabletManagerV2Global);
        add_singleton!(JayDamageTrackingGlobal);
        add_singleton!(ExtOutputImageCaptureSourceManagerV1Global);
        add_singleton!(ExtForeignToplevelImageCaptureSourceManagerV1Global);
        add_singleton!(ExtImageCopyCaptureManagerV1Global);
        add_singleton!(WpFifoManagerV1Global);
        add_singleton!(WpCommitTimingManagerV1Global);
        add_singleton!(ExtDataControlManagerV1Global);
        add_singleton!(WlFixesGlobal);
        add_singleton!(ExtWorkspaceManagerV1Global);
        add_singleton!(WpColorManagerV1Global);
        add_singleton!(XdgToplevelTagManagerV1Global);
        add_singleton!(JayHeadManagerV1Global);
        add_singleton!(WpPointerWarpV1Global);
        add_singleton!(WlUpgradeGlobal);
    }

    pub fn add_backend_singletons(&self, backend: &Rc<dyn Backend>) {
        macro_rules! add_singleton {
            ($name:ident) => {
                self.add_global_no_broadcast(&Rc::new($name::new(self.name())));
            };
        }
        if backend.supports_presentation_feedback() {
            add_singleton!(WpPresentationGlobal);
        }
    }

    pub fn name(&self) -> GlobalName {
        let id = self.next_name.fetch_add(1);
        if id == 0 {
            panic!("Global names overflowed");
        }
        GlobalName(id)
    }

    fn insert_no_broadcast<'a>(&'a self, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
    }

    fn insert_no_broadcast_<'a>(&'a self, global: &Rc<dyn Global>) {
        self.registry.set(global.name(), global.clone());
    }

    fn insert(&self, state: &State, global: Rc<dyn Global>) {
        self.insert_no_broadcast_(&global);
        self.broadcast(state, global.required_caps(), global.xwayland_only(), |r| {
            r.send_global(&global)
        });
    }

    pub fn get(
        &self,
        name: GlobalName,
        client_caps: ClientCaps,
        allow_xwayland_only: bool,
    ) -> Result<Rc<dyn Global>, GlobalsError> {
        let global = self.take(name, false)?;
        if client_caps.not_contains(global.required_caps())
            || (global.xwayland_only() && !allow_xwayland_only)
        {
            return Err(GlobalsError::GlobalDoesNotExist(name));
        }
        Ok(global)
    }

    pub fn remove<T: RemovableWaylandGlobal>(
        &self,
        state: &State,
        global: &Rc<T>,
    ) -> Result<(), GlobalsError> {
        let _global = self.take(global.name(), true)?;
        global.remove(self);
        let replacement = global.clone().create_replacement();
        assert_eq!(global.name(), replacement.name());
        assert_eq!(global.interface().0, replacement.interface().0);
        self.removed.set(global.name(), replacement);
        self.broadcast(state, global.required_caps(), global.xwayland_only(), |r| {
            r.send_global_remove(global.name())
        });
        Ok(())
    }

    pub fn lock_seats(&self) -> Locked<'_, GlobalName, Rc<WlSeatGlobal>> {
        self.seats.lock()
    }

    pub fn notify_all(&self, registry: &Rc<WlRegistry>) {
        let caps = registry.client.effective_caps.get();
        let xwayland = registry.client.is_xwayland;
        let globals = self.registry.lock();
        macro_rules! emit {
            ($singleton:expr) => {
                for global in globals.values() {
                    if global.singleton() == $singleton {
                        if global.exposed(&registry.client.state)
                            && caps.contains(global.required_caps())
                            && (xwayland || !global.xwayland_only())
                        {
                            registry.send_global(global);
                        }
                    }
                }
            };
        }
        emit!(true);
        emit!(false);
    }

    fn broadcast<F: Fn(&Rc<WlRegistry>)>(
        &self,
        state: &State,
        required_caps: ClientCaps,
        xwayland_only: bool,
        f: F,
    ) {
        state.clients.broadcast(required_caps, xwayland_only, |c| {
            let registries = c.lock_registries();
            for registry in registries.values() {
                f(registry);
            }
            // c.flush();
        });
    }

    fn take(&self, name: GlobalName, remove: bool) -> Result<Rc<dyn Global>, GlobalsError> {
        let res = if remove {
            self.registry.remove(&name)
        } else {
            match self.registry.get(&name) {
                Some(res) => Some(res),
                _ => self.removed.get(&name),
            }
        };
        match res {
            Some(g) => Ok(g),
            None => Err(GlobalsError::GlobalDoesNotExist(name)),
        }
    }

    #[expect(dead_code)]
    pub fn get_output(&self, output: GlobalName) -> Result<Rc<WlOutputGlobal>, GlobalsError> {
        match self.outputs.get(&output) {
            Some(o) => Ok(o),
            _ => Err(GlobalsError::OutputDoesNotExist(output)),
        }
    }

    pub fn add_global<T: WaylandGlobal>(&self, state: &State, global: &Rc<T>) {
        global.clone().add(self);
        self.insert(state, global.clone())
    }

    pub fn add_global_no_broadcast<T: WaylandGlobal>(&self, global: &Rc<T>) {
        global.clone().add(self);
        self.insert_no_broadcast(global.clone());
    }
}

pub trait WaylandGlobal: Global + 'static {
    fn add(self: Rc<Self>, globals: &Globals) {
        let _ = globals;
    }
    fn remove(&self, globals: &Globals) {
        let _ = globals;
    }
}

pub trait RemovableWaylandGlobal: WaylandGlobal {
    fn create_replacement(self: Rc<Self>) -> Rc<dyn Global>;
}
