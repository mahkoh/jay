/*
dir display_node {
    @inherit dfs_node,
    id: reg,
    extents_live: reg,
    extents_render: reg,
    num_outputs: reg,
    num_stacked: reg,
    num_stacked_above_layers: reg,
    num_stacked_in_overlay: reg,
    outputs: view (key = 0),
    stacked: view (key = 0),
    stacked_above_layers: view (key = 0),
    stacked_in_overlay: view (key = 0),
}
 */
use crate::dfs::dfs_helpers::format_output_link;
use crate::tree::DisplayNode;
use crate::tree::NodesStack;
use crate::tree::OutputNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::display::display_dfs_g_fuse::generated::display_node;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::FuseLinkView;
use crate::utils::fuse::fuse_views::IterDir;
use crate::utils::fuse::fuse_views::IterDirDyn;
use crate::utils::fuse::fuse_views::IterDirDynView;
use crate::utils::fuse::fuse_views::IterDirView;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

impl DisplayNode {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<display_node::View>().without_key()
    }
}

impl display_node::Dir for DisplayNode {
    type ViewOutputs = IterDir<DisplayOutputs>;
    type ViewStacked = IterDirDyn<StackedDir<Stacked>>;
    type ViewStackedAboveLayers = IterDirDyn<StackedDir<StackedAboveLayers>>;
    type ViewStackedInOverlay = IterDirDyn<StackedDir<StackedInOverlay>>;

    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_extents_live(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[LiveTL].extents.get().str_fmt(buf, ctx);
    }

    fn read_extents_render(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.node_state[RenderTL].extents.get().str_fmt(buf, ctx);
    }

    fn read_num_outputs(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.outputs.len().str_fmt(buf, ctx);
    }

    fn read_num_stacked(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.stacked.stacked.iter().count().str_fmt(buf, ctx);
    }

    fn read_num_stacked_above_layers(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.stacked_above_layers
            .stacked
            .iter()
            .count()
            .str_fmt(buf, ctx);
    }

    fn read_num_stacked_in_overlay(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.stacked_in_overlay
            .stacked
            .iter()
            .count()
            .str_fmt(buf, ctx);
    }
}

struct DisplayOutputs;

impl IterDirView<DisplayNode> for DisplayOutputs {
    type Value = OutputNode;
    type View = FuseLink<OutputLink>;

    fn iter(t: &DisplayNode, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
        for output in t.outputs.lock().values() {
            f(&output.global.connector.name, output);
        }
    }

    fn get(t: &DisplayNode, _key: u64, name: &str) -> Option<Rc<Self::Value>> {
        for output in t.outputs.lock().values() {
            if output.global.connector.name.as_str() == name {
                return Some(output.clone());
            }
        }
        None
    }
}

struct OutputLink;

impl FuseLinkView<OutputNode> for OutputLink {
    fn readlink(t: &OutputNode, _key: u64, depth: u64, buf: &mut String) {
        format_output_link(buf, depth, &t.global);
    }
}

/// Selects one of the display's stacks.
trait StackSelector: 'static {
    fn stack(t: &DisplayNode) -> &Rc<NodesStack>;
}

struct Stacked;

impl StackSelector for Stacked {
    fn stack(t: &DisplayNode) -> &Rc<NodesStack> {
        &t.stacked
    }
}

struct StackedAboveLayers;

impl StackSelector for StackedAboveLayers {
    fn stack(t: &DisplayNode) -> &Rc<NodesStack> {
        &t.stacked_above_layers
    }
}

struct StackedInOverlay;

impl StackSelector for StackedInOverlay {
    fn stack(t: &DisplayNode) -> &Rc<NodesStack> {
        &t.stacked_in_overlay
    }
}

struct StackedDir<S>(PhantomData<fn() -> S>);

impl<S> IterDirDynView<DisplayNode> for StackedDir<S>
where
    S: StackSelector,
{
    fn iter(t: &Rc<DisplayNode>, _key: u64, mut f: impl FnMut(&str, FuseInodeWithKey)) {
        let mut buf = itoa::Buffer::new();
        for node in S::stack(&t).stacked.iter() {
            f(
                buf.format(node.node_id().raw()),
                node.deref().clone().node_debugfs_dyn(),
            );
        }
    }

    fn get(t: &Rc<DisplayNode>, _key: u64, name: &str) -> Option<FuseInodeWithKey> {
        let id = u32::from_str(name).ok()?;
        for node in S::stack(&t).stacked.iter() {
            if node.node_id().raw() == id {
                return Some(node.deref().clone().node_debugfs_dyn());
            }
        }
        None
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_4fbd32a10984802e2ddbf4f56f3bfaecca34e33ad906da55faf2ae7b83e70201.rs",
));
// FUSE GENERATED STOP
