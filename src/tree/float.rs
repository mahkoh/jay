use {
    crate::{
        backend::KeyState,
        cursor::KnownCursor,
        cursor_user::CursorUser,
        fixed::Fixed,
        ifs::wl_seat::{
            BTN_LEFT, NodeSeatState, SeatId, WlSeatGlobal,
            tablet::{TabletTool, TabletToolChanges, TabletToolId},
        },
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        state::State,
        text::TextTexture,
        tree::{
            ContainingNode, Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId,
            StackedNode, TileDragDestination, ToplevelNode, WorkspaceNode, walker::NodeVisitor,
        },
        utils::{
            asyncevent::AsyncEvent, clonecell::CloneCell, double_click_state::DoubleClickState,
            errorfmt::ErrorFmt, linkedlist::LinkedNode, on_drop_event::OnDropEvent,
            smallmap::SmallMapMut,
        },
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        ops::Deref,
        rc::Rc,
    },
};

tree_id!(FloatNodeId);
pub struct FloatNode {
    pub id: FloatNodeId,
    pub state: Rc<State>,
    pub visible: Cell<bool>,
    pub position: Cell<Rect>,
    pub display_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
    pub workspace_link: Cell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
    pub workspace: CloneCell<Rc<WorkspaceNode>>,
    pub child: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    pub active: Cell<bool>,
    pub seat_state: NodeSeatState,
    pub layout_scheduled: Cell<bool>,
    pub render_titles_scheduled: Cell<bool>,
    pub title_rect: Cell<Rect>,
    pub title: RefCell<String>,
    pub title_textures: RefCell<SmallMapMut<Scale, TextTexture, 2>>,
    cursors: RefCell<AHashMap<CursorType, CursorState>>,
    pub attention_requested: Cell<bool>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum CursorType {
    Seat(SeatId),
    TabletTool(TabletToolId),
}

struct CursorState {
    cursor: KnownCursor,
    target: bool,
    x: i32,
    y: i32,
    op_type: OpType,
    op_active: bool,
    dist_hor: i32,
    dist_ver: i32,
    double_click_state: DoubleClickState,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum OpType {
    Move,
    ResizeLeft,
    ResizeTop,
    ResizeRight,
    ResizeBottom,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
}

pub async fn float_layout(state: Rc<State>) {
    loop {
        let node = state.pending_float_layout.pop().await;
        if node.layout_scheduled.get() {
            node.perform_layout();
        }
    }
}

pub async fn float_titles(state: Rc<State>) {
    loop {
        let node = state.pending_float_titles.pop().await;
        if node.render_titles_scheduled.get() {
            node.render_titles_scheduled.set(false);
            node.render_title_phase1().triggered().await;
            node.render_title_phase2();
        }
    }
}

impl FloatNode {
    pub fn new(
        state: &Rc<State>,
        ws: &Rc<WorkspaceNode>,
        position: Rect,
        child: Rc<dyn ToplevelNode>,
    ) -> Rc<Self> {
        let floater = Rc::new(FloatNode {
            id: state.node_ids.next(),
            state: state.clone(),
            visible: Cell::new(ws.float_visible()),
            position: Cell::new(position),
            display_link: RefCell::new(None),
            workspace_link: Cell::new(None),
            workspace: CloneCell::new(ws.clone()),
            child: CloneCell::new(Some(child.clone())),
            active: Cell::new(false),
            seat_state: Default::default(),
            layout_scheduled: Cell::new(false),
            render_titles_scheduled: Cell::new(false),
            title_rect: Default::default(),
            title: Default::default(),
            title_textures: Default::default(),
            cursors: Default::default(),
            attention_requested: Cell::new(false),
        });
        floater.pull_child_properties();
        *floater.display_link.borrow_mut() = Some(state.root.stacked.add_last(floater.clone()));
        floater
            .workspace_link
            .set(Some(ws.stacked.add_last(floater.clone())));
        child.tl_set_parent(floater.clone());
        child.tl_set_visible(floater.visible.get());
        child.tl_restack_popups();
        floater.schedule_layout();
        if floater.visible.get() {
            state.damage(position);
        }
        floater
    }

    pub fn on_spaces_changed(self: &Rc<Self>) {
        self.schedule_layout();
    }

    pub fn on_colors_changed(self: &Rc<Self>) {
        self.schedule_render_titles();
    }

    pub fn schedule_layout(self: &Rc<Self>) {
        if !self.layout_scheduled.replace(true) {
            self.state.pending_float_layout.push(self.clone());
        }
    }

    fn perform_layout(self: &Rc<Self>) {
        let child = match self.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = self.position.get();
        let theme = &self.state.theme;
        let bw = theme.sizes.border_width.get();
        let th = theme.sizes.title_height.get();
        let cpos = Rect::new_sized(
            pos.x1() + bw,
            pos.y1() + bw + th + 1,
            (pos.width() - 2 * bw).max(0),
            (pos.height() - 2 * bw - th - 1).max(0),
        )
        .unwrap();
        let tr = Rect::new_sized(bw, bw, (pos.width() - 2 * bw).max(0), th).unwrap();
        child.clone().tl_change_extents(&cpos);
        self.title_rect.set(tr);
        self.layout_scheduled.set(false);
        self.schedule_render_titles();
    }

    pub fn schedule_render_titles(self: &Rc<Self>) {
        if !self.render_titles_scheduled.replace(true) {
            self.state.pending_float_titles.push(self.clone());
        }
    }

    fn render_title_phase1(&self) -> Rc<AsyncEvent> {
        let on_completed = Rc::new(OnDropEvent::default());
        let theme = &self.state.theme;
        let tc = match self.active.get() {
            true => theme.colors.focused_title_text.get(),
            false => theme.colors.unfocused_title_text.get(),
        };
        let bw = theme.sizes.border_width.get();
        let font = theme.font.get();
        let title = self.title.borrow_mut();
        let pos = self.position.get();
        if pos.width() <= 2 * bw {
            return on_completed.event();
        }
        let ctx = match self.state.render_ctx.get() {
            Some(c) => c,
            _ => return on_completed.event(),
        };
        let scales = self.state.scales.lock();
        let tr = self.title_rect.get();
        let tt = &mut *self.title_textures.borrow_mut();
        for (scale, _) in scales.iter() {
            let tex =
                tt.get_or_insert_with(*scale, || TextTexture::new(&self.state.cpu_worker, &ctx));
            let mut th = tr.height();
            let mut scalef = None;
            let mut width = tr.width();
            if *scale != 1 {
                let scale = scale.to_f64();
                th = (th as f64 * scale).round() as _;
                width = (width as f64 * scale).round() as _;
                scalef = Some(scale);
            }
            if th == 0 || width == 0 {
                continue;
            }
            tex.schedule_render(
                on_completed.clone(),
                1,
                None,
                width,
                th,
                1,
                &font,
                &title,
                tc,
                true,
                false,
                scalef,
            );
        }
        on_completed.event()
    }

    fn render_title_phase2(&self) {
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let title = self.title.borrow();
        let tt = &*self.title_textures.borrow();
        for (_, tt) in tt {
            if let Err(e) = tt.flip() {
                log::error!("Could not render title {}: {}", title, ErrorFmt(e));
            }
        }
        let pos = self.position.get();
        if self.visible.get() && pos.width() >= 2 * bw {
            let tr =
                Rect::new_sized(pos.x1() + bw, pos.y1() + bw, pos.width() - 2 * bw, th).unwrap();
            self.state.damage(tr);
        }
    }

    fn pointer_move(
        self: &Rc<Self>,
        id: CursorType,
        cursor: &CursorUser,
        x: Fixed,
        y: Fixed,
        target: bool,
    ) {
        let x = x.round_down();
        let y = y.round_down();
        let theme = &self.state.theme;
        let bw = theme.sizes.border_width.get();
        let th = theme.sizes.title_height.get();
        let mut seats = self.cursors.borrow_mut();
        let seat_state = seats.entry(id).or_insert_with(|| CursorState {
            cursor: KnownCursor::Default,
            target,
            x,
            y,
            op_type: OpType::Move,
            op_active: false,
            dist_hor: 0,
            dist_ver: 0,
            double_click_state: Default::default(),
        });
        seat_state.x = x;
        seat_state.y = y;
        let pos = self.position.get();
        if seat_state.op_active {
            let mut x1 = pos.x1();
            let mut y1 = pos.y1();
            let mut x2 = pos.x2();
            let mut y2 = pos.y2();
            match seat_state.op_type {
                OpType::Move => {
                    let dx = x - seat_state.dist_hor;
                    let dy = y - seat_state.dist_ver;
                    x1 += dx;
                    y1 += dy;
                    x2 += dx;
                    y2 += dy;
                }
                OpType::ResizeLeft => {
                    x1 += x - seat_state.dist_hor;
                    x1 = x1.min(x2 - 2 * bw);
                }
                OpType::ResizeTop => {
                    y1 += y - seat_state.dist_ver;
                    y1 = y1.min(y2 - 2 * bw - th - 1);
                }
                OpType::ResizeRight => {
                    x2 += x - pos.width() + seat_state.dist_hor;
                    x2 = x2.max(x1 + 2 * bw);
                }
                OpType::ResizeBottom => {
                    y2 += y - pos.height() + seat_state.dist_ver;
                    y2 = y2.max(y1 + 2 * bw + th + 1);
                }
                OpType::ResizeTopLeft => {
                    x1 += x - seat_state.dist_hor;
                    y1 += y - seat_state.dist_ver;
                    x1 = x1.min(x2 - 2 * bw);
                    y1 = y1.min(y2 - 2 * bw - th - 1);
                }
                OpType::ResizeTopRight => {
                    x2 += x - pos.width() + seat_state.dist_hor;
                    y1 += y - seat_state.dist_ver;
                    x2 = x2.max(x1 + 2 * bw);
                    y1 = y1.min(y2 - 2 * bw - th - 1);
                }
                OpType::ResizeBottomLeft => {
                    x1 += x - seat_state.dist_hor;
                    y2 += y - pos.height() + seat_state.dist_ver;
                    x1 = x1.min(x2 - 2 * bw);
                    y2 = y2.max(y1 + 2 * bw + th + 1);
                }
                OpType::ResizeBottomRight => {
                    x2 += x - pos.width() + seat_state.dist_hor;
                    y2 += y - pos.height() + seat_state.dist_ver;
                    x2 = x2.max(x1 + 2 * bw);
                    y2 = y2.max(y1 + 2 * bw + th + 1);
                }
            }
            let new_pos = Rect::new(x1, y1, x2, y2).unwrap();
            self.position.set(new_pos);
            if self.visible.get() {
                self.state.damage(pos);
                self.state.damage(new_pos);
            }
            self.schedule_layout();
            return;
        }
        let resize_left = x < bw;
        let resize_right = x >= pos.width() - bw;
        let resize_top = y < bw;
        let resize_bottom = y >= pos.height() - bw;
        let id = 0
            | ((resize_left as usize) << 0)
            | ((resize_right as usize) << 1)
            | ((resize_top as usize) << 2)
            | ((resize_bottom as usize) << 3);
        const OP_TYPES: [OpType; 16] = [
            OpType::Move,              // 0000
            OpType::ResizeLeft,        // 0001
            OpType::ResizeRight,       // 0010
            OpType::Move,              // 0011
            OpType::ResizeTop,         // 0100
            OpType::ResizeTopLeft,     // 0101
            OpType::ResizeTopRight,    // 0110
            OpType::Move,              // 0111
            OpType::ResizeBottom,      // 1000
            OpType::ResizeBottomLeft,  // 1001
            OpType::ResizeBottomRight, // 1010
            OpType::Move,              // 1011
            OpType::Move,              // 1100
            OpType::Move,              // 1101
            OpType::Move,              // 1110
            OpType::Move,              // 1111
        ];
        let op_type = OP_TYPES[id];
        let new_cursor = match op_type {
            OpType::Move => KnownCursor::Default,
            OpType::ResizeLeft => KnownCursor::EwResize,
            OpType::ResizeTop => KnownCursor::NsResize,
            OpType::ResizeRight => KnownCursor::EwResize,
            OpType::ResizeBottom => KnownCursor::NsResize,
            OpType::ResizeTopLeft => KnownCursor::NwResize,
            OpType::ResizeTopRight => KnownCursor::NeResize,
            OpType::ResizeBottomLeft => KnownCursor::SwResize,
            OpType::ResizeBottomRight => KnownCursor::SeResize,
        };
        seat_state.op_type = op_type;
        if new_cursor != mem::replace(&mut seat_state.cursor, new_cursor) {
            if seat_state.target {
                cursor.set_known(new_cursor);
            }
        }
    }

    fn set_workspace(self: &Rc<Self>, ws: &Rc<WorkspaceNode>) {
        if let Some(c) = self.child.get() {
            c.tl_set_workspace(ws);
        }
        self.workspace_link
            .set(Some(ws.stacked.add_last(self.clone())));
        self.workspace.set(ws.clone());
        self.stacked_set_visible(ws.float_visible());
    }

    fn update_child_title(self: &Rc<Self>, title: &str) {
        let mut t = self.title.borrow_mut();
        if t.deref() != title {
            t.clear();
            t.push_str(title);
            self.schedule_render_titles();
        }
    }

    fn update_child_active(self: &Rc<Self>, active: bool) {
        if self.active.replace(active) != active {
            self.schedule_render_titles();
        }
    }

    fn pull_child_properties(self: &Rc<Self>) {
        let child = match self.child.get() {
            None => return,
            Some(c) => c,
        };
        let data = child.tl_data();
        let activation_requested = data.wants_attention.get();
        self.attention_requested.set(activation_requested);
        if activation_requested {
            self.workspace
                .get()
                .cnode_child_attention_request_changed(&**self, true);
        }
        self.update_child_title(&data.title.borrow());
        self.update_child_active(data.active());
    }

    fn discard_child_properties(&self) {
        if self.attention_requested.get() {
            self.workspace
                .get()
                .cnode_child_attention_request_changed(self, false);
        }
    }

    fn restack(&self) {
        if let Some(dl) = &*self.display_link.borrow() {
            self.state.root.stacked.add_last_existing(&dl);
            if let Some(tl) = self.child.get() {
                tl.tl_restack_popups();
            }
            self.state.tree_changed();
        }
    }

    fn button(
        self: Rc<Self>,
        id: CursorType,
        cursor: &CursorUser,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        pressed: bool,
    ) {
        let mut cursors = self.cursors.borrow_mut();
        let cursor_data = match cursors.get_mut(&id) {
            Some(s) => s,
            _ => return,
        };
        if !cursor_data.op_active {
            if !pressed {
                return;
            }
            if cursor_data.op_type == OpType::Move {
                if let Some(tl) = self.child.get() {
                    tl.node_do_focus(seat, Direction::Unspecified);
                }
            }
            if cursor_data.double_click_state.click(
                &self.state,
                time_usec,
                cursor_data.x,
                cursor_data.y,
            ) && cursor_data.op_type == OpType::Move
            {
                if let Some(tl) = self.child.get() {
                    drop(cursors);
                    seat.set_tl_floating(tl, false);
                    return;
                }
            }
            cursor_data.op_active = true;
            let pos = self.position.get();
            match cursor_data.op_type {
                OpType::Move => {
                    self.restack();
                    cursor_data.dist_hor = cursor_data.x;
                    cursor_data.dist_ver = cursor_data.y;
                }
                OpType::ResizeLeft => cursor_data.dist_hor = cursor_data.x,
                OpType::ResizeTop => cursor_data.dist_ver = cursor_data.y,
                OpType::ResizeRight => cursor_data.dist_hor = pos.width() - cursor_data.x,
                OpType::ResizeBottom => cursor_data.dist_ver = pos.height() - cursor_data.y,
                OpType::ResizeTopLeft => {
                    cursor_data.dist_hor = cursor_data.x;
                    cursor_data.dist_ver = cursor_data.y;
                }
                OpType::ResizeTopRight => {
                    cursor_data.dist_hor = pos.width() - cursor_data.x;
                    cursor_data.dist_ver = cursor_data.y;
                }
                OpType::ResizeBottomLeft => {
                    cursor_data.dist_hor = cursor_data.x;
                    cursor_data.dist_ver = pos.height() - cursor_data.y;
                }
                OpType::ResizeBottomRight => {
                    cursor_data.dist_hor = pos.width() - cursor_data.x;
                    cursor_data.dist_ver = pos.height() - cursor_data.y;
                }
            }
        } else if !pressed {
            cursor_data.op_active = false;
            let ws = cursor.output().ensure_workspace();
            self.set_workspace(&ws);
        }
    }

    pub fn tile_drag_destination(
        self: &Rc<Self>,
        source: NodeId,
        abs_x: i32,
        abs_y: i32,
    ) -> Option<TileDragDestination> {
        let child = self.child.get()?;
        let theme = &self.state.theme.sizes;
        let bw = theme.border_width.get();
        let th = theme.title_height.get();
        let pos = self.position.get();
        let body = Rect::new(
            pos.x1() + bw,
            pos.y1() + bw + th + 1,
            pos.x2() - bw,
            pos.y2() - bw,
        )?;
        child.tl_tile_drag_destination(source, None, body, abs_x, abs_y)
    }
}

impl Debug for FloatNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FloatNode").finish_non_exhaustive()
    }
}

impl Node for FloatNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_float(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.child.get() {
            c.node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        self.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn node_child_title_changed(self: Rc<Self>, _child: &dyn Node, title: &str) {
        self.update_child_title(title);
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let pos = self.position.get();
        if x < bw || x >= pos.width() - bw {
            return FindTreeResult::AcceptsInput;
        }
        if y < bw + th + 1 || y >= pos.height() - bw {
            return FindTreeResult::AcceptsInput;
        }
        let child = match self.child.get() {
            Some(c) => c,
            _ => return FindTreeResult::Other,
        };
        let x = x - bw;
        let y = y - bw - th - 1;
        tree.push(FoundNode {
            node: child.clone(),
            x,
            y,
        });
        child.node_find_tree_at(x, y, tree, usecase)
    }

    fn node_child_active_changed(self: Rc<Self>, _child: &dyn Node, active: bool, _depth: u32) {
        self.update_child_active(active);
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_floating(self, x, y)
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        _serial: u64,
    ) {
        if button != BTN_LEFT {
            return;
        }
        self.button(
            CursorType::Seat(seat.id()),
            seat.pointer_cursor(),
            seat,
            time_usec,
            state == KeyState::Pressed,
        );
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(
            CursorType::Seat(seat.id()),
            seat.pointer_cursor(),
            x,
            y,
            false,
        );
    }

    fn node_on_pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>) {
        let mut cursors = self.cursors.borrow_mut();
        let id = CursorType::Seat(seat.id());
        if let Some(seat_state) = cursors.get_mut(&id) {
            seat_state.target = false;
        }
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("float focus");
        let mut cursors = self.cursors.borrow_mut();
        let id = CursorType::Seat(seat.id());
        if let Some(seat_state) = cursors.get_mut(&id) {
            seat_state.target = true;
            seat.pointer_cursor().set_known(seat_state.cursor);
        }
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(
            CursorType::Seat(seat.id()),
            seat.pointer_cursor(),
            x,
            y,
            false,
        );
    }

    fn node_on_tablet_tool_leave(&self, tool: &Rc<TabletTool>, _time_usec: u64) {
        let id = CursorType::TabletTool(tool.id);
        self.cursors.borrow_mut().remove(&id);
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        _time_usec: u64,
        x: Fixed,
        y: Fixed,
    ) {
        tool.cursor().set_known(KnownCursor::Default);
        self.pointer_move(CursorType::TabletTool(tool.id), tool.cursor(), x, y, true);
    }

    fn node_on_tablet_tool_apply_changes(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
        x: Fixed,
        y: Fixed,
    ) {
        self.pointer_move(CursorType::TabletTool(tool.id), tool.cursor(), x, y, false);
        if let Some(changes) = changes {
            if let Some(pressed) = changes.down {
                self.button(
                    CursorType::TabletTool(tool.id),
                    tool.cursor(),
                    tool.seat(),
                    time_usec,
                    pressed,
                );
            }
        }
    }

    fn node_into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        Some(self.clone())
    }

    fn node_into_containing_node(self: Rc<Self>) -> Option<Rc<dyn ContainingNode>> {
        Some(self)
    }

    fn node_is_float(&self) -> bool {
        true
    }
}

impl ContainingNode for FloatNode {
    fn cnode_replace_child(self: Rc<Self>, _old: &dyn Node, new: Rc<dyn ToplevelNode>) {
        self.discard_child_properties();
        self.child.set(Some(new.clone()));
        new.tl_set_parent(self.clone());
        self.pull_child_properties();
        new.tl_set_visible(self.visible.get());
        self.schedule_layout();
        if self.visible.get() {
            self.state.damage(self.position.get());
        }
    }

    fn cnode_remove_child2(self: Rc<Self>, _child: &dyn Node, _preserve_focus: bool) {
        self.discard_child_properties();
        self.child.set(None);
        self.display_link.borrow_mut().take();
        self.workspace_link.set(None);
        if self.visible.get() {
            self.state.damage(self.position.get());
        }
    }

    fn cnode_accepts_child(&self, _node: &dyn Node) -> bool {
        true
    }

    fn cnode_child_attention_request_changed(self: Rc<Self>, _node: &dyn Node, set: bool) {
        if self.attention_requested.replace(set) != set {
            self.workspace
                .get()
                .cnode_child_attention_request_changed(&*self, set);
        }
    }

    fn cnode_workspace(self: Rc<Self>) -> Rc<WorkspaceNode> {
        self.workspace.get()
    }

    fn cnode_set_child_position(self: Rc<Self>, _child: &dyn Node, x: i32, y: i32) {
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let (x, y) = (x - bw, y - th - bw - 1);
        let pos = self.position.get();
        if pos.position() != (x, y) {
            let new_pos = pos.at_point(x, y);
            self.position.set(new_pos);
            self.state.damage(pos);
            self.state.damage(new_pos);
            self.schedule_layout();
        }
    }

    fn cnode_resize_child(
        self: Rc<Self>,
        _child: &dyn Node,
        new_x1: Option<i32>,
        new_y1: Option<i32>,
        new_x2: Option<i32>,
        new_y2: Option<i32>,
    ) {
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let pos = self.position.get();
        let mut x1 = pos.x1();
        let mut x2 = pos.x2();
        let mut y1 = pos.y1();
        let mut y2 = pos.y2();
        if let Some(v) = new_x1 {
            x1 = (v - bw).min(x2 - bw - bw);
        }
        if let Some(v) = new_x2 {
            x2 = (v + bw).max(x1 + bw + bw);
        }
        if let Some(v) = new_y1 {
            y1 = (v - th - bw - 1).min(y2 - bw - th - bw - 1);
        }
        if let Some(v) = new_y2 {
            y2 = (v + bw).max(y1 + bw + th + bw + 1);
        }
        let new_pos = Rect::new(x1, y1, x2, y2).unwrap();
        if new_pos != pos {
            self.position.set(new_pos);
            if self.visible.get() {
                self.state.damage(pos);
                self.state.damage(new_pos);
            }
            self.schedule_layout();
        }
    }
}

impl StackedNode for FloatNode {
    fn stacked_set_visible(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            self.state.damage(self.position.get());
        }
        if let Some(child) = self.child.get() {
            child.tl_set_visible(visible);
        }
        self.seat_state.set_visible(self, visible);
    }

    fn stacked_has_workspace_link(&self) -> bool {
        true
    }
}
