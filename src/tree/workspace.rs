use crate::render::Renderer;
use crate::tree::container::ContainerNode;
use crate::tree::{FloatNode, FoundNode, Node, NodeId, OutputNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::LinkedList;
use std::rc::Rc;

tree_id!(WorkspaceNodeId);

pub struct WorkspaceNode {
    pub id: WorkspaceNodeId,
    pub output: CloneCell<Rc<OutputNode>>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub floaters: LinkedList<Rc<FloatNode>>,
}

impl WorkspaceNode {
    pub fn set_container(&self, container: &Rc<ContainerNode>) {
        let output = self.output.get().position.get();
        container
            .clone()
            .change_size(output.width(), output.height());
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

    fn change_size(self: Rc<Self>, width: i32, height: i32) {
        if let Some(c) = self.container.get() {
            c.change_size(width, height);
        }
    }
}
