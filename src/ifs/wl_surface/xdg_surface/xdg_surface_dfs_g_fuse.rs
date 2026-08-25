/*
dir xdg_surface {
    @inherit dfs_object,
    role: reg,
    acked_serial: reg,
    destroyed: reg,
    num_popups: reg,
    has_workspace: reg,
    geometry: reg,
    extents: reg,
    effective_geometry_live: reg,
    effective_geometry_render: reg,
    absolute_desired_extents_live: reg,
    absolute_desired_extents_render: reg,
    surface: link,
    role_obj: link (opt),
    popups: view (key = 0),
}
 */
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDir;
use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDirView;
use crate::dfs::dfs_helpers::DfsObjectCopyHashMap;
use crate::dfs::dfs_helpers::format_object_link;
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::wl_surface::xdg_surface::xdg_surface_dfs_g_fuse::generated::xdg_surface;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl XdgSurface {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<xdg_surface::View>().without_key()
    }
}

impl xdg_surface::Dir for XdgSurface {
    type ViewPopups = DfsCopyHashMapObjectLinkDir<Popups>;

    fn read_role(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.role.get().name().str_fmt(buf, ctx);
    }

    fn read_acked_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.acked_serial.get().map(|s| s.raw()).str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_num_popups(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.popups.len().str_fmt(buf, ctx);
    }

    fn read_has_workspace(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.workspace.get().is_some().str_fmt(buf, ctx);
    }

    fn read_geometry(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.geometry.get().str_fmt(buf, ctx);
    }

    fn read_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.extents.get().str_fmt(buf, ctx);
    }

    fn read_effective_geometry_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.effective_geometry[LiveTL].get().str_fmt(buf, ctx);
    }

    fn read_effective_geometry_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.effective_geometry[RenderTL].get().str_fmt(buf, ctx);
    }

    fn read_absolute_desired_extents_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.absolute_desired_extents[LiveTL]
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_absolute_desired_extents_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.absolute_desired_extents[RenderTL]
            .get()
            .str_fmt(buf, ctx);
    }

    fn readlink_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.surface.client.id, self.surface.id);
    }

    fn has_role_obj(&self) -> bool {
        self.ext.is_some()
    }

    fn readlink_role_obj(&self, depth: u64, buf: &mut String) {
        if let Some(ext) = self.ext.get() {
            format_object_link(buf, depth, self.surface.client.id, ext.object_id());
        }
    }
}

struct Popups;

impl DfsCopyHashMapObjectLinkDirView<XdgSurface> for Popups {
    fn client(t: &XdgSurface) -> &Rc<Client> {
        &t.surface.client
    }

    fn map(t: &XdgSurface, _key: u64) -> &impl DfsObjectCopyHashMap {
        &t.popups
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_e37c4ac761d6f54aa230864e9bfccc6df454acfd118614314078b7a8303128a2.rs",
));
// FUSE GENERATED STOP
