/*
dir root {
    version: reg,
    num_clients: reg,
    color_management_enabled: reg,
    config_dir: reg,
    config_locked_shortcuts: reg,
    connector_ids: reg,
    create_default_seat: reg,
    cursor_user_group_ids: reg,
    cursor_user_ids: reg,
    data_control_device_ids: reg,
    data_offer_ids: reg,
    data_source_ids: reg,
    default_gfx_api: reg,
    default_vrr_cursor_hz: reg,
    default_workspace_capture: reg,
    direct_scanout_enabled: reg,
    dma_buf_ids: reg,
    double_click_distance: reg,
    double_click_interval_usec: reg,
    drm_dev_ids: reg,
    dummy_output_id: reg,
    enable_ei_acceptor: reg,
    enable_primary_selection: reg,
    explicit_sync_enabled: reg,
    explicit_sync_supported: reg,
    fallback_output: reg,
    float_above_fullscreen: reg,
    idle_inhibitor_ids: reg,
    input_device_group_ids: reg,
    input_device_ids: reg,
    keyboard_state_ids: reg,
    no_client_prime: reg,
    node_ids: reg,
    outputs_without_hc: reg,
    physical_keyboard_ids: reg,
    render_ctx_ever_initialized: reg,
    render_ctx_version: reg,
    seat_ids: reg,
    serial: reg,
    session_management_enabled: reg,
    show_bar: reg,
    show_pin_icon: reg,
    split_reuses_container: reg,
    subsurface_ids: reg,
    supports_presentation_feedback: reg,
    tablet_ids: reg,
    tablet_pad_ids: reg,
    tablet_tool_ids: reg,
    tray_item_ids: reg,
    tree_changed_sent: reg,
    ui_drag_enabled: reg,
    ui_drag_threshold_squared: reg,
    visualize_compositing: reg,
    workspace_display_order: reg,
    clients: view (key = 0),
    outputs: view (key = 0),
    workspaces: view (key = 0),
    seats: view (key = 0),
    root: custom,
    backend: custom (opt),
    globals: view (key = 0),
}

dir seats {
    by_id: link,
    by_name: view (key = 0),
}
*/
use crate::client::Client;
use crate::client::ClientId;
use crate::client::ClientView;
use crate::dfs::dfs_helpers::DfsGlobalLink;
use crate::dfs::dfs_helpers::write_root_link;
use crate::globals::DfsGlobalsView;
use crate::globals::GlobalBase;
use crate::state::State;
use crate::state::state_dfs_g_fuse::generated::root;
use crate::state::state_dfs_g_fuse::generated::seats;
use crate::tree::NodeBase;
use crate::tree::OutputNode;
use crate::tree::OutputNodeView;
use crate::tree::WorkspaceNode;
use crate::tree::WorkspaceNodeView;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::CopyHashMapDir2;
use crate::utils::fuse::fuse_views::CopyHashMapDir2View;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::IterDir;
use crate::utils::fuse::fuse_views::IterDirKeyed;
use crate::utils::fuse::fuse_views::IterDirKeyedView;
use crate::utils::fuse::fuse_views::IterDirView;
use crate::utils::static_rc::static_rc;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::version::VERSION;
use std::rc::Rc;
use std::str::FromStr;
use uapi::OwnedFd;

impl State {
    pub fn debugfs(self: &Rc<Self>) -> Rc<dyn FuseInode> {
        self.tv_wrap_rc_ref_clone::<root::View>()
    }

    pub fn snapshot_debugfs(
        self: &Rc<Self>,
        root: &str,
        json: bool,
    ) -> Result<Rc<OwnedFd>, FuseError> {
        let inode: Rc<dyn FuseInode> = self.tv_wrap_rc_ref_clone::<root::View>();
        inode.snapshot(0, root, json)
    }
}

impl root::Dir for State {
    type ViewClients = IterDir<Clients>;
    type ViewOutputs = IterDir<Outputs>;
    type ViewWorkspaces = CopyHashMapDir2<Workspaces>;
    type ViewSeats = seats::View;
    type ViewGlobals = DfsGlobalsView;

    fn read_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        VERSION.str_fmt(buf, ctx);
    }

    fn read_num_clients(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.clients.clients.borrow().len().str_fmt(buf, ctx);
    }

    fn read_color_management_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.color_management_enabled.get().str_fmt(buf, ctx);
    }

    fn read_config_dir(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.config_dir.str_fmt(buf, ctx);
    }

    fn read_config_locked_shortcuts(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.config_locked_shortcuts.get().str_fmt(buf, ctx);
    }

    fn read_connector_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.connector_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_create_default_seat(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.create_default_seat.get().str_fmt(buf, ctx);
    }

    fn read_cursor_user_group_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_user_group_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_cursor_user_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_user_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_data_control_device_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data_control_device_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_data_offer_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data_offer_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_data_source_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data_source_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_default_gfx_api(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.default_gfx_api.get().to_str().str_fmt(buf, ctx);
    }

    fn read_default_vrr_cursor_hz(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.default_vrr_cursor_hz.get().str_fmt(buf, ctx);
    }

    fn read_default_workspace_capture(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.default_workspace_capture.get().str_fmt(buf, ctx);
    }

    fn read_direct_scanout_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.direct_scanout_enabled.get().str_fmt(buf, ctx);
    }

    fn read_dma_buf_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dma_buf_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_double_click_distance(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.double_click_distance.get().str_fmt(buf, ctx);
    }

    fn read_double_click_interval_usec(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.double_click_interval_usec.get().str_fmt(buf, ctx);
    }

    fn read_drm_dev_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drm_dev_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_dummy_output_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dummy_output_id.raw().str_fmt(buf, ctx);
    }

    fn read_enable_ei_acceptor(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.enable_ei_acceptor.get().str_fmt(buf, ctx);
    }

    fn read_enable_primary_selection(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.enable_primary_selection.get().str_fmt(buf, ctx);
    }

    fn read_explicit_sync_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.explicit_sync_enabled.get().str_fmt(buf, ctx);
    }

    fn read_explicit_sync_supported(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.explicit_sync_supported.get().str_fmt(buf, ctx);
    }

    fn read_fallback_output(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.fallback_output
            .get()
            .map(|v| v.raw())
            .str_fmt(buf, ctx);
    }

    fn read_float_above_fullscreen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.float_above_fullscreen.get().str_fmt(buf, ctx);
    }

    fn read_idle_inhibitor_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.idle_inhibitor_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_input_device_group_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.input_device_group_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_input_device_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.input_device_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_keyboard_state_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.keyboard_state_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_no_client_prime(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.no_client_prime.str_fmt(buf, ctx);
    }

    fn read_node_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_ids.last().str_fmt(buf, ctx);
    }

    fn read_outputs_without_hc(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.outputs_without_hc.get().str_fmt(buf, ctx);
    }

    fn read_physical_keyboard_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.physical_keyboard_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_render_ctx_ever_initialized(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_ctx_ever_initialized.get().str_fmt(buf, ctx);
    }

    fn read_render_ctx_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_ctx_version.get().str_fmt(buf, ctx);
    }

    fn read_seat_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.seat_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.serial.get().str_fmt(buf, ctx);
    }

    fn read_session_management_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.session_management_enabled.get().str_fmt(buf, ctx);
    }

    fn read_show_bar(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.show_bar.get().str_fmt(buf, ctx);
    }

    fn read_show_pin_icon(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.show_pin_icon.get().str_fmt(buf, ctx);
    }

    fn read_split_reuses_container(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.split_reuses_container.get().str_fmt(buf, ctx);
    }

    fn read_subsurface_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.subsurface_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_supports_presentation_feedback(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.supports_presentation_feedback.get().str_fmt(buf, ctx);
    }

    fn read_tablet_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tablet_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_tablet_pad_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tablet_pad_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_tablet_tool_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tablet_tool_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_tray_item_ids(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tray_item_ids.last().raw().str_fmt(buf, ctx);
    }

    fn read_tree_changed_sent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tree_changed_sent.get().str_fmt(buf, ctx);
    }

    fn read_ui_drag_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ui_drag_enabled.get().str_fmt(buf, ctx);
    }

    fn read_ui_drag_threshold_squared(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ui_drag_threshold_squared.get().str_fmt(buf, ctx);
    }

    fn read_visualize_compositing(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.visualize_compositing.get().str_fmt(buf, ctx);
    }

    fn read_workspace_display_order(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.workspace_display_order.get().text().str_fmt(buf, ctx);
    }

    fn get_root(self: &Rc<Self>, _key: u64) -> FuseInodeWithKey {
        self.root.clone().node_debugfs()
    }

    fn get_backend(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        self.backend.get().debugfs()
    }
}

struct Clients;

impl IterDirView<State> for Clients {
    type Value = Client;
    type View = ClientView;

    fn iter(t: &State, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
        let mut buf = itoa::Buffer::new();
        for (id, holder) in t.clients.clients.borrow().iter() {
            f(buf.format(id.raw()), &holder.data);
        }
    }

    fn get(t: &State, _key: u64, name: &str) -> Option<Rc<Self::Value>> {
        let id = u64::from_str(name).ok()?;
        t.clients.get(ClientId::from_raw(id)).ok()
    }
}

struct Outputs;

impl IterDirView<State> for Outputs {
    type Value = OutputNode;
    type View = OutputNodeView;

    fn iter(t: &State, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
        for v in t.outputs.lock().values() {
            if let Some(node) = &v.node {
                f(&v.connector.name, node);
            }
        }
    }

    fn get(t: &State, _key: u64, name: &str) -> Option<Rc<Self::Value>> {
        for v in t.outputs.lock().values() {
            if v.connector.name.as_str() == name {
                return v.node.clone();
            }
        }
        None
    }
}

struct Workspaces;

impl CopyHashMapDir2View<State> for Workspaces {
    type Key = String;
    type KeyRef = str;
    type Value = WorkspaceNode;
    type View = WorkspaceNodeView;
    type StringBuf = ();

    fn get(t: &State, _key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>> {
        &t.workspaces
    }

    fn format_key(_buf: &mut Self::StringBuf, key: &Self::Key, f: impl FnOnce(&str)) {
        f(key);
    }

    fn parse_name(
        key: &str,
        f: impl FnOnce(&Self::KeyRef) -> Option<Rc<Self::Value>>,
    ) -> Option<Rc<Self::Value>> {
        f(key)
    }
}

impl seats::Dir for State {
    type ViewByName = IterDirKeyed<SeatsByName>;

    fn readlink_by_id(&self, depth: u64, buf: &mut String) {
        write_root_link(buf, depth);
        buf.push_str("globals/seats");
    }
}

struct SeatsByName;

impl IterDirKeyedView<State> for SeatsByName {
    type Value = ();
    type View = FuseLink<DfsGlobalLink>;

    fn iter(t: Rc<State>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        for s in t.globals.seats.lock().values() {
            f(&s.seat_name(), static_rc(), s.name().raw() as _);
        }
    }

    fn get(t: Rc<State>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        for s in t.globals.seats.lock().values() {
            if s.seat_name() == name {
                return Some((static_rc().clone(), s.name().raw() as _));
            }
        }
        None
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_8563e4e637c55ed8bf14a6455be558a3271e5c626f37595a04b66b07ff35056e.rs",
));
// FUSE GENERATED STOP
