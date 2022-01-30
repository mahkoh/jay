use crate::backend::{KeyState, Output, OutputId, ScrollAxis};
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::rect::Rect;
use crate::render::Renderer;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::{LinkedList, LinkedNode};
use crate::NumCell;
pub use container::*;
use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::ops::Deref;
use std::rc::Rc;
pub use workspace::*;

mod container;
mod workspace;

pub struct NodeIds {
    next: NumCell<u32>,
}

impl Default for NodeIds {
    fn default() -> Self {
        Self {
            next: NumCell::new(1),
        }
    }
}

impl NodeIds {
    pub fn next<T: From<NodeId>>(&self) -> T {
        NodeId(self.next.fetch_add(1)).into()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId(pub u32);

impl NodeId {
    #[allow(dead_code)]
    pub fn raw(&self) -> u32 {
        self.0
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub enum FindTreeResult {
    AcceptsInput,
    Other,
}

pub trait AbsoluteNode: Node {
    fn into_node(self: Rc<Self>) -> Rc<dyn Node>;

    fn absolute_position(&self) -> Rect;
}

pub trait Node {
    fn id(&self) -> NodeId;
    fn seat_state(&self) -> &NodeSeatState;
    fn destroy_node(&self, detach: bool);

    fn button(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        let _ = seat;
        let _ = button;
        let _ = state;
    }

    fn scroll(&self, seat: &WlSeatGlobal, delta: i32, axis: ScrollAxis) {
        let _ = seat;
        let _ = delta;
        let _ = axis;
    }

    fn focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn unfocus(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let _ = x;
        let _ = y;
        let _ = tree;
        FindTreeResult::Other
    }

    fn remove_child(&self, child: &dyn Node) {
        let _ = child;
    }

    fn child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        let _ = (child, width, height);
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn motion(&self, seat: &WlSeatGlobal, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        let _ = renderer;
        let _ = x;
        let _ = y;
    }

    fn into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        None
    }

    fn into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>> {
        None
    }

    fn is_float(&self) -> bool {
        false
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        let _ = rect;
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        let _ = ws;
    }
}

pub struct FoundNode {
    pub node: Rc<dyn Node>,
    pub x: i32,
    pub y: i32,
}

tree_id!(ToplevelNodeId);

pub struct DisplayNode {
    pub id: NodeId,
    pub outputs: CopyHashMap<OutputId, Rc<OutputNode>>,
    pub stacked: LinkedList<Rc<dyn AbsoluteNode>>,
    pub seat_state: NodeSeatState,
}

impl DisplayNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            outputs: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
        }
    }
}

impl Node for DisplayNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let mut outputs = self.outputs.lock();
        for output in outputs.values() {
            output.destroy_node(false);
        }
        outputs.clear();
        for stacked in self.stacked.iter() {
            stacked.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        for stacked in self.stacked.iter() {
            let ext = stacked.absolute_position();
            if !ext.contains(x, y) {
                continue;
            }
            let (x, y) = ext.translate(x, y);
            let idx = tree.len();
            tree.push(FoundNode {
                node: stacked.deref().clone().into_node(),
                x,
                y,
            });
            match stacked.find_tree_at(x, y, tree) {
                FindTreeResult::AcceptsInput => return FindTreeResult::AcceptsInput,
                FindTreeResult::Other => {
                    tree.drain(idx..);
                }
            }
        }
        let outputs = self.outputs.lock();
        for output in outputs.values() {
            let pos = output.position.get();
            if pos.contains(x, y) {
                let (x, y) = pos.translate(x, y);
                tree.push(FoundNode {
                    node: output.clone(),
                    x,
                    y,
                });
                output.find_tree_at(x, y, tree);
                break;
            }
        }
        FindTreeResult::AcceptsInput
    }
}

tree_id!(OutputNodeId);
pub struct OutputNode {
    pub display: Rc<DisplayNode>,
    pub id: OutputNodeId,
    pub position: Cell<Rect>,
    pub backend: Rc<dyn Output>,
    pub workspaces: RefCell<Vec<Rc<WorkspaceNode>>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub seat_state: NodeSeatState,
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
            self.display.remove_child(self);
        }
        let mut workspaces = self.workspaces.borrow_mut();
        for workspace in workspaces.drain(..) {
            workspace.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
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

    fn remove_child(&self, _child: &dyn Node) {
        self.workspace.set(None);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_output(self, x, y);
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        self.position.set(*rect);
        if let Some(c) = self.workspace.get() {
            c.change_extents(rect);
        }
    }
}

tree_id!(FloatNodeId);
pub struct FloatNode {
    pub id: FloatNodeId,
    pub visible: Cell<bool>,
    pub position: Cell<Rect>,
    pub display: Rc<DisplayNode>,
    pub display_link: Cell<Option<LinkedNode<Rc<dyn AbsoluteNode>>>>,
    pub workspace_link: Cell<Option<LinkedNode<Rc<dyn AbsoluteNode>>>>,
    pub workspace: CloneCell<Rc<WorkspaceNode>>,
    pub child: CloneCell<Option<Rc<dyn Node>>>,
    pub seat_state: NodeSeatState,
}

impl AbsoluteNode for FloatNode {
    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        self
    }

    fn absolute_position(&self) -> Rect {
        self.position.get()
    }
}

impl Node for FloatNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let _v = self.display_link.take();
        let _v = self.workspace_link.take();
        if let Some(child) = self.child.get() {
            child.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let child = match self.child.get() {
            Some(c) => c,
            _ => return FindTreeResult::Other,
        };
        tree.push(FoundNode {
            node: child.clone(),
            x,
            y,
        });
        child.find_tree_at(x, y, tree)
    }

    fn remove_child(&self, _child: &dyn Node) {
        self.child.set(None);
        self.display_link.set(None);
        self.workspace_link.set(None);
    }

    fn child_size_changed(&self, _child: &dyn Node, width: i32, height: i32) {
        let pos = self.position.get();
        self.position
            .set(Rect::new_sized(pos.x1(), pos.x2(), width, height).unwrap());
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_floating(self, x, y)
    }

    fn into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        Some(self)
    }

    fn is_float(&self) -> bool {
        true
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        if let Some(child) = self.child.get() {
            child.change_extents(rect);
        }
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        if let Some(c) = self.child.get() {
            c.set_workspace(ws);
        }
        self.workspace_link.set(Some(ws.stacked.add_last(self.clone())));
        self.workspace.set(ws.clone());
    }
}
