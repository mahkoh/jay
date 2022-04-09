use {
    crate::{
        backend::KeyState,
        client::{Client, ClientId},
        fixed::Fixed,
        ifs::{
            wl_seat::{wl_pointer::PendingScroll, Dnd, NodeSeatState, WlSeatGlobal},
            wl_surface::WlSurface,
        },
        rect::Rect,
        render::Renderer,
        utils::numcell::NumCell,
        xkbcommon::ModifierState,
    },
    jay_config::Direction,
    std::{
        fmt::{Debug, Display},
        rc::Rc,
    },
};
pub use {container::*, display::*, float::*, output::*, toplevel::*, walker::*, workspace::*};

mod container;
mod display;
mod float;
mod output;
mod toplevel;
mod walker;
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

#[derive(Copy, Clone, Eq, PartialEq)]
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
    fn visible(&self) -> bool;

    fn set_visible(&self, visible: bool) {
        let _ = visible;
    }

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

    fn close(&self) {
        // nothing
    }

    fn move_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn move_self(self: Rc<Self>, direction: Direction) {
        let _ = direction;
    }

    fn move_focus_from_child(
        self: Rc<Self>,
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

    fn axis_event(&self, seat: &WlSeatGlobal, event: &PendingScroll) {
        let _ = seat;
        let _ = event;
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
