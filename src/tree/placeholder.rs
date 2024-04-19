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
        text::{self, TextTexture},
        tree::{
            Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeVisitor,
            ToplevelData, ToplevelNode, ToplevelNodeBase,
        },
        utils::{errorfmt::ErrorFmt, smallmap::SmallMap},
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
};

tree_id!(PlaceholderNodeId);

pub struct PlaceholderNode {
    id: PlaceholderNodeId,
    toplevel: ToplevelData,
    destroyed: Cell<bool>,
    pub textures: SmallMap<Scale, TextTexture, 2>,
}

impl PlaceholderNode {
    pub fn new_for(state: &Rc<State>, node: Rc<dyn ToplevelNode>) -> Self {
        Self {
            id: state.node_ids.next(),
            toplevel: ToplevelData::new(
                state,
                node.tl_data().title.borrow().clone(),
                node.node_client(),
            ),
            destroyed: Default::default(),
            textures: Default::default(),
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn update_texture(&self) {
        if let Some(ctx) = self.toplevel.state.render_ctx.get() {
            let scales = self.toplevel.state.scales.lock();
            let rect = self.toplevel.pos.get();
            for (scale, _) in scales.iter() {
                let old_tex = self.textures.remove(scale);
                let mut width = rect.width();
                let mut height = rect.height();
                if *scale != 1 {
                    let scale = scale.to_f64();
                    width = (width as f64 * scale).round() as _;
                    height = (height as f64 * scale).round() as _;
                }
                if width != 0 && height != 0 {
                    let font = format!("monospace {}", width / 10);
                    match text::render_fitting(
                        &ctx,
                        old_tex,
                        Some(height),
                        &font,
                        "Fullscreen",
                        self.toplevel.state.theme.colors.unfocused_title_text.get(),
                        false,
                        None,
                    ) {
                        Ok(t) => {
                            self.textures.insert(*scale, t);
                        }
                        Err(e) => {
                            log::warn!("Could not render fullscreen texture: {}", ErrorFmt(e));
                        }
                    }
                }
            }
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

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.set_known_cursor(KnownCursor::Default);
        seat.enter_toplevel(self.clone());
    }

    fn node_is_placeholder(&self) -> bool {
        true
    }

    fn node_into_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }
}

impl ToplevelNodeBase for PlaceholderNode {
    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect) {
        self.toplevel.pos.set(*rect);
        if let Some(p) = self.toplevel.parent.get() {
            p.node_child_size_changed(self.deref(), rect.width(), rect.height());
        }
        self.update_texture();
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
}
