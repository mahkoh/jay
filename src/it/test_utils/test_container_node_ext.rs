use crate::it::test_error::TestResult;
use crate::rect::Rect;
use crate::tree::ContainerChild;
use crate::tree::ContainerChildType;
use crate::tree::ContainerNode;
use crate::tree::NodeBase;
use crate::tree::NodeId;
use crate::tree::ToplevelNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::linkedlist::NodeRef;
use std::rc::Rc;

pub trait TestContainerExt {
    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>>;
    fn is_mono(&self) -> bool;
    fn child_ids(&self) -> Vec<NodeId>;
    fn child_body(&self, tl: &dyn ToplevelNode) -> TestResult<Rect>;
    fn child_title_rect(&self, tl: &dyn ToplevelNode) -> TestResult<Rect>;
    fn child_title_offsets(&self, tl: &dyn ToplevelNode) -> TestResult<(i32, i32, i32)>;
    fn child_ty(&self, tl: &dyn ToplevelNode) -> TestResult<ContainerChildType>;
    fn child_is_visible(&self, tl: &dyn ToplevelNode) -> TestResult<bool>;
}

fn child<T>(
    c: &ContainerNode,
    tl: &dyn ToplevelNode,
    f: impl FnOnce(&NodeRef<ContainerChild>) -> T,
) -> TestResult<T> {
    for node in c.children.iter_valid(LiveTL) {
        if node.node.node_id() == tl.node_id() {
            return Ok(f(&node));
        }
    }
    bail!(
        "Toplevel {} is not a child of container {}",
        tl.node_id(),
        c.node_id(),
    )
}

impl TestContainerExt for ContainerNode {
    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>> {
        match self.children.first() {
            None => bail!("container does not have children"),
            Some(c) => Ok(c.node.clone()),
        }
    }

    fn is_mono(&self) -> bool {
        self.node_state[LiveTL].mono_child.get().is_some()
    }

    fn child_ids(&self) -> Vec<NodeId> {
        self.children
            .iter_valid(LiveTL)
            .map(|c| c.node.node_id())
            .collect()
    }

    fn child_body(&self, tl: &dyn ToplevelNode) -> TestResult<Rect> {
        child(self, tl, |c| c.node_state[LiveTL].body.get())
    }

    fn child_title_rect(&self, tl: &dyn ToplevelNode) -> TestResult<Rect> {
        child(self, tl, |c| c.node_state[LiveTL].title_rect.get())
    }

    fn child_title_offsets(&self, tl: &dyn ToplevelNode) -> TestResult<(i32, i32, i32)> {
        child(self, tl, |c| {
            let offsets = &c.node_state[LiveTL].offsets;
            (
                offsets.overlay_icon.get(),
                offsets.toplevel_icon.get(),
                offsets.title.get(),
            )
        })
    }

    fn child_ty(&self, tl: &dyn ToplevelNode) -> TestResult<ContainerChildType> {
        child(self, tl, |c| c.node_state[LiveTL].ty.get())
    }

    fn child_is_visible(&self, tl: &dyn ToplevelNode) -> TestResult<bool> {
        child(self, tl, |c| c.node.node_visible(LiveTL))
    }
}
