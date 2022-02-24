use crate::cursor::KnownCursor;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::walker::NodeVisitor;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::linkedlist::LinkedList;
use crate::{CloneCell, DisplayNode};
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;

tree_id!(OutputNodeId);
pub struct OutputNode {
    pub display: Rc<DisplayNode>,
    pub id: OutputNodeId,
    pub position: Cell<Rect>,
    pub global: Rc<WlOutputGlobal>,
    pub workspaces: RefCell<Vec<Rc<WorkspaceNode>>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub seat_state: NodeSeatState,
    pub layers: [LinkedList<Rc<ZwlrLayerSurfaceV1>>; 4],
}

impl Debug for OutputNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputNode").finish_non_exhaustive()
    }
}

impl Node for OutputNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.display.clone().remove_child(self);
        }
        let mut workspaces = self.workspaces.borrow_mut();
        for workspace in workspaces.drain(..) {
            workspace.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_output(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let ws = self.workspaces.borrow_mut();
        for ws in ws.deref() {
            visitor.visit_workspace(ws);
        }
        for layers in &self.layers {
            for surface in layers.iter() {
                visitor.visit_layer_surface(surface.deref());
            }
        }
    }

    fn absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if let Some(ws) = self.workspace.get() {
            tree.push(FoundNode {
                node: ws.clone(),
                x,
                y,
            });
            ws.find_tree_at(x, y, tree);
        }
        FindTreeResult::AcceptsInput
    }

    fn remove_child(self: Rc<Self>, _child: &dyn Node) {
        self.workspace.set(None);
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_output(self, x, y);
    }

    fn is_output(&self) -> bool {
        true
    }

    fn into_output(self: Rc<Self>) -> Option<Rc<OutputNode>> {
        Some(self)
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        self.position.set(*rect);
        if let Some(c) = self.workspace.get() {
            c.change_extents(rect);
        }
        for layer in &self.layers {
            for surface in layer.iter() {
                surface.deref().clone().change_extents(rect);
            }
        }
    }
}
