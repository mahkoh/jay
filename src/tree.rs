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

pub trait SizedNode: Sized + 'static {
    fn id(&self) -> NodeId;
    fn seat_state(&self) -> &NodeSeatState;
    fn destroy_node(&self, detach: bool);
    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor);
    fn visit_children(&self, visitor: &mut dyn NodeVisitor);
    fn visible(&self) -> bool;

    fn last_active_child(self: &Rc<Self>) -> Rc<dyn Node> {
        self.clone()
    }

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

    fn child_title_changed(self: &Rc<Self>, child: &dyn Node, title: &str) {
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

    fn set_mono(self: &Rc<Self>, child: Option<&dyn Node>) {
        let _ = child;
    }

    fn set_split(self: &Rc<Self>, split: ContainerSplit) {
        let _ = split;
    }

    fn create_split(self: &Rc<Self>, split: ContainerSplit) {
        let _ = split;
    }

    fn focus_self(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn close(&self) {
        // nothing
    }

    fn move_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn move_self(self: &Rc<Self>, direction: Direction) {
        let _ = direction;
    }

    fn move_focus_from_child(
        self: &Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        child: &dyn Node,
        direction: Direction,
    ) {
        let _ = seat;
        let _ = direction;
        let _ = child;
    }

    fn move_child(self: &Rc<Self>, child: Rc<dyn Node>, direction: Direction) {
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

    fn button(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState, serial: u32) {
        let _ = seat;
        let _ = button;
        let _ = state;
        let _ = serial;
    }

    fn axis_event(self: &Rc<Self>, seat: &WlSeatGlobal, event: &PendingScroll) {
        let _ = seat;
        let _ = event;
    }

    fn focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn focus_parent(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn toggle_floating(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>) {
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

    fn replace_child(self: &Rc<Self>, old: &dyn Node, new: Rc<dyn Node>) {
        let _ = old;
        let _ = new;
    }

    fn remove_child(self: &Rc<Self>, child: &dyn Node) {
        let _ = child;
    }

    fn child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        let _ = (child, width, height);
    }

    fn child_active_changed(self: &Rc<Self>, child: &dyn Node, active: bool, depth: u32) {
        let _ = (child, active, depth);
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn pointer_enter(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
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

    fn pointer_motion(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        let _ = seat;
        let _ = x;
        let _ = y;
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        let _ = renderer;
        let _ = x;
        let _ = y;
    }

    fn into_float(self: &Rc<Self>) -> Option<Rc<FloatNode>> {
        None
    }

    fn into_container(self: &Rc<Self>) -> Option<Rc<ContainerNode>> {
        None
    }

    fn is_container(&self) -> bool {
        false
    }

    fn is_output(&self) -> bool {
        false
    }

    fn into_output(self: &Rc<Self>) -> Option<Rc<OutputNode>> {
        None
    }

    fn accepts_child(&self, node: &dyn Node) -> bool {
        let _ = node;
        false
    }

    fn insert_child(self: &Rc<Self>, node: Rc<dyn Node>, direction: Direction) {
        let _ = node;
        let _ = direction;
    }

    fn is_float(&self) -> bool {
        false
    }

    fn is_workspace(&self) -> bool {
        false
    }

    fn change_extents(self: &Rc<Self>, rect: &Rect) {
        let _ = rect;
    }

    fn set_workspace(self: &Rc<Self>, ws: &Rc<WorkspaceNode>) {
        let _ = ws;
    }

    fn set_parent(self: &Rc<Self>, parent: Rc<dyn Node>) {
        let _ = parent;
    }

    fn client(&self) -> Option<Rc<Client>> {
        None
    }

    fn client_id(&self) -> Option<ClientId> {
        self.client().map(|c| c.id)
    }

    fn into_surface(self: &Rc<Self>) -> Option<Rc<WlSurface>> {
        None
    }

    fn dnd_drop(&self, dnd: &Dnd) {
        let _ = dnd;
    }

    fn dnd_leave(&self, dnd: &Dnd) {
        let _ = dnd;
    }

    fn dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u32) {
        let _ = dnd;
        let _ = x;
        let _ = y;
        let _ = serial;
    }

    fn dnd_motion(&self, dnd: &Dnd, x: Fixed, y: Fixed) {
        let _ = dnd;
        let _ = x;
        let _ = y;
    }
}

pub trait Node {
    fn node_id(&self) -> NodeId;
    fn node_seat_state(&self) -> &NodeSeatState;
    fn node_destroy(&self, detach: bool);
    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor);
    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor);
    fn node_visible(&self) -> bool;
    fn node_last_active_child(self: Rc<Self>) -> Rc<dyn Node>;
    fn node_set_visible(&self, visible: bool);
    fn node_get_workspace(&self) -> Option<Rc<WorkspaceNode>>;
    fn node_is_contained_in(&self, other: NodeId) -> bool;
    fn node_child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str);
    fn node_get_parent_mono(&self) -> Option<bool>;
    fn node_get_parent_split(&self) -> Option<ContainerSplit>;
    fn node_set_parent_mono(&self, mono: bool);
    fn node_set_parent_split(&self, split: ContainerSplit);
    fn node_get_mono(&self) -> Option<bool>;
    fn node_get_split(&self) -> Option<ContainerSplit>;
    fn node_set_mono(self: Rc<Self>, child: Option<&dyn Node>);
    fn node_set_split(self: Rc<Self>, split: ContainerSplit);
    fn node_create_split(self: Rc<Self>, split: ContainerSplit);
    fn node_focus_self(self: Rc<Self>, seat: &Rc<WlSeatGlobal>);
    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction);
    fn node_close(&self);
    fn node_move_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction);
    fn node_move_self(self: Rc<Self>, direction: Direction);
    fn node_move_focus_from_child(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        child: &dyn Node,
        direction: Direction,
    );
    fn node_move_child(self: Rc<Self>, child: Rc<dyn Node>, direction: Direction);
    fn node_absolute_position(&self) -> Rect;
    fn node_absolute_position_constrains_input(&self) -> bool;
    fn node_active_changed(&self, active: bool);
    fn node_key(&self, seat: &WlSeatGlobal, key: u32, state: u32);
    fn node_mods(&self, seat: &WlSeatGlobal, mods: ModifierState);
    fn node_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        state: KeyState,
        serial: u32,
    );
    fn node_axis_event(self: Rc<Self>, seat: &WlSeatGlobal, event: &PendingScroll);
    fn node_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>);
    fn node_focus_parent(&self, seat: &Rc<WlSeatGlobal>);
    fn node_toggle_floating(self: Rc<Self>, seat: &Rc<WlSeatGlobal>);
    fn node_unfocus(&self, seat: &WlSeatGlobal);
    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult;
    fn node_replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn Node>);
    fn node_remove_child(self: Rc<Self>, child: &dyn Node);
    fn node_child_size_changed(&self, child: &dyn Node, width: i32, height: i32);
    fn node_child_active_changed(self: Rc<Self>, child: &dyn Node, active: bool, depth: u32);
    fn node_leave(&self, seat: &WlSeatGlobal);
    fn node_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed);
    fn node_pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>);
    fn node_pointer_focus(&self, seat: &Rc<WlSeatGlobal>);
    fn node_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed);
    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32);
    fn node_into_float(self: Rc<Self>) -> Option<Rc<FloatNode>>;
    fn node_into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>>;
    fn node_is_container(&self) -> bool;
    fn node_is_output(&self) -> bool;
    fn node_into_output(self: Rc<Self>) -> Option<Rc<OutputNode>>;
    fn node_accepts_child(&self, node: &dyn Node) -> bool;
    fn node_insert_child(self: Rc<Self>, node: Rc<dyn Node>, direction: Direction);
    fn node_is_float(&self) -> bool;
    fn node_is_workspace(&self) -> bool;
    fn node_change_extents(self: Rc<Self>, rect: &Rect);
    fn node_set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>);
    fn node_set_parent(self: Rc<Self>, parent: Rc<dyn Node>);
    fn node_client(&self) -> Option<Rc<Client>>;
    fn node_client_id(&self) -> Option<ClientId>;
    fn node_into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>>;
    fn node_dnd_drop(&self, dnd: &Dnd);
    fn node_dnd_leave(&self, dnd: &Dnd);
    fn node_dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u32);
    fn node_dnd_motion(&self, dnd: &Dnd, x: Fixed, y: Fixed);
}

impl<T: SizedNode> Node for T {
    fn node_id(&self) -> NodeId {
        <Self as SizedNode>::id(self)
    }
    fn node_seat_state(&self) -> &NodeSeatState {
        <Self as SizedNode>::seat_state(self)
    }
    fn node_destroy(&self, detach: bool) {
        <Self as SizedNode>::destroy_node(self, detach)
    }
    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        <Self as SizedNode>::visit(&self, visitor)
    }
    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        <Self as SizedNode>::visit_children(self, visitor)
    }
    fn node_visible(&self) -> bool {
        <Self as SizedNode>::visible(self)
    }
    fn node_last_active_child(self: Rc<Self>) -> Rc<dyn Node> {
        <Self as SizedNode>::last_active_child(&self)
    }
    fn node_set_visible(&self, visible: bool) {
        <Self as SizedNode>::set_visible(self, visible)
    }
    fn node_get_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        <Self as SizedNode>::get_workspace(self)
    }
    fn node_is_contained_in(&self, other: NodeId) -> bool {
        <Self as SizedNode>::is_contained_in(self, other)
    }
    fn node_child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        <Self as SizedNode>::child_title_changed(&self, child, title)
    }
    fn node_get_parent_mono(&self) -> Option<bool> {
        <Self as SizedNode>::get_parent_mono(self)
    }
    fn node_get_parent_split(&self) -> Option<ContainerSplit> {
        <Self as SizedNode>::get_parent_split(self)
    }
    fn node_set_parent_mono(&self, mono: bool) {
        <Self as SizedNode>::set_parent_mono(self, mono)
    }
    fn node_set_parent_split(&self, split: ContainerSplit) {
        <Self as SizedNode>::set_parent_split(self, split)
    }
    fn node_get_mono(&self) -> Option<bool> {
        <Self as SizedNode>::get_mono(self)
    }
    fn node_get_split(&self) -> Option<ContainerSplit> {
        <Self as SizedNode>::get_split(self)
    }
    fn node_set_mono(self: Rc<Self>, child: Option<&dyn Node>) {
        <Self as SizedNode>::set_mono(&self, child)
    }
    fn node_set_split(self: Rc<Self>, split: ContainerSplit) {
        <Self as SizedNode>::set_split(&self, split)
    }
    fn node_create_split(self: Rc<Self>, split: ContainerSplit) {
        <Self as SizedNode>::create_split(&self, split)
    }
    fn node_focus_self(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        <Self as SizedNode>::focus_self(&self, seat)
    }
    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        <Self as SizedNode>::do_focus(&self, seat, direction)
    }
    fn node_close(&self) {
        <Self as SizedNode>::close(self)
    }
    fn node_move_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        <Self as SizedNode>::move_focus(&self, seat, direction)
    }
    fn node_move_self(self: Rc<Self>, direction: Direction) {
        <Self as SizedNode>::move_self(&self, direction)
    }
    fn node_move_focus_from_child(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        child: &dyn Node,
        direction: Direction,
    ) {
        <Self as SizedNode>::move_focus_from_child(&self, seat, child, direction)
    }
    fn node_move_child(self: Rc<Self>, child: Rc<dyn Node>, direction: Direction) {
        <Self as SizedNode>::move_child(&self, child, direction)
    }
    fn node_absolute_position(&self) -> Rect {
        <Self as SizedNode>::absolute_position(self)
    }
    fn node_absolute_position_constrains_input(&self) -> bool {
        <Self as SizedNode>::absolute_position_constrains_input(self)
    }
    fn node_active_changed(&self, active: bool) {
        <Self as SizedNode>::active_changed(self, active)
    }
    fn node_key(&self, seat: &WlSeatGlobal, key: u32, state: u32) {
        <Self as SizedNode>::key(self, seat, key, state)
    }
    fn node_mods(&self, seat: &WlSeatGlobal, mods: ModifierState) {
        <Self as SizedNode>::mods(self, seat, mods)
    }
    fn node_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        state: KeyState,
        serial: u32,
    ) {
        <Self as SizedNode>::button(&self, seat, button, state, serial)
    }
    fn node_axis_event(self: Rc<Self>, seat: &WlSeatGlobal, event: &PendingScroll) {
        <Self as SizedNode>::axis_event(&self, seat, event)
    }
    fn node_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        <Self as SizedNode>::focus(&self, seat)
    }
    fn node_focus_parent(&self, seat: &Rc<WlSeatGlobal>) {
        <Self as SizedNode>::focus_parent(self, seat)
    }
    fn node_toggle_floating(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        <Self as SizedNode>::toggle_floating(&self, seat)
    }
    fn node_unfocus(&self, seat: &WlSeatGlobal) {
        <Self as SizedNode>::unfocus(self, seat)
    }
    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        <Self as SizedNode>::find_tree_at(self, x, y, tree)
    }
    fn node_replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn Node>) {
        <Self as SizedNode>::replace_child(&self, old, new)
    }
    fn node_remove_child(self: Rc<Self>, child: &dyn Node) {
        <Self as SizedNode>::remove_child(&self, child)
    }
    fn node_child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        <Self as SizedNode>::child_size_changed(self, child, width, height)
    }
    fn node_child_active_changed(self: Rc<Self>, child: &dyn Node, active: bool, depth: u32) {
        <Self as SizedNode>::child_active_changed(&self, child, active, depth)
    }
    fn node_leave(&self, seat: &WlSeatGlobal) {
        <Self as SizedNode>::leave(self, seat)
    }
    fn node_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        <Self as SizedNode>::pointer_enter(&self, seat, x, y)
    }
    fn node_pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>) {
        <Self as SizedNode>::pointer_unfocus(self, seat)
    }
    fn node_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        <Self as SizedNode>::pointer_focus(self, seat)
    }
    fn node_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        <Self as SizedNode>::pointer_motion(&self, seat, x, y)
    }
    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        <Self as SizedNode>::render(self, renderer, x, y)
    }
    fn node_into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        <Self as SizedNode>::into_float(&self)
    }
    fn node_into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>> {
        <Self as SizedNode>::into_container(&self)
    }
    fn node_is_container(&self) -> bool {
        <Self as SizedNode>::is_container(self)
    }
    fn node_is_output(&self) -> bool {
        <Self as SizedNode>::is_output(self)
    }
    fn node_into_output(self: Rc<Self>) -> Option<Rc<OutputNode>> {
        <Self as SizedNode>::into_output(&self)
    }
    fn node_accepts_child(&self, node: &dyn Node) -> bool {
        <Self as SizedNode>::accepts_child(self, node)
    }
    fn node_insert_child(self: Rc<Self>, node: Rc<dyn Node>, direction: Direction) {
        <Self as SizedNode>::insert_child(&self, node, direction)
    }
    fn node_is_float(&self) -> bool {
        <Self as SizedNode>::is_float(self)
    }
    fn node_is_workspace(&self) -> bool {
        <Self as SizedNode>::is_workspace(self)
    }
    fn node_change_extents(self: Rc<Self>, rect: &Rect) {
        <Self as SizedNode>::change_extents(&self, rect)
    }
    fn node_set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        <Self as SizedNode>::set_workspace(&self, ws)
    }
    fn node_set_parent(self: Rc<Self>, parent: Rc<dyn Node>) {
        <Self as SizedNode>::set_parent(&self, parent)
    }
    fn node_client(&self) -> Option<Rc<Client>> {
        <Self as SizedNode>::client(self)
    }
    fn node_client_id(&self) -> Option<ClientId> {
        <Self as SizedNode>::client_id(self)
    }
    fn node_into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>> {
        <Self as SizedNode>::into_surface(&self)
    }
    fn node_dnd_drop(&self, dnd: &Dnd) {
        <Self as SizedNode>::dnd_drop(self, dnd)
    }
    fn node_dnd_leave(&self, dnd: &Dnd) {
        <Self as SizedNode>::dnd_leave(self, dnd)
    }
    fn node_dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u32) {
        <Self as SizedNode>::dnd_enter(self, dnd, x, y, serial)
    }
    fn node_dnd_motion(&self, dnd: &Dnd, x: Fixed, y: Fixed) {
        <Self as SizedNode>::dnd_motion(self, dnd, x, y)
    }
}

pub struct FoundNode {
    pub node: Rc<dyn Node>,
    pub x: i32,
    pub y: i32,
}
