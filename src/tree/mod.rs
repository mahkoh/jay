use crate::backend::{KeyState, Output, OutputId, ScrollAxis};
use crate::fixed::Fixed;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::rect::Rect;
use crate::render::Renderer;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::{LinkedList, LinkedNode};
use crate::NumCell;
pub use container::*;
use std::cell::{Cell, RefCell};
use std::fmt::Display;
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

pub trait Node {
    fn id(&self) -> NodeId;

    fn clear(&self) {
        // nothing
    }

    fn button(self: Rc<Self>, seat: &WlSeatGlobal, button: u32, state: KeyState) {
        let _ = seat;
        let _ = button;
        let _ = state;
    }

    fn scroll(&self, seat: &WlSeatGlobal, delta: i32, axis: ScrollAxis) {
        let _ = seat;
        let _ = delta;
        let _ = axis;
    }

    fn focus(self: Rc<Self>, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn unfocus(self: Rc<Self>, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn find_child_at(&self, x: i32, y: i32) -> Option<FoundNode> {
        let _ = x;
        let _ = y;
        None
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

    fn enter(self: Rc<Self>, seat: &WlSeatGlobal, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn motion(&self, seat: &WlSeatGlobal, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn render(&self, renderer: &mut dyn Renderer, x: i32, y: i32) {
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

    fn get_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn change_size(self: Rc<Self>, width: i32, height: i32) {
        let _ = width;
        let _ = height;
    }
}

pub struct FoundNode {
    pub node: Rc<dyn Node>,
    pub x: i32,
    pub y: i32,
    pub contained: bool,
}

tree_id!(ToplevelNodeId);

pub struct DisplayNode {
    pub id: NodeId,
    pub outputs: CopyHashMap<OutputId, Rc<OutputNode>>,
    pub floaters: LinkedList<Rc<FloatNode>>,
}

impl DisplayNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            outputs: Default::default(),
            floaters: Default::default(),
        }
    }
}

impl Node for DisplayNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn clear(&self) {
        let mut outputs = self.outputs.lock();
        for output in outputs.values() {
            output.clear();
        }
        outputs.clear();
        for floater in self.floaters.iter() {
            floater.clear();
        }
    }

    fn find_child_at(&self, x: i32, y: i32) -> Option<FoundNode> {
        let outputs = self.outputs.lock();
        for output in outputs.values() {
            let pos = output.position.get();
            if pos.contains(x, y) {
                let (x, y) = pos.translate(x, y);
                return Some(FoundNode {
                    node: output.clone(),
                    x,
                    y,
                    contained: true,
                });
            }
        }
        None
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
}

impl Node for OutputNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn clear(&self) {
        let mut workspaces = self.workspaces.borrow_mut();
        for workspace in workspaces.drain(..) {
            workspace.clear();
        }
    }

    fn find_child_at(&self, x: i32, y: i32) -> Option<FoundNode> {
        if let Some(ws) = self.workspace.get() {
            Some(FoundNode {
                node: ws,
                x,
                y,
                contained: true,
            })
        } else {
            None
        }
    }

    fn render(&self, renderer: &mut dyn Renderer, _x: i32, _y: i32) {
        renderer.render_output(self);
    }

    fn remove_child(&self, _child: &dyn Node) {
        self.workspace.set(None);
    }

    fn change_size(self: Rc<Self>, width: i32, height: i32) {
        self.position
            .set(Rect::new_sized(0, 0, width, height).unwrap());
        if let Some(c) = self.workspace.get() {
            c.change_size(width, height);
        }
    }
}

tree_id!(FloatNodeId);
pub struct FloatNode {
    pub id: FloatNodeId,
    pub visible: Cell<bool>,
    pub position: Cell<Rect>,
    pub display: Rc<DisplayNode>,
    pub display_link: Cell<Option<LinkedNode<Rc<FloatNode>>>>,
    pub workspace_link: Cell<Option<LinkedNode<Rc<FloatNode>>>>,
    pub workspace: CloneCell<Rc<WorkspaceNode>>,
    pub child: CloneCell<Option<Rc<dyn Node>>>,
}

impl Node for FloatNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn clear(&self) {
        self.child.set(None);
    }

    fn find_child_at(&self, x: i32, y: i32) -> Option<FoundNode> {
        self.child.get().and_then(|c| c.find_child_at(x, y))
    }

    fn remove_child(&self, _child: &dyn Node) {
        self.child.set(None);
        self.display_link.set(None);
        self.workspace_link.set(None);
    }

    fn render(&self, renderer: &mut dyn Renderer, x: i32, y: i32) {
        renderer.render_floating(self, x, y)
    }

    fn into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        Some(self)
    }

    fn is_float(&self) -> bool {
        true
    }

    fn get_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        Some(self.workspace.get())
    }

    fn child_size_changed(&self, _child: &dyn Node, width: i32, height: i32) {
        let pos = self.position.get();
        self.position
            .set(Rect::new_sized(pos.x1(), pos.x2(), width, height).unwrap());
    }
}
