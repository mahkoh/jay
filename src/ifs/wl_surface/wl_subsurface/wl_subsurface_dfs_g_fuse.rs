/*
dir subsurface {
    @inherit dfs_object,
    parent: link,
    child: link,
}
 */
use crate::dfs::dfs_helpers::format_object_link;
use crate::ifs::wl_surface::wl_subsurface::WlSubsurface;
use crate::ifs::wl_surface::wl_subsurface::wl_subsurface_dfs_g_fuse::generated::subsurface::Dir;
use crate::ifs::wl_surface::wl_subsurface::wl_subsurface_dfs_g_fuse::generated::subsurface::View;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl WlSubsurface {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<View>().without_key()
    }
}

impl Dir for WlSubsurface {
    fn readlink_parent(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.parent.client.id, self.parent.id);
    }

    fn readlink_child(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.surface.client.id, self.surface.id);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_ea7e68d5ad2e6e99f50f2c3d925818652f88b0e16e5dbccdce3f7dbeac9e33d6.rs",
));
// FUSE GENERATED STOP
