use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::container::ContainerNode;
use crate::tree::{AbsoluteNode, FindTreeResult, FoundNode, Node, NodeId, OutputNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::LinkedList;
use std::rc::Rc;
use crate::cursor::KnownCursor;

tree_id!(WorkspaceNodeId);

pub struct WorkspaceNode {
    pub id: WorkspaceNodeId,
    pub output: CloneCell<Rc<OutputNode>>,
    pub container: CloneCell<Option<Rc<ContainerNode>>>,
    pub stacked: LinkedList<Rc<dyn AbsoluteNode>>,
    pub seat_state: NodeSeatState,
}

impl WorkspaceNode {
    pub fn set_container(self: &Rc<Self>, container: &Rc<ContainerNode>) {
        let output = self.output.get().position.get();
        container.clone().change_extents(&output);
        container.clone().set_workspace(self);
        self.container.set(Some(container.clone()));
    }
}

impl Node for WorkspaceNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.output.get().remove_child(self);
        }
        if let Some(container) = self.container.take() {
            container.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if let Some(n) = self.container.get() {
            tree.push(FoundNode {
                node: n.clone(),
                x,
                y,
            });
            n.find_tree_at(x, y, tree);
        }
        FindTreeResult::AcceptsInput
    }

    fn remove_child(&self, _child: &dyn Node) {
        self.container.set(None);
    }

    fn pointer_target(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_workspace(self, x, y);
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        if let Some(c) = self.container.get() {
            c.change_extents(rect);
        }
    }
}
