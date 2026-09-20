/*
dir lock_surface {
    @inherit dfs_node,
    @inherit dfs_object,
    destroyed: reg,
    desired_width: reg,
    desired_height: reg,
    output: link (opt),
    surface: link,
}
 */
use crate::dfs::dfs_helpers::format_object_link;
use crate::dfs::dfs_helpers::format_output_link;
use crate::ifs::wl_surface::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use crate::ifs::wl_surface::ext_session_lock_surface_v1::session_lock_surface_dfs_g_fuse::generated::lock_surface;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl ExtSessionLockSurfaceV1 {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<lock_surface::View>().without_key()
    }
}

impl lock_surface::Dir for ExtSessionLockSurfaceV1 {
    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_desired_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.desired_size.get().width().str_fmt(buf, ctx);
    }

    fn read_desired_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.desired_size.get().height().str_fmt(buf, ctx);
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

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_86284c232f9d07a3f759977d1723d45d3210da193e258237dd3ca95bf253ba6d.rs",
));
// FUSE GENERATED STOP
