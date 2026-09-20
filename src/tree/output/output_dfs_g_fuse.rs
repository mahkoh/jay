/*
dir output_node {
    @inherit dfs_node,
    id: reg,
    name: reg,
    status: reg,
    is_dummy: reg,
    title_visible: reg,
    tearing: reg,
    damage_scheduled: reg,
    hardware_cursor_needs_render: reg,
    update_render_data_scheduled: reg,
    has_hardware_cursor: reg,
    render_margin_ns: reg,
    flip_margin_ns: reg,
    has_lock_surface: reg,
    num_workspaces: reg,
    num_tray_items: reg,
    num_screencasts: reg,
    num_screencopies: reg,
    num_jay_outputs: reg,
    num_cursor_users: reg,
    pos_live: reg,
    pos_render: reg,
    scale: reg,
    legacy_scale: reg,
    non_exclusive: reg,
    workspace_rect: reg,
    bar: reg,
    exclusive_top: reg,
    exclusive_right: reg,
    exclusive_bottom: reg,
    exclusive_left: reg,
    tray_start_rel: reg,
    workspace: link (opt),
    overlay: link (opt),
    lock_surface: link (opt),
    workspaces: view (key = 0),
    layers: view (key = 0),
    tray_items: view (key = 0),
    connector: custom (opt),
}
 */
use crate::dfs::dfs_helpers::format_object_link;
use crate::dfs::dfs_helpers::format_path_link;
use crate::dfs::dfs_helpers::format_workspace_link;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use crate::tree::OutputNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::WorkspaceNode;
use crate::tree::output::output_dfs_g_fuse::generated::output_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::FuseLinkView;
use crate::utils::fuse::fuse_views::IterDir;
use crate::utils::fuse::fuse_views::IterDirKeyed;
use crate::utils::fuse::fuse_views::IterDirKeyedView;
use crate::utils::fuse::fuse_views::IterDirView;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
pub use output_node::View as OutputNodeView;
use std::rc::Rc;
use std::str::FromStr;

impl OutputNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<output_node::View>().without_key()
    }
}

impl output_node::Dir for OutputNode {
    type ViewWorkspaces = IterDir<Workspaces>;
    type ViewLayers = IterDirKeyed<Layers>;
    type ViewTrayItems = IterDirKeyed<TrayItems>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.global.connector.name.as_str().str_fmt(buf, ctx);
    }

    fn read_status(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.status.get().as_str().str_fmt(buf, ctx);
    }

    fn read_is_dummy(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.is_dummy.str_fmt(buf, ctx);
    }

    fn read_title_visible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.title_visible.get().str_fmt(buf, ctx);
    }

    fn read_tearing(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tearing.get().str_fmt(buf, ctx);
    }

    fn read_damage_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.damage_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_hardware_cursor_needs_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.hardware_cursor_needs_render.get().str_fmt(buf, ctx);
    }

    fn read_update_render_data_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.update_render_data_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_has_hardware_cursor(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.hardware_cursor.get().is_some().str_fmt(buf, ctx);
    }

    fn read_render_margin_ns(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_margin_ns.get().str_fmt(buf, ctx);
    }

    fn read_flip_margin_ns(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.flip_margin_ns.get().str_fmt(buf, ctx);
    }

    fn read_has_lock_surface(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .lock_surface
            .get()
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_num_workspaces(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.workspaces.iter_valid(LiveTL).count().str_fmt(buf, ctx);
    }

    fn read_num_tray_items(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tray_items.iter_valid(LiveTL).count().str_fmt(buf, ctx);
    }

    fn read_num_screencasts(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.screencasts.len().str_fmt(buf, ctx);
    }

    fn read_num_screencopies(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.screencopies.len().str_fmt(buf, ctx);
    }

    fn read_num_jay_outputs(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.jay_outputs.len().str_fmt(buf, ctx);
    }

    fn read_num_cursor_users(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.cursor_users.len().str_fmt(buf, ctx);
    }

    fn read_pos_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].pos.get().str_fmt(buf, ctx);
    }

    fn read_pos_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL].pos.get().str_fmt(buf, ctx);
    }

    fn read_scale(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .scale
            .get()
            .to_f64()
            .str_fmt(buf, ctx);
    }

    fn read_legacy_scale(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].legacy_scale.get().str_fmt(buf, ctx);
    }

    fn read_non_exclusive(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .rects
            .non_exclusive
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_workspace_rect(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .rects
            .workspace
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_bar(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].rects.bar.get().str_fmt(buf, ctx);
    }

    fn read_exclusive_top(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_zones.get().top.str_fmt(buf, ctx);
    }

    fn read_exclusive_right(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_zones.get().right.str_fmt(buf, ctx);
    }

    fn read_exclusive_bottom(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_zones.get().bottom.str_fmt(buf, ctx);
    }

    fn read_exclusive_left(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_zones.get().left.str_fmt(buf, ctx);
    }

    fn read_tray_start_rel(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tray_start_rel.get().str_fmt(buf, ctx);
    }

    fn has_workspace(&self) -> bool {
        self.node_state[LiveTL].workspace.is_some()
    }

    fn readlink_workspace(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.node_state[LiveTL].workspace.get() {
            format_workspace_link(buf, depth, &t);
        }
    }

    fn has_overlay(&self) -> bool {
        self.node_state[LiveTL].overlay.get().is_some()
    }

    fn readlink_overlay(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.node_state[LiveTL].overlay.get() {
            format_workspace_link(buf, depth, &t);
        }
    }

    fn has_lock_surface(&self) -> bool {
        self.node_state[LiveTL].lock_surface.is_some()
    }

    fn readlink_lock_surface(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.node_state[LiveTL].lock_surface.get() {
            format_object_link(buf, depth, t.client.id, t.id);
        }
    }

    fn get_connector(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        self.global.connector.connector.clone().debugfs_link()
    }
}

struct Layers;

const LAYER_NAMES: [&str; 4] = ["background", "bottom", "top", "overlay"];

impl IterDirKeyedView<OutputNode> for Layers {
    type Value = OutputNode;
    type View = IterDir<Layer>;

    fn iter(t: Rc<OutputNode>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        for (idx, name) in LAYER_NAMES.into_iter().enumerate() {
            f(name, &t, idx as u64);
        }
    }

    fn get(t: Rc<OutputNode>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        let idx = LAYER_NAMES.iter().position(|n| *n == name)?;
        Some((t, idx as u64))
    }
}

struct Layer;

impl IterDirView<OutputNode> for Layer {
    type Value = ZwlrLayerSurfaceV1;
    type View = FuseLink<LayerLink>;

    fn iter(t: &OutputNode, key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
        let mut buf = itoa::Buffer::new();
        for ls in t.layers[key as usize].iter_valid(LiveTL) {
            f(buf.format(ls.id.raw()), &ls.item);
        }
    }

    fn get(t: &OutputNode, key: u64, name: &str) -> Option<Rc<Self::Value>> {
        let id = u64::from_str(name).ok()?;
        for ls in t.layers[key as usize].iter_valid(LiveTL) {
            if ls.id.raw() == id {
                return Some(ls.item.clone());
            }
        }
        None
    }
}

struct LayerLink;

impl FuseLinkView<ZwlrLayerSurfaceV1> for LayerLink {
    fn readlink(t: &ZwlrLayerSurfaceV1, _key: u64, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, t.client.id, t.id);
    }
}

struct TrayItems;

impl IterDirKeyedView<OutputNode> for TrayItems {
    type Value = OutputNode;
    type View = FuseLink<TrayItemLink>;

    fn iter(t: Rc<OutputNode>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let mut buf = itoa::Buffer::new();
        for item in t.tray_items.iter_valid(LiveTL) {
            let id = item.data().tray_item_id.raw();
            f(buf.format(id), &t, id);
        }
    }

    fn get(t: Rc<OutputNode>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        let id = u64::from_str(name).ok()?;
        for item in t.tray_items.iter_valid(LiveTL) {
            if item.data().tray_item_id.raw() == id {
                return Some((t, id));
            }
        }
        None
    }
}

struct TrayItemLink;

impl FuseLinkView<OutputNode> for TrayItemLink {
    fn readlink(t: &OutputNode, key: u64, depth: u64, buf: &mut String) {
        for item in t.tray_items.iter_valid(LiveTL) {
            let data = item.data();
            if data.tray_item_id.raw() == key {
                format_object_link(buf, depth, data.surface.client.id, item.object_id());
                return;
            }
        }
    }
}

struct Workspaces;

impl IterDirView<OutputNode> for Workspaces {
    type Value = WorkspaceNode;
    type View = FuseLink<WorkspaceLink>;

    fn iter(t: &OutputNode, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
        for node in t.workspaces.iter_valid(LiveTL) {
            let ws = &node.item;
            f(&ws.name, ws);
        }
    }

    fn get(t: &OutputNode, _key: u64, name: &str) -> Option<Rc<Self::Value>> {
        for node in t.workspaces.iter_valid(LiveTL) {
            if node.item.name.as_str() == name {
                return Some(node.item.clone());
            }
        }
        None
    }
}

struct WorkspaceLink;

impl FuseLinkView<WorkspaceNode> for WorkspaceLink {
    fn readlink(t: &WorkspaceNode, _key: u64, depth: u64, buf: &mut String) {
        format_path_link(buf, depth, "workspaces", &t.name);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_47696bb92bcd93ee6ae08077363adbffe11c5b61751d95e1a6f27dc947e378df.rs",
));
// FUSE GENERATED STOP
