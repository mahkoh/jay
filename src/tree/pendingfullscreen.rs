use crate::backend::ButtonState;
use crate::client::Client;
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::BTN_LEFT;
use crate::ifs::wl_seat::BTN_RIGHT;
use crate::ifs::wl_seat::NodeSeatState;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_factory_v1::JayIconSurfaceFactoryV1;
use crate::rect::Rect;
use crate::renderer::Renderer;
use crate::scale::Scale;
use crate::state::OutputEventListener;
use crate::state::State;
use crate::transactions::TransactionData;
use crate::transactions::Transactionable;
use crate::transactions::TransactionableExt;
use crate::tree::ContainerSplit;
use crate::tree::ContainingNode;
use crate::tree::Direction;
use crate::tree::FindTreeResult;
use crate::tree::FindTreeUsecase;
use crate::tree::FoundNode;
use crate::tree::Node;
use crate::tree::NodeBase;
use crate::tree::NodeId;
use crate::tree::NodeLayerLink;
use crate::tree::NodeLocation;
use crate::tree::NodeVisitor;
use crate::tree::OutputNode;
use crate::tree::PlaceholderNode;
use crate::tree::TileDragDestination;
use crate::tree::ToplevelData;
use crate::tree::ToplevelDataTransactionOp;
use crate::tree::ToplevelNode;
use crate::tree::ToplevelNodeBase;
use crate::tree::ToplevelOpt;
use crate::tree::ToplevelType;
use crate::tree::TreeTimeline;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::WorkspaceEventListener;
use crate::tree::WorkspaceNode;
use crate::tree::default_tile_drag_destination;
use crate::utils::clonecell::CloneCell;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use jay_proc::GetLiveness;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;
use std::rc::Weak;

mod pendingfullscreen_dfs_g_fuse;

tree_id!(PendingFullscreenNodeId);

/// The tile a fullscreen window gets while it is not the active fullscreen
/// window of its workspace. Its window stays marked visible so focus reaches it.
#[derive(GetLiveness)]
pub struct PendingFullscreenNode {
    id: PendingFullscreenNodeId,
    #[liveness]
    toplevel: ToplevelData,
    toplevel_opt: ToplevelOpt,
    child: CloneCell<Option<Rc<PlaceholderNode>>>,
    destroyed: Cell<bool>,
    location: Cell<Option<NodeLocation>>,
    transaction_data: TransactionData<PendingFullscreenTransactionOp>,
}

impl PendingFullscreenNode {
    pub fn new_for(
        state: &Rc<State>,
        placeholder: &Rc<PlaceholderNode>,
        node: &Rc<dyn ToplevelNode>,
        slf: &Weak<Self>,
    ) -> Self {
        let id = state.node_ids.next();
        placeholder.set_in_pending_wrapper(true);
        placeholder.set_hover(false);
        Self {
            id,
            // The fullscreen marker goes into the title bar, like the float pin.
            toplevel: ToplevelData::new(
                state,
                placeholder.tl_data().title.borrow().clone(),
                node.node_client(),
                ToplevelType::PendingFullscreen(Some(node.tl_data().identifier.get())),
                id,
                slf,
            ),
            toplevel_opt: ToplevelOpt::new(node),
            child: CloneCell::new(Some(placeholder.clone())),
            destroyed: Default::default(),
            location: Cell::new(node.node_location()),
            transaction_data: TransactionData::new(&state.tree),
        }
    }

    pub fn child(&self) -> Option<Rc<PlaceholderNode>> {
        self.child.get()
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn replace_child(self: &Rc<Self>, child: &Rc<PlaceholderNode>) {
        child.set_in_pending_wrapper(true);
        child.set_hover(false);
        self.child.set(Some(child.clone()));
        child.tl_set_parent(self.clone());
    }

    pub fn clear_child(self: &Rc<Self>) {
        self.child.set(None);
    }
}

impl NodeBase for PendingFullscreenNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.toplevel.seat_state
    }

    fn node_visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_pending_fullscreen(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.child.get() {
            c.clone().node_visit_dyn(visitor);
        }
        // Walking the workspace's pending list as well would report each seat twice.
        if let Some(tl) = self.toplevel_opt.get() {
            tl.node_visit_dyn(visitor);
        }
    }

    fn node_visible(&self, tl: TreeTimeline) -> bool {
        self.toplevel.visible[tl].get()
    }

    fn node_absolute_position(&self, _tl: TreeTimeline) -> Rect {
        self.toplevel.desired_extents.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.toplevel.output_opt(LiveTL)
    }

    fn node_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.toplevel.workspace[LiveTL].get()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.location.get()
    }

    fn node_layer(&self) -> NodeLayerLink {
        self.toplevel.node_layer()
    }

    fn node_debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.debugfs()
    }

    fn node_accepts_focus(&self) -> bool {
        false
    }

    fn node_do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        if let Some(tl) = self.toplevel_opt.get() {
            seat.focus_toplevel(tl);
        }
    }

    fn node_active_changed(&self, active: bool) {
        self.toplevel.update_self_active(self, active);
    }

    fn node_find_tree_at(
        &self,
        _x: i32,
        _y: i32,
        _tree: &mut Vec<FoundNode>,
        _usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        FindTreeResult::AcceptsInput
    }

    fn node_render(&self, renderer: &mut Renderer<'_>, x: i32, y: i32, bounds: Option<&Rect>) {
        let Some(c) = self.child.get() else {
            return;
        };
        renderer.render_fullscreen_tile(&c, x, y, bounds, self.toplevel_opt.get().as_deref());
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        self.toplevel.client.clone()
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }

    fn node_make_visible(self: &Rc<Self>) {
        self.toplevel.make_visible(&**self);
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        _time_usec: u64,
        button: u32,
        state: ButtonState,
        _serial: u64,
    ) {
        if state != ButtonState::Pressed {
            return;
        }
        let Some(tl) = self.toplevel_opt.get() else {
            return;
        };
        match button {
            BTN_LEFT => seat.focus_toplevel(tl),
            BTN_RIGHT => tl.tl_set_fullscreen(false, None),
            _ => {}
        }
    }

    fn node_on_leave(&self, _seat: &WlSeatGlobal) {
        if let Some(c) = self.child.get() {
            c.set_hover(false);
        }
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        if let Some(c) = self.child.get() {
            c.set_hover(true);
        }
        seat.pointer_cursor().set_known(KnownCursor::Default);
        seat.enter_toplevel(self.clone());
    }

    fn node_into_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }
    fn node_is_pending_fullscreen(&self) -> bool {
        true
    }

    fn node_contains_pending_fullscreen(&self) -> bool {
        true
    }

    fn node_pending_fullscreen_moved_to_ws(&self, ws: &Rc<WorkspaceNode>) {
        if let Some(tl) = self.toplevel_opt.get() {
            tl.tl_data().retarget_pending_fullscreen_workspace(&tl, ws);
        }
        // A workspace change leaves the pointer focus state stale.
        if let Some(child) = self.child() {
            child.set_hover(false);
        }
    }
}

impl ToplevelNodeBase for PendingFullscreenNode {
    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel
    }

    fn tl_accepts_keyboard_focus(&self) -> bool {
        true
    }

    fn tl_focus_child(&self) -> Option<Rc<dyn Node>> {
        let tl = self.toplevel_opt.get()?;
        if let Some(child) = tl.tl_focus_child() {
            return Some(child);
        }
        let node: Rc<dyn Node> = tl;
        Some(node)
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        self.location.set(ws.node_location());
        if let Some(tl) = self.toplevel_opt.get() {
            tl.tl_set_workspace(ws);
        }
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect) {
        self.toplevel.content_size.set(*rect);
        if let Some(c) = self.child.get() {
            c.clone().tl_change_extents(rect);
        }
        if let Some(p) = self.toplevel.parent.get() {
            p.node_child_size_changed(self.deref(), rect.width(), rect.height());
        }
    }

    fn tl_close(self: Rc<Self>) {
        let slf = self.clone();
        self.toplevel.state.run_toplevel.schedule(move || {
            slf.tl_destroy();
        });
    }

    fn tl_set_visible_impl(&self, visible: bool) {
        // The tile is rendered through its placeholder child.
        if let Some(child) = self.child() {
            child.tl_set_visible(visible);
        }
        if let Some(tl) = self.toplevel_opt.get() {
            tl.clone().tl_set_visible(visible);
        }
    }

    fn tl_destroy_impl(self: &Rc<Self>) {
        self.destroyed.set(true);
        if let Some(c) = self.child.take() {
            c.tl_destroy_dyn();
        }
        let Some(ws) = self.toplevel.workspace[LiveTL].get() else {
            return;
        };
        if let Some(tl) = self.toplevel_opt.get() {
            ws.remove_pending_fullscreen(tl.deref());
        }
    }

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        self.toplevel_opt.get().unwrap_or(self)
    }

    fn tl_admits_children(&self) -> bool {
        false
    }

    fn tl_tile_drag_destination(
        self: Rc<Self>,
        source: NodeId,
        split: Option<ContainerSplit>,
        abs_bounds: Rect,
        x: i32,
        y: i32,
    ) -> Option<TileDragDestination> {
        default_tile_drag_destination(self, source, split, abs_bounds, x, y)
    }

    fn tl_icon_surface_factory(&self) -> Option<Rc<JayIconSurfaceFactoryV1>> {
        self.toplevel_opt
            .get()
            .and_then(|tl| tl.tl_icon_surface_factory())
    }

    fn tl_schedule_data_op(self: Rc<Self>, op: ToplevelDataTransactionOp) {
        self.add_transaction_op(PendingFullscreenTransactionOp::ToplevelData(op));
    }
}

impl ContainingNode for PendingFullscreenNode {
    fn cnode_replace_child(self: Rc<Self>, _old: &dyn Node, _new: Rc<dyn ToplevelNode>) {
        // The slot only ever holds the placeholder of the wrapped window.
        log::error!("tried to replace a child of a pending fullscreen node");
    }

    fn cnode_remove_child2(self: Rc<Self>, _child: &dyn Node, _preserve_focus: bool) {
        self.clear_child();
    }

    fn cnode_accepts_child(&self, _node: &dyn Node) -> bool {
        false
    }

    fn cnode_workspace(self: Rc<Self>) -> Option<Rc<WorkspaceNode>> {
        self.toplevel.workspace[LiveTL].get()
    }

    fn cnode_make_visible(self: Rc<Self>, _child: &dyn Node) {
        self.node_make_visible();
    }

    fn cnode_child_attention_request_changed(self: Rc<Self>, _node: &dyn Node, _set: bool) {
        // nothing
    }
}

impl OutputEventListener for PendingFullscreenNode {
    fn scale_changed(self: Rc<Self>, _on: &Rc<OutputNode>, _scale: Scale) {
        if let Some(tl) = self.toplevel_opt.get() {
            tl.tl_data().scale_changed();
        }
    }
}

impl WorkspaceEventListener for PendingFullscreenNode {
    fn output_changed(
        self: Rc<Self>,
        _ws: &Rc<WorkspaceNode>,
        old: &Rc<OutputNode>,
        new: &Rc<OutputNode>,
    ) {
        if let Some(tl) = self.toplevel_opt.get() {
            tl.tl_data().workspace_output_changed(old, new);
        }
    }
}

impl Transactionable for PendingFullscreenNode {
    type T = PendingFullscreenTransactionOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        match op {
            PendingFullscreenTransactionOp::ToplevelData(v) => {
                self.toplevel.run_op(v);
            }
        }
    }
}

pub enum PendingFullscreenTransactionOp {
    ToplevelData(ToplevelDataTransactionOp),
}
