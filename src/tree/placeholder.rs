use {
    crate::{
        client::Client,
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::wl_seat::{NodeSeatState, WlSeatGlobal},
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        state::State,
        text::TextTexture,
        tree::{
            ContainerSplit, Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId,
            NodeLayerLink, NodeLocation, NodeVisitor, OutputNode, TileDragDestination,
            ToplevelData, ToplevelNode, ToplevelNodeBase, ToplevelType, WorkspaceNode,
            default_tile_drag_destination,
        },
        utils::{
            asyncevent::AsyncEvent, errorfmt::ErrorFmt, on_drop_event::OnDropEvent,
            smallmap::SmallMapMut,
        },
    },
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::{Rc, Weak},
        sync::Arc,
    },
};

tree_id!(PlaceholderNodeId);

pub struct PlaceholderNode {
    id: PlaceholderNodeId,
    toplevel: ToplevelData,
    destroyed: Cell<bool>,
    update_textures_scheduled: Cell<bool>,
    state: Rc<State>,
    location: Cell<Option<NodeLocation>>,
    pub textures: RefCell<SmallMapMut<Scale, TextTexture, 2>>,
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
            state: state.clone(),
            location: Cell::new(node.node_location()),
            textures: Default::default(),
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
            state: state.clone(),
            location: Default::default(),
            textures: Default::default(),
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn schedule_update_texture(self: &Rc<Self>) {
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
        let rect = self.toplevel.pos.get();
        let mut textures = self.textures.borrow_mut();
        for (scale, _) in scales.iter() {
            let tex = textures.get_or_insert_with(*scale, || TextTexture::new(&self.state, &ctx));
            let mut width = rect.width();
            let mut height = rect.height();
            if *scale != 1 {
                let scale = scale.to_f64();
                width = (width as f64 * scale).round() as _;
                height = (height as f64 * scale).round() as _;
            }
            if width != 0 && height != 0 {
                let font = Arc::new(format!("monospace {}", width / 10));
                tex.schedule_render_fitting(
                    on_completed.clone(),
                    Some(height),
                    &font,
                    "Fullscreen",
                    self.toplevel.state.theme.colors.unfocused_title_text.get(),
                    false,
                    None,
                );
            }
        }
        on_completed.event()
    }

    fn update_texture_phase2(&self) {
        let textures = &*self.textures.borrow();
        for (_, texture) in textures {
            if let Err(e) = texture.flip() {
                log::warn!("Could not render fullscreen texture: {}", ErrorFmt(e));
            }
        }
        if self.node_visible() {
            self.state.damage(self.node_absolute_position());
        }
    }
}

impl Node for PlaceholderNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.toplevel.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_placeholder(&self);
    }

    fn node_visit_children(&self, _visitor: &mut dyn NodeVisitor) {
        // nothing
    }

    fn node_visible(&self) -> bool {
        self.toplevel.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.toplevel.pos.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.toplevel.output_opt()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.location.get()
    }

    fn node_layer(&self) -> NodeLayerLink {
        self.toplevel.node_layer()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
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

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, bounds: Option<&Rect>) {
        renderer.render_placeholder(self, x, y, bounds);
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        self.toplevel.client.clone()
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }

    fn node_make_visible(self: Rc<Self>) {
        self.toplevel.make_visible(&*self);
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
        self.toplevel.pos.set(*rect);
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

    fn tl_destroy_impl(&self) {
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
}
