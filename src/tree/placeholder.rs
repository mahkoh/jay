use crate::client::Client;
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::NodeSeatState;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::rect::Rect;
use crate::renderer::Renderer;
use crate::scale::Scale;
use crate::state::GfxCtxChangedListener;
use crate::state::OutputEventListener;
use crate::state::ScalesChangedListener;
use crate::state::State;
use crate::text::TextError;
use crate::text::TextTexture;
use crate::transactions::TransactionData;
use crate::transactions::Transactionable;
use crate::transactions::TransactionableExt;
use crate::tree::ContainerSplit;
use crate::tree::Direction;
use crate::tree::FindTreeResult;
use crate::tree::FindTreeUsecase;
use crate::tree::FoundNode;
use crate::tree::NodeBase;
use crate::tree::NodeId;
use crate::tree::NodeLayerLink;
use crate::tree::NodeLocation;
use crate::tree::NodeVisitor;
use crate::tree::OutputNode;
use crate::tree::TileDragDestination;
use crate::tree::ToplevelData;
use crate::tree::ToplevelDataTransactionOp;
use crate::tree::ToplevelNode;
use crate::tree::ToplevelNodeBase;
use crate::tree::ToplevelType;
use crate::tree::TreeTimeline;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::WorkspaceEventListener;
use crate::tree::WorkspaceNode;
use crate::tree::default_tile_drag_destination;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::event_listener::EventListener;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::on_drop_event::OnDropEvent;
use crate::utils::smallmap::SmallMapMut;
use jay_proc::GetLiveness;
use std::cell::Cell;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;

mod placeholder_dfs_g_fuse;

tree_id!(PlaceholderNodeId);

const LABEL_TEXT: &str = "Fullscreen";
const LABEL_FONT_MIN: i32 = 14;
const LABEL_FONT_MAX: i32 = 48;
const HINT_TEXT: &str = "left click: fullscreen\nright click: exit fullscreen";
const HINT_FONT_MAX: i32 = 14;

// The hint is drawn inside the tile, so its font has to follow the extent of the
// tile: a fixed size overflows a small tile, and a tile of a display at a large
// scale is small in logical pixels. A monospace advance is about 3/5 em, so the
// longer line fits into the width of the tile when the font is at most 5/3 times
// the width divided by the number of characters, and both lines fit into the
// height of the tile when the font is at most 5/12 of the height. The upper bound is
// the size the hint was drawn at before, larger text would only cover the preview.
fn hint_font_size(width: i32, height: i32) -> i32 {
    let chars = HINT_TEXT
        .lines()
        .map(|line| line.chars().count() as i32)
        .max()
        .unwrap_or(1);
    let by_width = width * 5 / (3 * chars);
    let by_height = height * 5 / 12;
    by_width.min(by_height).clamp(1, HINT_FONT_MAX)
}

#[derive(GetLiveness)]
pub struct PlaceholderNode {
    id: PlaceholderNodeId,
    #[liveness]
    toplevel: ToplevelData,
    destroyed: Cell<bool>,
    update_textures_scheduled: Cell<bool>,
    in_pending_wrapper: Cell<bool>,
    hover: Cell<bool>,
    state: Rc<State>,
    location: Cell<Option<NodeLocation>>,
    pub textures: RefCell<SmallMapMut<Scale, TextTexture, 2>>,
    pub label_textures: RefCell<SmallMapMut<Scale, TextTexture, 2>>,
    transaction_data: TransactionData<PlaceholderTransactionOp>,
    _gfx_ctx_listener: EventListener<dyn GfxCtxChangedListener>,
    _scales_listener: EventListener<dyn ScalesChangedListener>,
}

pub async fn placeholder_render_textures(state: Rc<State>) {
    loop {
        let container = state.pending_placeholder_render_textures.pop().await;
        container.update_textures_scheduled.set(false);
        container.update_texture_phase1().triggered().await;
        container.update_texture_phase2();
    }
}

impl PlaceholderNode {
    pub fn new_for(state: &Rc<State>, node: Rc<dyn ToplevelNode>, slf: &Weak<Self>) -> Self {
        let id = state.node_ids.next();
        Self {
            id,
            toplevel: ToplevelData::new(
                state,
                node.tl_data().title.borrow().clone(),
                node.node_client(),
                ToplevelType::Placeholder(Some(node.tl_data().identifier.get())),
                id,
                slf,
            ),
            destroyed: Default::default(),
            update_textures_scheduled: Cell::new(false),
            in_pending_wrapper: Cell::new(false),
            hover: Cell::new(false),
            state: state.clone(),
            location: Cell::new(node.node_location()),
            textures: Default::default(),
            label_textures: Default::default(),
            transaction_data: TransactionData::new(&state.tree),
            _gfx_ctx_listener: EventListener::attached(slf.clone(), &state.gfx_ctx_changed),
            _scales_listener: EventListener::attached(slf.clone(), &state.scales_changed),
        }
    }

    pub fn new_empty(state: &Rc<State>, slf: &Weak<Self>) -> Self {
        let id = state.node_ids.next();
        Self {
            id,
            toplevel: ToplevelData::new(
                state,
                String::new(),
                None,
                ToplevelType::Placeholder(None),
                id,
                slf,
            ),
            destroyed: Default::default(),
            update_textures_scheduled: Default::default(),
            in_pending_wrapper: Default::default(),
            hover: Default::default(),
            state: state.clone(),
            location: Default::default(),
            textures: Default::default(),
            label_textures: Default::default(),
            transaction_data: TransactionData::new(&state.tree),
            _gfx_ctx_listener: EventListener::attached(slf.clone(), &state.gfx_ctx_changed),
            _scales_listener: EventListener::attached(slf.clone(), &state.scales_changed),
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn set_in_pending_wrapper(self: &Rc<Self>, in_pending_wrapper: bool) {
        self.in_pending_wrapper.set(in_pending_wrapper);
        self.textures.borrow_mut().clear();
        self.schedule_update_texture();
    }

    pub fn in_pending_wrapper(&self) -> bool {
        self.in_pending_wrapper.get()
    }

    pub fn hover(&self) -> bool {
        self.hover.get()
    }

    pub fn set_hover(self: &Rc<Self>, hover: bool) {
        if self.hover.get() != hover {
            self.hover.set(hover);
            self.schedule_update_texture();
        }
    }

    fn schedule_update_texture(self: &Rc<Self>) {
        if !self.update_textures_scheduled.replace(true) {
            self.state
                .pending_placeholder_render_textures
                .push(self.clone());
        }
    }

    fn update_texture_phase1(&self) -> Rc<AsyncEvent> {
        let on_completed = Rc::new(OnDropEvent::default());
        let Some(ctx) = self.toplevel.state.render_ctx.get() else {
            return on_completed.event();
        };
        let scales = self.toplevel.state.scales.lock();
        let rect = self.toplevel.content_size.get();
        let mut textures = self.textures.borrow_mut();
        let mut label_textures = self.label_textures.borrow_mut();
        for (scale, _) in scales.iter() {
            let mut width = rect.width();
            let mut height = rect.height();
            if *scale != 1 {
                let scale = scale.to_f64();
                width = (width as f64 * scale).round() as _;
                height = (height as f64 * scale).round() as _;
            }
            // The text renderer multiplies the font by the scale it is given, so the
            // sizes below stay logical. Scaling them here as well grew the text with
            // the square of the scale.
            if width != 0 && height != 0 {
                // A tile-sized label texture would cost tens of MB per scale.
                let logical = rect.width().min(rect.height());
                let font_size = (logical / 10).clamp(LABEL_FONT_MIN, LABEL_FONT_MAX);
                let font = Arc::from(format!("monospace {font_size}").into_boxed_str());
                let label = label_textures
                    .get_or_insert_with(*scale, || TextTexture::new(&self.state, &ctx));
                label.schedule_render_fitting(
                    on_completed.clone(),
                    None,
                    &font,
                    LABEL_TEXT,
                    self.toplevel.state.theme.colors.unfocused_title_text.get(),
                    false,
                    scale.to_f64().into(),
                );
                if self.in_pending_wrapper.get() && self.hover.get() {
                    let tex =
                        textures.get_or_insert_with(*scale, || TextTexture::new(&self.state, &ctx));
                    let font_size = hint_font_size(rect.width(), rect.height());
                    let font = Arc::from(format!("monospace {font_size}").into_boxed_str());
                    let markup = format!("<span background=\"#101010\">{HINT_TEXT}</span>");
                    tex.schedule_render_fitting(
                        on_completed.clone(),
                        None,
                        &font,
                        &markup,
                        self.toplevel.state.theme.colors.unfocused_title_text.get(),
                        true,
                        scale.to_f64().into(),
                    );
                }
            }
        }
        on_completed.event()
    }

    fn update_texture_phase2(&self) {
        let textures = &*self.textures.borrow();
        for (_, texture) in textures {
            if let Err(e) = texture.flip() {
                // A hint texture of a tile that was never hovered is not scheduled.
                if !matches!(e, TextError::NotScheduled) {
                    log::warn!("Could not render placeholder texture: {}", ErrorFmt(e));
                }
            }
        }
        let label_textures = &*self.label_textures.borrow();
        for (_, texture) in label_textures {
            if let Err(e) = texture.flip() {
                if !matches!(e, TextError::NotScheduled) {
                    log::warn!("Could not render placeholder label: {}", ErrorFmt(e));
                }
            }
        }
        // Damage has to follow the current extents, not the last paint: a float
        // drag moves the tile to another output without painting on the old one.
        let rect = self.toplevel.desired_extents.get();
        if self.node_visible(RenderTL) || self.in_pending_wrapper.get() {
            if !rect.is_empty() {
                self.state.damage(rect);
            }
        }
    }
}

impl NodeBase for PlaceholderNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.toplevel.seat_state
    }

    fn node_visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_placeholder(&self);
    }

    fn node_visit_children(&self, _visitor: &mut dyn NodeVisitor) {
        // nothing
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

    fn node_do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_toplevel(self.clone());
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
        renderer.render_placeholder(self, x, y, bounds);
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

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.pointer_cursor().set_known(KnownCursor::Default);
        seat.enter_toplevel(self.clone());
    }

    fn node_into_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }

    fn node_is_placeholder(&self) -> bool {
        true
    }
}

impl ToplevelNodeBase for PlaceholderNode {
    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        self.location.set(ws.node_location());
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect) {
        self.toplevel.content_size.set(*rect);
        if let Some(p) = self.toplevel.parent.get() {
            p.node_child_size_changed(self.deref(), rect.width(), rect.height());
        }
        self.schedule_update_texture();
    }

    fn tl_close(self: Rc<Self>) {
        let slf = self.clone();
        self.toplevel.state.run_toplevel.schedule(move || {
            slf.tl_destroy();
        });
    }

    fn tl_set_visible_impl(&self, _visible: bool) {
        // nothing
    }

    fn tl_destroy_impl(self: &Rc<Self>) {
        self.destroyed.set(true);
    }

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        self
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

    fn tl_schedule_data_op(self: Rc<Self>, op: ToplevelDataTransactionOp) {
        self.add_transaction_op(PlaceholderTransactionOp::ToplevelData(op));
    }
}

impl OutputEventListener for PlaceholderNode {
    fn scale_changed(self: Rc<Self>, _on: &Rc<OutputNode>, _scale: Scale) {
        self.toplevel.scale_changed();
    }
}

impl WorkspaceEventListener for PlaceholderNode {
    fn output_changed(
        self: Rc<Self>,
        _ws: &Rc<WorkspaceNode>,
        old: &Rc<OutputNode>,
        new: &Rc<OutputNode>,
    ) {
        self.toplevel.workspace_output_changed(old, new);
    }
}

impl GfxCtxChangedListener for PlaceholderNode {
    fn handle_gfx_context_change(self: Rc<Self>) {
        self.textures.borrow_mut().clear();
        self.label_textures.borrow_mut().clear();
        self.schedule_update_texture();
    }
}

impl ScalesChangedListener for PlaceholderNode {
    fn changed(self: Rc<Self>) {
        self.textures.borrow_mut().clear();
        self.label_textures.borrow_mut().clear();
        self.schedule_update_texture();
    }
}

pub enum PlaceholderTransactionOp {
    ToplevelData(ToplevelDataTransactionOp),
}

impl Transactionable for PlaceholderNode {
    type T = PlaceholderTransactionOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        match op {
            PlaceholderTransactionOp::ToplevelData(v) => {
                self.toplevel.run_op(v);
            }
        }
    }
}
