/*
dir layer_surface {
    @inherit dfs_node,
    @inherit dfs_object,
    namespace: reg,
    mapped: reg,
    destroyed: reg,
    layer: reg,
    anchor: reg,
    keyboard_interactivity: reg,
    exclusive_zone: reg,
    exclusive_zone_size: reg,
    exclusive_edge: reg,
    exclusive_size_top: reg,
    exclusive_size_right: reg,
    exclusive_size_bottom: reg,
    exclusive_size_left: reg,
    need_position_update: reg,
    num_popups: reg,
    is_linked: reg,
    pos: reg,
    output_extents: reg,
    size_width: reg,
    size_height: reg,
    margin_top: reg,
    margin_right: reg,
    margin_bottom: reg,
    margin_left: reg,
    last_configure_width: reg,
    last_configure_height: reg,
    output: link (opt),
    surface: link,
    popups: view (key = 0),
}
 */
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDir;
use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDirView;
use crate::dfs::dfs_helpers::DfsObjectCopyHashMap;
use crate::dfs::dfs_helpers::format_object_link;
use crate::dfs::dfs_helpers::format_output_link;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ExclusiveZone;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::layer_surface_dfs_g_fuse::generated::layer_surface;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl ZwlrLayerSurfaceV1 {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<layer_surface::View>().without_key()
    }
}

impl layer_surface::Dir for ZwlrLayerSurfaceV1 {
    type ViewPopups = DfsCopyHashMapObjectLinkDir<Popups>;

    fn read_namespace(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self._namespace.str_fmt(buf, ctx);
    }

    fn read_mapped(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mapped.get().str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_layer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.layer.get().str_fmt(buf, ctx);
    }

    fn read_anchor(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.anchor.get().str_fmt(buf, ctx);
    }

    fn read_keyboard_interactivity(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.keyboard_interactivity.get().str_fmt(buf, ctx);
    }

    fn read_exclusive_zone(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let zone = match self.exclusive_zone.get() {
            ExclusiveZone::MoveSelf => "move_self",
            ExclusiveZone::FixedSelf => "fixed_self",
            ExclusiveZone::Acquire(_) => "acquire",
        };
        zone.str_fmt(buf, ctx);
    }

    fn read_exclusive_zone_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let size = match self.exclusive_zone.get() {
            ExclusiveZone::Acquire(v) => Some(v),
            _ => None,
        };
        size.str_fmt(buf, ctx);
    }

    fn read_exclusive_edge(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_edge.get().str_fmt(buf, ctx);
    }

    fn read_exclusive_size_top(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_size.get().top.str_fmt(buf, ctx);
    }

    fn read_exclusive_size_right(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_size.get().right.str_fmt(buf, ctx);
    }

    fn read_exclusive_size_bottom(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_size.get().bottom.str_fmt(buf, ctx);
    }

    fn read_exclusive_size_left(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_size.get().left.str_fmt(buf, ctx);
    }

    fn read_need_position_update(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.need_position_update.get().str_fmt(buf, ctx);
    }

    fn read_num_popups(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.popups.len().str_fmt(buf, ctx);
    }

    fn read_is_linked(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.link.borrow().is_some().str_fmt(buf, ctx);
    }

    fn read_pos(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos.get().str_fmt(buf, ctx);
    }

    fn read_output_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.output_extents.get().str_fmt(buf, ctx);
    }

    fn read_size_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.size.get().0.str_fmt(buf, ctx);
    }

    fn read_size_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.size.get().1.str_fmt(buf, ctx);
    }

    fn read_margin_top(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.margin.get().0.str_fmt(buf, ctx);
    }

    fn read_margin_right(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.margin.get().1.str_fmt(buf, ctx);
    }

    fn read_margin_bottom(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.margin.get().2.str_fmt(buf, ctx);
    }

    fn read_margin_left(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.margin.get().3.str_fmt(buf, ctx);
    }

    fn read_last_configure_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.last_configure.get().0.str_fmt(buf, ctx);
    }

    fn read_last_configure_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.last_configure.get().1.str_fmt(buf, ctx);
    }

    fn has_output(&self) -> bool {
        self.output.global.is_some()
    }

    fn readlink_output(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.output.global.get() {
            format_output_link(buf, depth, &t);
        }
    }

    fn readlink_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.client.id, self.surface.id);
    }
}

struct Popups;

impl DfsCopyHashMapObjectLinkDirView<ZwlrLayerSurfaceV1> for Popups {
    fn client(t: &ZwlrLayerSurfaceV1) -> &Rc<Client> {
        &t.client
    }

    fn map(t: &ZwlrLayerSurfaceV1, _key: u64) -> &impl DfsObjectCopyHashMap {
        &t.popups
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_8d18882bedc87dd4b3a648cabb3e02c51a5ebbd9a052ab887032bc6806eb80d8.rs",
));
// FUSE GENERATED STOP
