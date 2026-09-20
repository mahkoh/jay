/*
dir float_node {
    @inherit dfs_node,
    id: reg,
    title: reg,
    visible_live: reg,
    visible_render: reg,
    requested_visible_live: reg,
    position_live: reg,
    position_render: reg,
    title_rect_live: reg,
    active_live: reg,
    attention_requested_live: reg,
    pinned_live: reg,
    child_is_placeholder_live: reg,
    is_pinned_globally: reg,
    layout_scheduled: reg,
    render_titles_scheduled: reg,
    needs_initial_size: reg,
    workspace_id: reg,
    child: custom (opt),
}
*/
use crate::tree::FloatNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::float::float_dfs_g_fuse::generated::float_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl FloatNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<float_node::View>().without_key()
    }
}

impl float_node::Dir for FloatNode {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_title(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.title.borrow().as_str().str_fmt(buf, ctx);
    }

    fn read_visible_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].visible.get().str_fmt(buf, ctx);
    }

    fn read_visible_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL].visible.get().str_fmt(buf, ctx);
    }

    fn read_requested_visible_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .requested_visible
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_position_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].position.get().str_fmt(buf, ctx);
    }

    fn read_position_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL].position.get().str_fmt(buf, ctx);
    }

    fn read_title_rect_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].title_rect.get().str_fmt(buf, ctx);
    }

    fn read_active_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].active.get().str_fmt(buf, ctx);
    }

    fn read_attention_requested_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .attention_requested
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_pinned_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].pinned.get().str_fmt(buf, ctx);
    }

    fn read_child_is_placeholder_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .child_is_placeholder
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_is_pinned_globally(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pinned_link.borrow().is_some().str_fmt(buf, ctx);
    }

    fn read_layout_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.layout_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_render_titles_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_titles_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_needs_initial_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.needs_initial_size.get().str_fmt(buf, ctx);
    }

    fn read_workspace_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.workspace.get().id.raw().str_fmt(buf, ctx);
    }

    fn get_child(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        let child = self.node_state[LiveTL].child.get()?;
        Some(child.node_debugfs_dyn())
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_8fc03c3458368436aa99922e4a4a16cca6e2740e95162f28afbfe62d2fea2103.rs",
));
// FUSE GENERATED STOP
