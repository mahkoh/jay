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
pub use {
    container::*, containing::*, display::*, float::*, output::*, placeholder::*, stacked::*,
    toplevel::*, walker::*, workspace::*,
};

mod container;
mod containing;
mod display;
mod float;
mod output;
mod placeholder;
mod stacked;
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

impl FindTreeResult {
    pub fn accepts_input(self) -> bool {
        self == Self::AcceptsInput
    }
}

pub trait Node: 'static {
    fn node_id(&self) -> NodeId;
    fn node_seat_state(&self) -> &NodeSeatState;
    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor);
    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor);
    fn node_visible(&self) -> bool;
    fn node_absolute_position(&self) -> Rect;

    fn node_child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        let _ = child;
        let _ = title;
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn node_active_changed(&self, active: bool) {
        let _ = active;
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let _ = x;
        let _ = y;
        let _ = tree;
        FindTreeResult::Other
    }

    fn node_child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        let _ = (child, width, height);
    }

    fn node_child_active_changed(self: Rc<Self>, child: &dyn Node, active: bool, depth: u32) {
        let _ = (child, active, depth);
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        let _ = renderer;
        let _ = x;
        let _ = y;
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        None
    }

    fn node_client_id(&self) -> Option<ClientId> {
        self.node_client().map(|c| c.id)
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        None
    }

    // EVENT HANDLERS

    fn node_on_key(&self, seat: &WlSeatGlobal, key: u32, state: u32) {
        let _ = seat;
        let _ = key;
        let _ = state;
    }

    fn node_on_mods(&self, seat: &WlSeatGlobal, mods: ModifierState) {
        let _ = seat;
        let _ = mods;
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        state: KeyState,
        serial: u32,
    ) {
        let _ = seat;
        let _ = button;
        let _ = state;
        let _ = serial;
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        let _ = seat;
        let _ = event;
    }

    fn node_on_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn node_on_unfocus(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn node_on_leave(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn node_on_pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("{} focus", std::any::type_name::<Self>());
        let _ = seat;
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn node_on_pointer_relative_motion(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = dx;
        let _ = dy;
        let _ = dx_unaccelerated;
        let _ = dy_unaccelerated;
    }

    fn node_on_dnd_drop(&self, dnd: &Dnd) {
        let _ = dnd;
    }

    fn node_on_dnd_leave(&self, dnd: &Dnd) {
        let _ = dnd;
    }

    fn node_on_dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u32) {
        let _ = dnd;
        let _ = x;
        let _ = y;
        let _ = serial;
    }

    fn node_on_dnd_motion(&self, dnd: &Dnd, x: Fixed, y: Fixed) {
        let _ = dnd;
        let _ = x;
        let _ = y;
    }

    // TYPE CONVERTERS

    fn node_into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        None
    }

    fn node_into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>> {
        None
    }

    fn node_into_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn node_into_output(self: Rc<Self>) -> Option<Rc<OutputNode>> {
        None
    }

    fn node_into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>> {
        None
    }

    fn node_into_containing_node(self: Rc<Self>) -> Option<Rc<dyn ContainingNode>> {
        None
    }

    // TYPE CHECKERS

    fn node_is_container(&self) -> bool {
        false
    }

    fn node_is_output(&self) -> bool {
        false
    }

    fn node_is_float(&self) -> bool {
        false
    }

    fn node_is_workspace(&self) -> bool {
        false
    }

    fn node_is_xwayland_surface(&self) -> bool {
        false
    }

    fn node_is_placeholder(&self) -> bool {
        false
    }
}

pub struct FoundNode {
    pub node: Rc<dyn Node>,
    pub x: i32,
    pub y: i32,
}
