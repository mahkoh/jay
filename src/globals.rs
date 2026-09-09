use crate::client::Client;
use crate::client::ClientCaps;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_registry::WlRegistry;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::Interface;
use crate::object::Version;
use crate::state::State;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::copyhashmap::Locked;
use crate::utils::numcell::NumCell;
use crate::wire::ObjectId;
use arrayvec::ArrayVec;
use jay_proc::jay_hash;
use linearize::Linearize;
use linearize::StaticMap;
use std::cell::Cell;
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::rc::Rc;
use thiserror::Error;

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

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq)]
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
    fn singleton(&self) -> Option<Singleton>;
}

pub trait Global: GlobalBase {
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
    fn permitted(&self, caps: ClientCaps, xwayland: bool) -> bool {
        caps.contains(self.required_caps()) && (xwayland || !self.xwayland_only())
    }
    fn not_permitted(&self, caps: ClientCaps, xwayland: bool) -> bool {
        !self.permitted(caps, xwayland)
    }
}

pub struct Globals {
    next_name: NumCell<u32>,
    registry: CopyHashMap<GlobalName, Rc<dyn Global>>,
    removed: CopyHashMap<GlobalName, Rc<dyn Global>>,
    pub outputs: CopyHashMap<GlobalName, Rc<WlOutputGlobal>>,
    pub seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
    pub singletons: StaticMap<Singleton, GlobalName>,
    exposed: StaticMap<Singleton, Cell<bool>>,
}

impl Globals {
    pub fn new() -> Self {
        let mut slf = Self {
            next_name: NumCell::new(1),
            registry: CopyHashMap::new(),
            removed: CopyHashMap::new(),
            outputs: Default::default(),
            seats: Default::default(),
            singletons: StaticMap::from_fn(|_| GlobalName(0)),
            exposed: Default::default(),
        };
        add_singletons(&mut slf);
        slf
    }

    pub fn clear(&self) {
        self.registry.clear();
        self.outputs.clear();
        self.seats.clear();
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
            r.handle_global(&global)
        });
    }

    pub fn get(
        &self,
        name: GlobalName,
        client_caps: ClientCaps,
        allow_xwayland_only: bool,
    ) -> Result<Rc<dyn Global>, GlobalsError> {
        let global = self.take(name, false)?;
        if global.not_permitted(client_caps, allow_xwayland_only) {
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
            r.handle_global_removed(&**global)
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
                    if global.singleton().is_some() == $singleton {
                        if global.exposed(&registry.client.state)
                            && global.permitted(caps, xwayland)
                        {
                            registry.handle_global(global);
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

    #[expect(unused)]
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

    fn add_global_no_broadcast<T: WaylandGlobal>(&self, global: &Rc<T>) {
        global.clone().add(self);
        self.insert_no_broadcast(global.clone());
    }

    pub fn expose_new_singletons(&self, state: &State) {
        let mut singletons = ArrayVec::<_, { Singleton::LENGTH }>::new();
        for (singleton, name) in self.singletons.iter() {
            if let Some(global) = self.registry.get(name) {
                let exposed = global.exposed(state);
                if self.exposed[singleton].replace(exposed) != exposed && exposed {
                    singletons.push(global);
                }
            }
        }
        if singletons.is_empty() {
            return;
        }
        for client in state.clients.clients.borrow().values() {
            let client = &client.data;
            let caps = client.effective_caps.get();
            let xwayland = client.is_xwayland;
            for global in &singletons {
                if global.permitted(caps, xwayland) {
                    for registry in client.objects.registries.lock().values() {
                        registry.handle_global(global);
                    }
                }
            }
        }
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

include!(concat!(env!("OUT_DIR"), "/singletons.rs"));

mod singletons {
    pub(super) use crate::ifs::color_management::wp_color_manager_v1::WpColorManagerV1Global;
    pub(super) use crate::ifs::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1Global;
    pub(super) use crate::ifs::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1Global;
    pub(super) use crate::ifs::ext_idle_notifier_v1::ExtIdleNotifierV1Global;
    pub(super) use crate::ifs::ext_image_copy::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1Global;
    pub(super) use crate::ifs::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1Global;
    pub(super) use crate::ifs::ext_session_lock_manager_v1::ExtSessionLockManagerV1Global;
    pub(super) use crate::ifs::ipc::data_control::ext_data_control_manager_v1::ExtDataControlManagerV1Global;
    pub(super) use crate::ifs::ipc::data_control::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1Global;
    pub(super) use crate::ifs::ipc::wl_data_device_manager::WlDataDeviceManagerGlobal;
    pub(super) use crate::ifs::ipc::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1Global;
    pub(super) use crate::ifs::jay_compositor::JayCompositorGlobal;
    pub(super) use crate::ifs::jay_damage_tracking::JayDamageTrackingGlobal;
    pub(super) use crate::ifs::jay_popup_ext_manager_v1::JayPopupExtManagerV1Global;
    pub(super) use crate::ifs::org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManagerGlobal;
    pub(super) use crate::ifs::wl_compositor::WlCompositorGlobal;
    pub(super) use crate::ifs::wl_drm::WlDrmGlobal;
    pub(super) use crate::ifs::wl_fixes::WlFixesGlobal;
    pub(super) use crate::ifs::wl_seat::ext_transient_seat_manager_v1::ExtTransientSeatManagerV1Global;
    pub(super) use crate::ifs::wl_seat::tablet::zwp_tablet_manager_v2::ZwpTabletManagerV2Global;
    pub(super) use crate::ifs::wl_seat::text_input::zwp_input_method_manager_v2::ZwpInputMethodManagerV2Global;
    pub(super) use crate::ifs::wl_seat::text_input::zwp_text_input_manager_v3::ZwpTextInputManagerV3Global;
    pub(super) use crate::ifs::wl_seat::wp_pointer_warp_v1::WpPointerWarpV1Global;
    pub(super) use crate::ifs::wl_seat::zwp_pointer_constraints_v1::ZwpPointerConstraintsV1Global;
    pub(super) use crate::ifs::wl_seat::zwp_pointer_gestures_v1::ZwpPointerGesturesV1Global;
    pub(super) use crate::ifs::wl_seat::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1Global;
    pub(super) use crate::ifs::wl_seat::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1Global;
    pub(super) use crate::ifs::wl_shm::WlShmGlobal;
    pub(super) use crate::ifs::wl_subcompositor::WlSubcompositorGlobal;
    pub(super) use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1Global;
    pub(super) use crate::ifs::wl_surface::xwayland_shell_v1::XwaylandShellV1Global;
    pub(super) use crate::ifs::wlr_output_manager::zwlr_output_manager_v1::ZwlrOutputManagerV1Global;
    pub(super) use crate::ifs::workspace_manager::ext_workspace_manager_v1::ExtWorkspaceManagerV1Global;
    pub(super) use crate::ifs::wp_alpha_modifier_v1::WpAlphaModifierV1Global;
    pub(super) use crate::ifs::wp_color_representation_manager_v1::WpColorRepresentationManagerV1Global;
    pub(super) use crate::ifs::wp_commit_timing_manager_v1::WpCommitTimingManagerV1Global;
    pub(super) use crate::ifs::wp_content_type_manager_v1::WpContentTypeManagerV1Global;
    pub(super) use crate::ifs::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1Global;
    pub(super) use crate::ifs::wp_fifo_manager_v1::WpFifoManagerV1Global;
    pub(super) use crate::ifs::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1Global;
    pub(super) use crate::ifs::wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1Global;
    pub(super) use crate::ifs::wp_presentation::WpPresentationGlobal;
    pub(super) use crate::ifs::wp_security_context_manager_v1::WpSecurityContextManagerV1Global;
    pub(super) use crate::ifs::wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1Global;
    pub(super) use crate::ifs::wp_tearing_control_manager_v1::WpTearingControlManagerV1Global;
    pub(super) use crate::ifs::wp_viewporter::WpViewporterGlobal;
    pub(super) use crate::ifs::xdg_activation_v1::XdgActivationV1Global;
    pub(super) use crate::ifs::xdg_session_manager_v1::XdgSessionManagerV1Global;
    pub(super) use crate::ifs::xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1Global;
    pub(super) use crate::ifs::xdg_toplevel_tag_manager_v1::XdgToplevelTagManagerV1Global;
    pub(super) use crate::ifs::xdg_wm_base::XdgWmBaseGlobal;
    pub(super) use crate::ifs::xdg_wm_dialog_v1::XdgWmDialogV1Global;
    pub(super) use crate::ifs::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1Global;
    pub(super) use crate::ifs::zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1Global;
    pub(super) use crate::ifs::zwlr_layer_shell_v1::ZwlrLayerShellV1Global;
    pub(super) use crate::ifs::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1Global;
    pub(super) use crate::ifs::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1Global;
    pub(super) use crate::ifs::zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1Global;
    pub(super) use crate::ifs::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Global;
    pub(super) use crate::ifs::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Global;
    pub(super) use crate::ifs::zxdg_output_manager_v1::ZxdgOutputManagerV1Global;
}
