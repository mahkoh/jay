use std::ops::Deref;
use crate::render::Renderer;
use crate::tree::container::ContainerNode;
use crate::tree::{FoundNode, Node, NodeId, OutputNode, StackedNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::LinkedList;
use std::rc::Rc;
use crate::rect::Rect;

tree_id!(WorkspaceNodeId);

pub struct WorkspaceNode {
    pub id: WorkspaceNodeId,
    pub output: CloneCell<Rc<OutputNode>>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub stacked: LinkedList<StackedNode>,
}

impl WorkspaceNode {
    pub fn set_container(&self, container: &Rc<ContainerNode>) {
        let output = self.output.get().position.get();
        container
            .clone()
            .change_extents(&output);
        self.container.set(Some(container.clone()));
    }
}

impl Node for WorkspaceNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn clear(&self) {
        if let Some(child) = self.container.take() {
            child.clear();
        }
    }

    fn find_child_at(&self, x: i32, y: i32) -> Option<FoundNode> {
        for stacked in self.stacked.rev_iter() {
            let (pos, node) = match stacked.deref() {
                StackedNode::Float(f) => (f.position.get(), &**f as &dyn Node),
                StackedNode::Popup(p) => (p.xdg.absolute_desired_extents.get(), &**p as &dyn Node),
            };
            if pos.contains(x, y) {
                let (x, y) = pos.translate(x, y);
                if let Some(n) = node.find_child_at(x, y) {
                    return Some(n);
                }
            }
        }
        match self.container.get() {
            Some(node) => Some(FoundNode {
                node,
                x,
                y,
                contained: true,
            }),
            _ => None,
        }
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_workspace(self, x, y);
    }

    fn get_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        Some(self)
    }

    fn remove_child(&self, _child: &dyn Node) {
        self.container.set(None);
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        if let Some(c) = self.container.get() {
            c.change_extents(rect);
        }
    }
}
