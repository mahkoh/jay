/*
dir workspace_node {
    @inherit dfs_node,
    id: reg,
    name: reg,
    output: link,
    visible: reg,
    visible_on_desired_output: reg,
    may_capture: reg,
    has_capture: reg,
    was_on_dummy_output: reg,
    has_container: reg,
    has_fullscreen: reg,
    num_stacked: reg,
    num_jay_workspaces: reg,
    num_ext_workspaces: reg,
    position_live: reg,
    position_render: reg,
    container: custom (opt),
    fullscreen: custom (opt),
}
 */
use crate::dfs::dfs_helpers::format_output_link;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::WorkspaceNode;
use crate::tree::workspace::workspace_dfs_g_fuse::generated::workspace_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;
pub use workspace_node::View as WorkspaceNodeView;

impl WorkspaceNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<workspace_node::View>().without_key()
    }
}

impl workspace_node::Dir for WorkspaceNode {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.name.as_str().str_fmt(buf, ctx);
    }

    fn readlink_output(&self, depth: u64, buf: &mut String) {
        let output = self.node_state[LiveTL].output.get();
        format_output_link(buf, depth, &output.global);
    }

    fn read_visible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].visible.get().str_fmt(buf, ctx);
    }

    fn read_visible_on_desired_output(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.visible_on_desired_output.get().str_fmt(buf, ctx);
    }

    fn read_may_capture(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.may_capture.get().str_fmt(buf, ctx);
    }

    fn read_has_capture(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.has_capture.get().str_fmt(buf, ctx);
    }

    fn read_was_on_dummy_output(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.was_on_dummy_output.get().str_fmt(buf, ctx);
    }

    fn read_has_container(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .container
            .get()
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_has_fullscreen(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .fullscreen
            .get()
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_num_stacked(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.stacked.iter().count().str_fmt(buf, ctx);
    }

    fn read_num_jay_workspaces(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.jay_workspaces.len().str_fmt(buf, ctx);
    }

    fn read_num_ext_workspaces(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ext_workspaces.len().str_fmt(buf, ctx);
    }

    fn read_position_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].position.get().str_fmt(buf, ctx);
    }

    fn read_position_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL].position.get().str_fmt(buf, ctx);
    }

    fn get_container(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        let container = self.node_state[LiveTL].container.get()?;
        Some(container.node_debugfs())
    }

    fn get_fullscreen(self: &Rc<Self>, _key: u64) -> Option<FuseInodeWithKey> {
        let fullscreen = self.node_state[LiveTL].fullscreen.get()?;
        Some(fullscreen.node_debugfs_dyn())
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_e1e506db10f12d3b438cf3a32682e5f62d39ea0cbf1d7a18566d681f2d01b7ac.rs",
));
// FUSE GENERATED STOP
