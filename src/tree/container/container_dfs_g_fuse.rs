/*
dir container_node {
    @inherit dfs_toplevel_node,
    id: reg,
    title: reg,
    split_live: reg,
    split_render: reg,
    is_mono_live: reg,
    is_mono_render: reg,
    num_children_live: reg,
    num_children_render: reg,
    sum_factors: reg,
    position_live: reg,
    position_render: reg,
    content_width_live: reg,
    content_height_live: reg,
    mono_body_live: reg,
    mono_content_live: reg,
    layout_scheduled: reg,
    compute_render_positions_scheduled: reg,
    render_titles_scheduled: reg,
    is_root_container_live: reg,
    is_overlay_root_container: reg,
    workspace_id: reg,
    children: view (key = 0),
}
*/
use crate::rect::Rect;
use crate::tree::ContainerNode;
use crate::tree::ContainerNodeState;
use crate::tree::NodeId;
use crate::tree::TreeTimeline;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::container::container_dfs_g_fuse::generated::container_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::IterDirDyn;
use crate::utils::fuse::fuse_views::IterDirDynView;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;
use std::str::FromStr;

impl ContainerNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<container_node::View>().without_key()
    }

    fn dfs_position(&self, tl: TreeTimeline) -> Rect {
        let ContainerNodeState {
            abs_x1,
            abs_y1,
            width,
            height,
            ..
        } = &self.node_state[tl];
        Rect::new_sized_saturating(abs_x1.get(), abs_y1.get(), width.get(), height.get())
    }
}

impl container_node::Dir for ContainerNode {
    type ViewChildren = IterDirDyn<Children>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_title(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.title.borrow().as_str().str_fmt(buf, ctx);
    }

    fn read_split_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].split.get().text().str_fmt(buf, ctx);
    }

    fn read_split_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL]
            .split
            .get()
            .text()
            .str_fmt(buf, ctx);
    }

    fn read_is_mono_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .mono_child
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_is_mono_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL]
            .mono_child
            .is_some()
            .str_fmt(buf, ctx);
    }

    fn read_num_children_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].num_children.get().str_fmt(buf, ctx);
    }

    fn read_num_children_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL]
            .num_children
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_sum_factors(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.sum_factors.get().str_fmt(buf, ctx);
    }

    fn read_position_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dfs_position(LiveTL).str_fmt(buf, ctx);
    }

    fn read_position_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dfs_position(RenderTL).str_fmt(buf, ctx);
    }

    fn read_content_width_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .content_width
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_content_height_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL]
            .content_height
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_mono_body_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].mono_body.get().str_fmt(buf, ctx);
    }

    fn read_mono_content_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].mono_content.get().str_fmt(buf, ctx);
    }

    fn read_layout_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.layout_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_compute_render_positions_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.compute_render_positions_scheduled
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_render_titles_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_titles_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_is_root_container_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.is_root_container[LiveTL]
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_is_overlay_root_container(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.toplevel_data.is_overlay_root_container[LiveTL]
            .get()
            .str_fmt(buf, ctx);
    }

    fn read_workspace_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.workspace.get().id.raw().str_fmt(buf, ctx);
    }
}

pub struct Children;

impl IterDirDynView<ContainerNode> for Children {
    fn iter(t: &Rc<ContainerNode>, _key: u64, mut f: impl FnMut(&str, FuseInodeWithKey)) {
        let mut buf = itoa::Buffer::new();
        for child in t.children.iter_valid(LiveTL) {
            f(
                buf.format(child.node.node_id().raw()),
                child.node.clone().node_debugfs_dyn(),
            );
        }
    }

    fn get(t: &Rc<ContainerNode>, _key: u64, name: &str) -> Option<FuseInodeWithKey> {
        let id = u32::from_str(name).ok()?;
        let child_nodes = t.child_nodes.borrow();
        let child = child_nodes.get(&NodeId(id))?;
        if !child.valid[LiveTL].get() {
            return None;
        }
        Some(child.node.clone().node_debugfs_dyn())
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_bebc296848726f5eea0ca352e4fce7009690a6fd5b5f620c56bc2bf837b41f39.rs",
));
// FUSE GENERATED STOP
