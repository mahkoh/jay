/*
dir input_popup {
    @inherit dfs_object,
    positioning_scheduled: reg,
    was_on_screen: reg,
    surface: link,
    input_method: link,
}
 */
use crate::dfs::dfs_helpers::format_object_link;
use crate::ifs::wl_surface::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2;
use crate::ifs::wl_surface::zwp_input_popup_surface_v2::zwp_input_popup_surface_v2_dfs_g_fuse::generated::input_popup;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl ZwpInputPopupSurfaceV2 {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<input_popup::View>().without_key()
    }
}

impl input_popup::Dir for ZwpInputPopupSurfaceV2 {
    fn read_positioning_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.positioning_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_was_on_screen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.was_on_screen.get().str_fmt(buf, ctx);
    }

    fn readlink_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.client.id, self.surface.id);
    }

    fn readlink_input_method(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.client.id, self.input_method.id);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_e4cdb34d0c80079884128581525763872ab01930c21a692b31c7ebce9fc46946.rs",
));
// FUSE GENERATED STOP
