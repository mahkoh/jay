/*
dir pending_fullscreen_node {
    @inherit dfs_toplevel_node,
    id: reg,
    title: reg,
    app_id: reg,
    destroyed: reg,
    visible_live: reg,
    visible_render: reg,
    content_size: reg,
    desired_extents: reg,
    is_fullscreen_live: reg,
    wants_attention: reg,
    has_parent: reg,
    workspace_id: reg,
    output_id: reg,
    child: custom (opt),
}
*/
use crate::tree::NodeBase;
use crate::tree::PendingFullscreenNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::pendingfullscreen::pendingfullscreen_dfs_g_fuse::generated::pending_fullscreen_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl PendingFullscreenNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<pending_fullscreen_node::View>()
            .without_key()
    }
}

impl pending_fullscreen_node::Dir for PendingFullscreenNode {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_title(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.title.borrow().as_str().str_fmt(buf, ctx);
    }

    fn read_app_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.app_id.borrow().as_str().str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_visible_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.visible[LiveTL].get().str_fmt(buf, ctx);
    }

    fn read_visible_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.visible[RenderTL].get().str_fmt(buf, ctx);
    }

    fn read_content_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.content_size.get().str_fmt(buf, ctx);
    }

    fn read_desired_extents(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.desired_extents.get().str_fmt(buf, ctx);
    }

    fn read_is_fullscreen_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.is_fullscreen[LiveTL].get().str_fmt(buf, ctx);
    }

    fn read_wants_attention(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.wants_attention.get().str_fmt(buf, ctx);
    }

    fn read_has_parent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.parent.is_some().str_fmt(buf, ctx);
    }

    fn read_workspace_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.workspace[LiveTL]
            .get()
            .map(|v| v.id.raw())
            .str_fmt(buf, ctx);
    }

    fn read_output_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_output_id().map(|v| v.raw()).str_fmt(buf, ctx);
    }

    fn get_child(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        self.child.get().map(|c| c.node_debugfs())
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_513a1d0190beae012b9cbcaea8e2771bdf9ac304b1837066815fb8b7f5a6e7ec.rs",
));
// FUSE GENERATED STOP
