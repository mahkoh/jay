use crate::backend::{ConnectorId, KeyState, ScrollAxis};
use crate::client::{Client, ClientId};
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{Dnd, NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::xwindow::Xwindow;
use crate::ifs::wl_surface::WlSurface;
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::walker::NodeVisitor;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::LinkedList;
use crate::utils::numcell::NumCell;
use crate::xkbcommon::ModifierState;
pub use container::*;
pub use float::*;
use jay_config::Direction;
pub use output::*;
use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::rc::Rc;
pub use workspace::*;

mod container;
mod float;
mod output;
pub mod toplevel;
pub mod walker;
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

pub trait Node {
    fn id(&self) -> NodeId;
    fn seat_state(&self) -> &NodeSeatState;
    fn destroy_node(&self, detach: bool);
    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor);
    fn visit_children(&self, visitor: &mut dyn NodeVisitor);

    fn get_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn is_contained_in(&self, other: NodeId) -> bool {
        let _ = other;
        false
    }

    fn child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        let _ = child;
        let _ = title;
    }

    fn get_parent_mono(&self) -> Option<bool> {
        None
    }

    fn get_parent_split(&self) -> Option<ContainerSplit> {
        None
    }

    fn set_parent_mono(&self, mono: bool) {
        let _ = mono;
    }

    fn set_parent_split(&self, split: ContainerSplit) {
        let _ = split;
    }

    fn get_mono(&self) -> Option<bool> {
        None
    }

    fn get_split(&self) -> Option<ContainerSplit> {
        None
    }

    fn set_mono(self: Rc<Self>, child: Option<&dyn Node>) {
        let _ = child;
    }

    fn set_split(self: Rc<Self>, split: ContainerSplit) {
        let _ = split;
    }

    fn create_split(self: Rc<Self>, split: ContainerSplit) {
        let _ = split;
    }

    fn focus_self(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn move_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn move_self(self: Rc<Self>, direction: Direction) {
        let _ = direction;
    }

    fn move_focus_from_child(
        &self,
        seat: &Rc<WlSeatGlobal>,
        child: &dyn Node,
        direction: Direction,
    ) {
        let _ = seat;
        let _ = direction;
        let _ = child;
    }

    fn move_child(self: Rc<Self>, child: Rc<dyn Node>, direction: Direction) {
        let _ = direction;
        let _ = child;
    }

    fn absolute_position(&self) -> Rect {
        Rect::new_empty(0, 0)
    }

    fn absolute_position_constrains_input(&self) -> bool {
        true
    }

    fn active_changed(&self, active: bool) {
        let _ = active;
    }

    fn key(&self, seat: &WlSeatGlobal, key: u32, state: u32) {
        let _ = seat;
        let _ = key;
        let _ = state;
    }

    fn mods(&self, seat: &WlSeatGlobal, mods: ModifierState) {
        let _ = seat;
        let _ = mods;
    }

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

    fn focus_parent(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn toggle_floating(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
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

    fn replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn Node>) {
        let _ = old;
        let _ = new;
    }

    fn remove_child(self: Rc<Self>, child: &dyn Node) {
        let _ = child;
    }

    fn child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        let _ = (child, width, height);
    }

    fn child_active_changed(self: Rc<Self>, child: &dyn Node, active: bool) {
        let _ = (child, active);
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
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

    fn is_container(&self) -> bool {
        false
    }

    fn is_output(&self) -> bool {
        false
    }

    fn into_output(self: Rc<Self>) -> Option<Rc<OutputNode>> {
        None
    }

    fn accepts_child(&self, node: &dyn Node) -> bool {
        let _ = node;
        false
    }

    fn insert_child(self: Rc<Self>, node: Rc<dyn Node>, direction: Direction) {
        let _ = node;
        let _ = direction;
    }

    fn is_float(&self) -> bool {
        false
    }

    fn is_workspace(&self) -> bool {
        false
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        let _ = rect;
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        let _ = ws;
    }

    fn set_parent(self: Rc<Self>, parent: Rc<dyn Node>) {
        let _ = parent;
    }

    fn client(&self) -> Option<Rc<Client>> {
        None
    }

    fn client_id(&self) -> Option<ClientId> {
        self.client().map(|c| c.id)
    }

    fn into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>> {
        None
    }

    fn dnd_drop(&self, dnd: &Dnd) {
        let _ = dnd;
    }

    fn dnd_leave(&self, dnd: &Dnd) {
        let _ = dnd;
    }

    fn dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed) {
        let _ = dnd;
        let _ = x;
        let _ = y;
    }

    fn dnd_motion(&self, dnd: &Dnd, x: Fixed, y: Fixed) {
        let _ = dnd;
        let _ = x;
        let _ = y;
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
    pub outputs: CopyHashMap<ConnectorId, Rc<OutputNode>>,
    pub stacked: LinkedList<Rc<dyn Node>>,
    pub xstacked: LinkedList<Rc<Xwindow>>,
    pub seat_state: NodeSeatState,
}

impl DisplayNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            outputs: Default::default(),
            stacked: Default::default(),
            xstacked: Default::default(),
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

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_display(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let outputs = self.outputs.lock();
        for (_, output) in outputs.deref() {
            visitor.visit_output(output);
        }
        for stacked in self.stacked.iter() {
            stacked.deref().clone().visit(visitor);
        }
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        for stacked in self.stacked.rev_iter() {
            let ext = stacked.absolute_position();
            if stacked.absolute_position_constrains_input() && !ext.contains(x, y) {
                // TODO: make constrain always true
                continue;
            }
            let (x, y) = ext.translate(x, y);
            let idx = tree.len();
            tree.push(FoundNode {
                node: stacked.deref().clone(),
                x,
                y,
            });
            match stacked.find_tree_at(x, y, tree) {
                FindTreeResult::AcceptsInput => {
                    return FindTreeResult::AcceptsInput;
                }
                FindTreeResult::Other => {
                    tree.drain(idx..);
                }
            }
        }
        let outputs = self.outputs.lock();
        for output in outputs.values() {
            let pos = output.global.pos.get();
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

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }
}
