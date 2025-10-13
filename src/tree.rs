use {
    crate::{
        backend::KeyState,
        client::{Client, ClientId},
        fixed::Fixed,
        ifs::{
            wl_seat::{
                Dnd, NodeSeatState, WlSeatGlobal,
                tablet::{
                    PadButtonState, TabletPad, TabletPadDial, TabletPadGroup, TabletPadRing,
                    TabletPadStrip, TabletRingEventSource, TabletStripEventSource, TabletTool,
                    TabletToolChanges, ToolButtonState,
                },
                wl_pointer::PendingScroll,
            },
            wl_surface::{WlSurface, tray::TrayItemId, zwlr_layer_surface_v1::ZwlrLayerSurfaceV1},
        },
        keyboard::KeyboardState,
        rect::Rect,
        renderer::Renderer,
        tree::transaction::TreeTransaction,
        utils::{linkedlist::NodeRef, numcell::NumCell},
    },
    jay_config::Direction as JayDirection,
    linearize::{Linearize, LinearizeExt},
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
pub mod transaction;
mod walker;
mod workspace;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Direction {
    Unspecified,
    Left,
    Down,
    Up,
    Right,
}

impl From<JayDirection> for Direction {
    fn from(d: JayDirection) -> Self {
        match d {
            JayDirection::Left => Self::Left,
            JayDirection::Down => Self::Down,
            JayDirection::Up => Self::Up,
            JayDirection::Right => Self::Right,
        }
    }
}

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
    #[expect(dead_code)]
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FindTreeUsecase {
    None,
    SelectToplevel,
    SelectWorkspace,
}

#[derive(Copy, Clone)]
pub enum NodeLocation {
    Workspace(OutputNodeId, WorkspaceNodeId),
    Output(OutputNodeId),
}

#[derive(Copy, Clone, Linearize, Eq, PartialEq, Debug)]
pub enum NodeLayer {
    Display,
    Layer0,
    Layer1,
    Output,
    Workspace,
    Tiled,
    Fullscreen,
    Stacked,
    Layer2,
    Layer3,
    StackedAboveLayers,
    Lock,
    InputMethod,
}

pub enum NodeLayerLink {
    Display,
    Layer0(NodeRef<Rc<ZwlrLayerSurfaceV1>>),
    Layer1(NodeRef<Rc<ZwlrLayerSurfaceV1>>),
    Output,
    Workspace,
    Tiled,
    Fullscreen,
    Stacked(NodeRef<Rc<dyn StackedNode>>),
    Layer2(NodeRef<Rc<ZwlrLayerSurfaceV1>>),
    Layer3(NodeRef<Rc<ZwlrLayerSurfaceV1>>),
    StackedAboveLayers(NodeRef<Rc<dyn StackedNode>>),
    Lock,
    InputMethod,
}

impl NodeLayerLink {
    pub fn layer(&self) -> NodeLayer {
        macro_rules! map {
            ($($id:ident,)*) => {
                match self {
                    $(
                        Self::$id { .. } => NodeLayer::$id,
                    )*
                }
            };
        }
        map! {
            Display,
            Layer0,
            Layer1,
            Output,
            Workspace,
            Tiled,
            Fullscreen,
            Stacked,
            Layer2,
            Layer3,
            StackedAboveLayers,
            Lock,
            InputMethod,
        }
    }
}

impl NodeLayer {
    pub fn prev(self) -> Self {
        if self == NodeLayer::Display {
            return NodeLayer::InputMethod;
        }
        Self::from_linear(self.linearize() - 1).unwrap_or(NodeLayer::InputMethod)
    }

    pub fn next(self) -> Self {
        Self::from_linear(self.linearize() + 1).unwrap_or(NodeLayer::Display)
    }
}

linear_ids!(TreeSerials, TreeSerial, u64);

pub trait Node: 'static {
    fn node_id(&self) -> NodeId;
    fn node_seat_state(&self) -> &NodeSeatState;
    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor);
    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor);
    fn node_visible(&self) -> bool;
    fn node_mapped_position(&self) -> Rect;
    fn node_output(&self) -> Option<Rc<OutputNode>>;
    fn node_location(&self) -> Option<NodeLocation>;
    fn node_layer(&self) -> NodeLayerLink;

    fn node_child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        let _ = child;
        let _ = title;
    }

    fn node_accepts_focus(&self) -> bool {
        true
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let _ = seat;
        let _ = direction;
    }

    fn node_active_changed(&self, active: bool) {
        let _ = active;
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        let _ = x;
        let _ = y;
        let _ = tree;
        let _ = usecase;
        FindTreeResult::Other
    }

    fn node_child_size_changed(self: Rc<Self>, child: &dyn Node, width: i32, height: i32) {
        let _ = (child, width, height);
    }

    fn node_child_active_changed(self: Rc<Self>, child: &dyn Node, active: bool, depth: u32) {
        let _ = (child, active, depth);
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, bounds: Option<&Rect>) {
        let _ = renderer;
        let _ = x;
        let _ = y;
        let _ = bounds;
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

    fn node_tray_item(&self) -> Option<TrayItemId> {
        None
    }

    fn node_make_visible(self: Rc<Self>, tt: &TreeTransaction) {
        let _ = tt;
    }

    // EVENT HANDLERS

    fn node_on_key(
        &self,
        seat: &WlSeatGlobal,
        time_usec: u64,
        key: u32,
        state: KeyState,
        kb_state: &KeyboardState,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = key;
        let _ = state;
        let _ = kb_state;
    }

    fn node_on_mods(&self, seat: &WlSeatGlobal, kb_state: &KeyboardState) {
        let _ = seat;
        let _ = kb_state;
    }

    fn node_on_touch_down(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = id;
        let _ = x;
        let _ = y;
    }

    fn node_on_touch_up(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32) {
        let _ = seat;
        let _ = time_usec;
        let _ = id;
    }

    fn node_on_touch_motion(
        self: Rc<Self>,
        seat: &WlSeatGlobal,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = id;
        let _ = x;
        let _ = y;
    }

    fn node_on_touch_frame(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn node_on_touch_cancel(&self, seat: &WlSeatGlobal) {
        let _ = seat;
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        serial: u64,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = button;
        let _ = state;
        let _ = serial;
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        let _ = seat;
        let _ = event;
    }

    fn node_on_focus(self: Rc<Self>, seat: &WlSeatGlobal) {
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

    fn node_on_dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u64) {
        let _ = dnd;
        let _ = x;
        let _ = y;
        let _ = serial;
    }

    fn node_on_dnd_motion(&self, dnd: &Dnd, time_usec: u64, x: Fixed, y: Fixed) {
        let _ = dnd;
        let _ = time_usec;
        let _ = x;
        let _ = y;
    }

    fn node_on_swipe_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let _ = seat;
        let _ = time_usec;
        let _ = finger_count;
    }

    fn node_on_swipe_update(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, dx: Fixed, dy: Fixed) {
        let _ = seat;
        let _ = time_usec;
        let _ = dx;
        let _ = dy;
    }

    fn node_on_swipe_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        let _ = seat;
        let _ = time_usec;
        let _ = cancelled;
    }

    fn node_on_pinch_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let _ = seat;
        let _ = time_usec;
        let _ = finger_count;
    }

    fn node_on_pinch_update(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = dx;
        let _ = dy;
        let _ = scale;
        let _ = rotation;
    }

    fn node_on_pinch_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        let _ = seat;
        let _ = time_usec;
        let _ = cancelled;
    }

    fn node_on_hold_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let _ = seat;
        let _ = time_usec;
        let _ = finger_count;
    }

    fn node_on_hold_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        let _ = seat;
        let _ = time_usec;
        let _ = cancelled;
    }

    fn node_on_tablet_pad_enter(&self, pad: &Rc<TabletPad>) {
        let _ = pad;
    }

    fn node_on_tablet_pad_leave(&self, pad: &Rc<TabletPad>) {
        let _ = pad;
    }

    fn node_on_tablet_pad_button(
        &self,
        pad: &Rc<TabletPad>,
        time_usec: u64,
        button: u32,
        state: PadButtonState,
    ) {
        let _ = pad;
        let _ = time_usec;
        let _ = button;
        let _ = state;
    }

    fn node_on_tablet_pad_mode_switch(
        &self,
        pad: &Rc<TabletPad>,
        group: &Rc<TabletPadGroup>,
        time_usec: u64,
        mode: u32,
    ) {
        let _ = pad;
        let _ = group;
        let _ = time_usec;
        let _ = mode;
    }

    fn node_on_tablet_pad_ring(
        &self,
        pad: &Rc<TabletPad>,
        ring: &Rc<TabletPadRing>,
        source: Option<TabletRingEventSource>,
        angle: Option<f64>,
        time_usec: u64,
    ) {
        let _ = pad;
        let _ = time_usec;
        let _ = ring;
        let _ = source;
        let _ = angle;
    }

    fn node_on_tablet_pad_strip(
        &self,
        pad: &Rc<TabletPad>,
        strip: &Rc<TabletPadStrip>,
        source: Option<TabletStripEventSource>,
        position: Option<f64>,
        time_usec: u64,
    ) {
        let _ = pad;
        let _ = time_usec;
        let _ = strip;
        let _ = source;
        let _ = position;
    }

    fn node_on_tablet_pad_dial(
        &self,
        pad: &Rc<TabletPad>,
        dial: &Rc<TabletPadDial>,
        value120: i32,
        time_usec: u64,
    ) {
        let _ = pad;
        let _ = time_usec;
        let _ = dial;
        let _ = value120;
    }

    fn node_on_tablet_tool_leave(&self, tool: &Rc<TabletTool>, time_usec: u64) {
        let _ = tool;
        let _ = time_usec;
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        x: Fixed,
        y: Fixed,
    ) {
        let _ = tool;
        let _ = time_usec;
        let _ = x;
        let _ = y;
    }

    fn node_on_tablet_tool_button(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        button: u32,
        state: ToolButtonState,
    ) {
        let _ = tool;
        let _ = time_usec;
        let _ = button;
        let _ = state;
    }

    fn node_on_tablet_tool_apply_changes(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
        x: Fixed,
        y: Fixed,
    ) {
        let _ = tool;
        let _ = time_usec;
        let _ = changes;
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

    fn node_into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>> {
        None
    }

    fn node_into_containing_node(self: Rc<Self>) -> Option<Rc<dyn ContainingNode>> {
        None
    }

    fn node_into_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        None
    }

    // TYPE CHECKERS

    fn node_is_container(&self) -> bool {
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
