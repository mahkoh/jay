/*
dir xwindow {
    @inherit dfs_toplevel_node,
    id: reg,
    window_id: reg,
    title: reg,
    class: reg,
    instance: reg,
    role: reg,
    pid: reg,
    surface_id: reg,
    surface_serial: reg,
    override_redirect: reg,
    mapped: reg,
    destroyed: reg,
    wants_floating: reg,
    never_focus: reg,
    utf8_title: reg,
    has_alpha: reg,
    modal: reg,
    fullscreen: reg,
    maximized_vert: reg,
    maximized_horz: reg,
    minimized: reg,
    extents: reg,
    pending_extents: reg,
    num_children: reg,
    has_parent: reg,
    is_mapped: reg,
    visible_live: reg,
    visible_render: reg,
    content_size: reg,
    desired_extents: reg,
    workspace_id: reg,
    surface: custom,
}
*/
use crate::ifs::wl_surface::x_surface::xwindow::Xwindow;
use crate::ifs::wl_surface::x_surface::xwindow::xwindow_dfs_g_fuse::generated::xwindow;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl Xwindow {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<xwindow::View>().without_key()
    }
}

impl xwindow::Dir for Xwindow {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_window_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.window_id.str_fmt(buf, ctx);
    }

    fn read_title(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.title.borrow().as_deref().str_fmt(buf, ctx);
    }

    fn read_class(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.class.borrow().as_deref().str_fmt(buf, ctx);
    }

    fn read_instance(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data
            .info
            .instance
            .borrow()
            .as_deref()
            .str_fmt(buf, ctx);
    }

    fn read_role(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.role.borrow().as_deref().str_fmt(buf, ctx);
    }

    fn read_pid(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.pid.get().str_fmt(buf, ctx);
    }

    fn read_surface_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data
            .surface_id
            .get()
            .map(|v| v.raw())
            .str_fmt(buf, ctx);
    }

    fn read_surface_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.surface_serial.get().str_fmt(buf, ctx);
    }

    fn read_override_redirect(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.override_redirect.get().str_fmt(buf, ctx);
    }

    fn read_mapped(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.mapped.get().str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_wants_floating(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.wants_floating.get().str_fmt(buf, ctx);
    }

    fn read_never_focus(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.never_focus.get().str_fmt(buf, ctx);
    }

    fn read_utf8_title(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.utf8_title.get().str_fmt(buf, ctx);
    }

    fn read_has_alpha(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.has_alpha.get().str_fmt(buf, ctx);
    }

    fn read_modal(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.modal.get().str_fmt(buf, ctx);
    }

    fn read_fullscreen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.fullscreen.get().str_fmt(buf, ctx);
    }

    fn read_maximized_vert(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.maximized_vert.get().str_fmt(buf, ctx);
    }

    fn read_maximized_horz(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.maximized_horz.get().str_fmt(buf, ctx);
    }

    fn read_minimized(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.minimized.get().str_fmt(buf, ctx);
    }

    fn read_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.extents.get().str_fmt(buf, ctx);
    }

    fn read_pending_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.info.pending_extents.get().str_fmt(buf, ctx);
    }

    fn read_num_children(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.children.len().str_fmt(buf, ctx);
    }

    fn read_has_parent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.parent.is_some().str_fmt(buf, ctx);
    }

    fn read_is_mapped(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.is_mapped().str_fmt(buf, ctx);
    }

    fn read_visible_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.visible[LiveTL].get().str_fmt(buf, ctx);
    }

    fn read_visible_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.visible[RenderTL].get().str_fmt(buf, ctx);
    }

    fn read_content_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.content_size.get().str_fmt(buf, ctx);
    }

    fn read_desired_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.desired_extents.get().str_fmt(buf, ctx);
    }

    fn read_workspace_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.workspace[LiveTL]
            .get()
            .map(|v| v.id.raw())
            .str_fmt(buf, ctx);
    }

    fn get_surface(self: &Rc<Self>, _key: u64) -> FuseInodeWithKey {
        self.x.surface.clone().node_debugfs()
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_50474d2bffbe8fc24729b536bdfcac811146a69dd7f64ee585a6cb0b44823129.rs",
));
// FUSE GENERATED STOP
