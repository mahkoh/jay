/*
dir placeholder_node {
    @inherit dfs_toplevel_node,
    id: reg,
    title: reg,
    app_id: reg,
    destroyed: reg,
    update_textures_scheduled: reg,
    visible_live: reg,
    visible_render: reg,
    content_size: reg,
    desired_extents: reg,
    pinned: reg,
    is_fullscreen_live: reg,
    wants_attention: reg,
    has_parent: reg,
    workspace_id: reg,
    output_id: reg,
}
*/
use crate::tree::NodeBase;
use crate::tree::PlaceholderNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::placeholder::placeholder_dfs_g_fuse::generated::placeholder_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl PlaceholderNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<placeholder_node::View>().without_key()
    }
}

impl placeholder_node::Dir for PlaceholderNode {
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

    fn read_update_textures_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.update_textures_scheduled.get().str_fmt(buf, ctx);
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

    fn read_pinned(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel.pinned.get().str_fmt(buf, ctx);
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
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_2f6edcf9174570018812c411b00c945e0b9d3fd8ca43ee48c36a93364079e333.rs",
));
// FUSE GENERATED STOP
