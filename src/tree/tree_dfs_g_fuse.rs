/*
dir dfs_node_seat_state (global) {
    no_focus_history: reg,
}

dir dfs_node (abstract, global) {
    node_type: reg,
    node_id: reg,
    node_seat_state: view (key = 0),
    node_visible: reg,
    node_absolute_position: reg,
    node_output: link (opt),
    node_workspace: link (opt),
    node_layer: reg,
    node_accepts_focus: reg,
    node_client: link (opt),
}

dir node {
    @inherit dfs_node,
}
 */
use crate::dfs::dfs_helpers::dfs_split_view;
use crate::dfs::dfs_helpers::format_client_link;
use crate::dfs::dfs_helpers::format_output_link;
use crate::dfs::dfs_helpers::format_workspace_link;
use crate::tree::Node;
use crate::tree::tree_dfs_g_fuse::generated::node;
use crate::utils::fuse::fuse_globals::dfs_node;
use crate::utils::fuse::fuse_globals::dfs_node_seat_state;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
pub use node::View as NodeView;
use std::any::type_name;

impl<T> dfs_node::Dir for T
where
    T: Node,
{
    type ViewNodeSeatState = dfs_node_seat_state::View;

    fn read_node_type(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        type_name::<T>().str_fmt(buf, ctx);
    }

    fn read_node_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_id().raw().str_fmt(buf, ctx);
    }

    fn read_node_visible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| self.node_visible(v));
    }

    fn read_node_absolute_position(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dfs_split_view(buf, ctx, |v| self.node_absolute_position(v));
    }

    fn has_node_output(&self) -> bool {
        self.node_output_id().is_some()
    }

    fn readlink_node_output(&self, depth: u64, buf: &mut String) {
        if let Some(output) = self.node_output() {
            format_output_link(buf, depth, &output.global);
        }
    }

    fn has_node_workspace(&self) -> bool {
        self.node_workspace().is_some()
    }

    fn readlink_node_workspace(&self, depth: u64, buf: &mut String) {
        if let Some(ws) = self.node_workspace() {
            format_workspace_link(buf, depth, &ws);
        }
    }

    fn read_node_layer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_layer().layer().text().str_fmt(buf, ctx);
    }

    fn read_node_accepts_focus(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_accepts_focus().str_fmt(buf, ctx);
    }

    fn has_node_client(&self) -> bool {
        self.node_client_id().is_some()
    }

    fn readlink_node_client(&self, depth: u64, buf: &mut String) {
        if let Some(id) = self.node_client_id() {
            format_client_link(buf, depth, id);
        }
    }
}

impl<T> node::Dir for T where T: Node {}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_7fb9f2b988e6f084399e05fef8de29f57e7cdad26830bb7d02986a6d267eff67.rs",
));
// FUSE GENERATED STOP
