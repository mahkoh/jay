use {
    crate::{
        backend::ButtonState,
        cursor::KnownCursor,
        cursor_user::CursorUser,
        fixed::Fixed,
        ifs::{
            wl_seat::{
                BTN_LEFT, BTN_RIGHT, NodeSeatState, SeatId, WlSeatGlobal,
                tablet::{TabletTool, TabletToolChanges, TabletToolId},
            },
            wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1::{
                ToplevelIcon, ToplevelIconUser,
            },
        },
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        state::State,
        text::TextTexture,
        transactions::{TransactionData, Transactionable, TransactionableExt},
        tree::{
            ContainingNode, Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeBase,
            NodeId, NodeLayerLink, NodeLocation, NodeStackTransactionOp, NodesStack,
            NodesStackElement, OutputNode, PinnedNode, SplitView, StackedNode, TileDragDestination,
            ToplevelNode,
            TreeTimeline::{self, LiveTL, RenderTL},
            WorkspaceChangeReason, WorkspaceNode, WorkspaceType, toplevel_set_floating,
            walker::NodeVisitor,
        },
        utils::{
            asyncevent::AsyncEvent,
            bhash::BHashMap,
            clonecell::CloneCell,
            double_click_state::DoubleClickState,
            errorfmt::ErrorFmt,
            linkedlist::LinkedNode,
            on_drop_event::OnDropEvent,
            smallmap::{SmallMap, SmallMapMut},
        },
    },
    arrayvec::ArrayVec,
    derivative::Derivative,
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
    pub node_state: SplitView<FloatNodeState>,
    pub display_link: RefCell<NodesStackElement>,
    pub workspace_link: Cell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
    pub pinned_link: RefCell<Option<LinkedNode<Rc<dyn PinnedNode>>>>,
    pub workspace: CloneCell<Rc<WorkspaceNode>>,
    pub location: Cell<NodeLocation>,
    pub seat_state: NodeSeatState,
    pub layout_scheduled: Cell<bool>,
    pub render_titles_scheduled: Cell<bool>,
    pub title: RefCell<String>,
    pub title_textures: RefCell<SmallMapMut<Scale, TextTexture, 2>>,
    pub icon: ToplevelIconUser,
    pub icons: SmallMap<Scale, ToplevelIcon, 2>,
    cursors: RefCell<BHashMap<CursorType, CursorState>>,
    transaction_data: TransactionData<FloatTransactionOp>,
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct FloatNodeState {
    pub visible: Cell<bool>,
    pub position: Cell<Rect>,
    pub child: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    #[derivative(Default(value = "Cell::new(WorkspaceType::Normal)"))]
    pub workspace_ty: Cell<WorkspaceType>,
    pub title_rect: Cell<Rect>,
    pub active: Cell<bool>,
    pub attention_requested: Cell<bool>,
    pub pinned: Cell<bool>,
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

impl State {
    fn float_stack(&self, ty: WorkspaceType) -> &Rc<NodesStack> {
        match ty {
            WorkspaceType::Normal => &self.root.stacked,
            WorkspaceType::Overlay => &self.root.stacked_in_overlay,
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
            node_state: Default::default(),
            display_link: state.float_stack(ws.ty).element(),
            workspace_link: Cell::new(None),
            pinned_link: RefCell::new(None),
            workspace: CloneCell::new(ws.clone()),
            location: Cell::new(ws.location()),
            seat_state: Default::default(),
            layout_scheduled: Cell::new(false),
            render_titles_scheduled: Cell::new(false),
            title: Default::default(),
            title_textures: Default::default(),
            icon: state.toplevel_icon_user(),
            icons: Default::default(),
            cursors: Default::default(),
            transaction_data: TransactionData::new(&state.tree),
        });
        floater.set_ns_visible(ws.float_visible());
        floater.set_ns_position(position);
        floater.set_ns_child(Some(&child));
        floater.set_ns_workspace_type(ws.ty);
        child.tl_update_icon(&floater.icon);
        floater.pull_child_properties();
        {
            let dl = &mut *floater.display_link.borrow_mut();
            dl.link = Some(dl.stack.stacked.add_last(floater.clone()));
        }
        floater
            .workspace_link
            .set(Some(ws.stacked.add_last(floater.clone())));
        child.tl_set_parent(floater.clone());
        let ns = &floater.node_state[LiveTL];
        child.tl_set_visible(ns.visible.get());
        child.tl_restack_popups();
        floater.schedule_layout();
        if ns.visible.get() {
            floater.display_link.borrow().invalidate();
        }
        if child.tl_data().pinned.get() {
            floater.toggle_pinned();
        }
        floater
    }

    pub fn on_spaces_changed(self: &Rc<Self>) {
        if self.icon.set_size(self.state.theme.title_icon_size(LiveTL))
            && let Some(child) = self.node_state[LiveTL].child.get()
        {
            child.tl_update_icon(&self.icon);
        }
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
        let ns = &self.node_state[LiveTL];
        let child = match ns.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = ns.position.get();
        let theme = &self.state.theme;
        let bw = theme.sizes.border_width.get(LiveTL);
        let th = theme.title_height(LiveTL);
        let tpuh = theme.title_plus_underline_height(LiveTL);
        let cpos = Rect::new_sized_saturating(
            pos.x1() + bw,
            pos.y1() + bw + tpuh,
            pos.width() - 2 * bw,
            pos.height() - 2 * bw - tpuh,
        );
        let tr = Rect::new_sized_saturating(bw, bw, pos.width() - 2 * bw, th);
        child.clone().tl_change_extents(&cpos);
        self.set_ns_title_rect(tr);
        self.layout_scheduled.set(false);
        self.schedule_render_titles();
    }

    pub fn schedule_render_titles(self: &Rc<Self>) {
        self.add_transaction_op(FloatTransactionOp::ScheduleRenderTitles);
    }

    fn render_title_phase1(&self) -> Rc<AsyncEvent> {
        let on_completed = Rc::new(OnDropEvent::default());
        let theme = &self.state.theme;
        let tc = match self.node_state[RenderTL].active.get() {
            true => theme.colors.focused_title_text.get(),
            false => theme.colors.unfocused_title_text.get(),
        };
        let font = theme.title_font();
        let title = self.title.borrow_mut();
        let ctx = match self.state.render_ctx.get() {
            Some(c) => c,
            _ => return on_completed.event(),
        };
        let scales = self.state.scales.lock();
        let tr = self.node_state[RenderTL].title_rect.get();
        let tt = &mut *self.title_textures.borrow_mut();
        self.icons.clear();
        for (scale, _) in scales.iter() {
            let tex = tt.get_or_insert_with(*scale, || TextTexture::new(&self.state, &ctx));
            let mut th = tr.height();
            let mut scalef = None;
            let mut width = tr.width();
            let icon = self
                .state
                .theme
                .show_window_icons
                .get()
                .then(|| self.icon.get(*scale))
                .flatten();
            if let Some(icon) = icon {
                width = (width - th).max(0);
                self.icons.insert(*scale, icon);
            }
            if self.node_state[RenderTL].workspace_ty.get() == WorkspaceType::Overlay {
                width = (width - th).max(0);
            }
            if self.state.show_pin_icon.get() || self.pinned_link.borrow().is_some() {
                width = (width - th).max(0);
            }
            if *scale != 1 {
                let scale = scale.to_f64();
                th = (th as f64 * scale).round() as _;
                width = (width as f64 * scale).round() as _;
                scalef = Some(scale);
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
        let th = theme.title_height(RenderTL);
        let bw = theme.sizes.border_width.get(RenderTL);
        let title = self.title.borrow();
        let tt = &*self.title_textures.borrow();
        for (_, tt) in tt {
            if let Err(e) = tt.flip() {
                log::error!("Could not render title {}: {}", title, ErrorFmt(e));
            }
        }
        let ns = &self.node_state[RenderTL];
        let pos = ns.position.get();
        if ns.visible.get() && pos.width() >= 2 * bw {
            let tr =
                Rect::new_sized_saturating(pos.x1() + bw, pos.y1() + bw, pos.width() - 2 * bw, th);
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
        let bw = theme.sizes.border_width.get(LiveTL);
        let tpuh = theme.title_plus_underline_height(LiveTL);
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
        let ns = &self.node_state[LiveTL];
        let pos = ns.position.get();
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
                    y1 = y1.min(y2 - 2 * bw - tpuh);
                }
                OpType::ResizeRight => {
                    x2 += x - pos.width() + seat_state.dist_hor;
                    x2 = x2.max(x1 + 2 * bw);
                }
                OpType::ResizeBottom => {
                    y2 += y - pos.height() + seat_state.dist_ver;
                    y2 = y2.max(y1 + 2 * bw + tpuh);
                }
                OpType::ResizeTopLeft => {
                    x1 += x - seat_state.dist_hor;
                    y1 += y - seat_state.dist_ver;
                    x1 = x1.min(x2 - 2 * bw);
                    y1 = y1.min(y2 - 2 * bw - tpuh);
                }
                OpType::ResizeTopRight => {
                    x2 += x - pos.width() + seat_state.dist_hor;
                    y1 += y - seat_state.dist_ver;
                    x2 = x2.max(x1 + 2 * bw);
                    y1 = y1.min(y2 - 2 * bw - tpuh);
                }
                OpType::ResizeBottomLeft => {
                    x1 += x - seat_state.dist_hor;
                    y2 += y - pos.height() + seat_state.dist_ver;
                    x1 = x1.min(x2 - 2 * bw);
                    y2 = y2.max(y1 + 2 * bw + tpuh);
                }
                OpType::ResizeBottomRight => {
                    x2 += x - pos.width() + seat_state.dist_hor;
                    y2 += y - pos.height() + seat_state.dist_ver;
                    x2 = x2.max(x1 + 2 * bw);
                    y2 = y2.max(y1 + 2 * bw + tpuh);
                }
            }
            let new_pos = Rect::new_saturating(x1, y1, x2, y2);
            self.set_ns_position(new_pos);
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

    fn set_workspace_(
        self: &Rc<Self>,
        ws: &Rc<WorkspaceNode>,
        update_pinned: bool,
        update_visible: bool,
    ) {
        let ns = &self.node_state[LiveTL];
        if let Some(c) = ns.child.get() {
            c.tl_set_workspace(ws);
        }
        self.workspace_link
            .set(Some(ws.stacked.add_last(self.clone())));
        self.workspace.set(ws.clone());
        if self.node_state[LiveTL].workspace_ty.get() != ws.ty {
            self.set_ns_workspace_type(ws.ty);
            self.display_link
                .borrow_mut()
                .restack_on(self.state.float_stack(ws.ty));
            if self.node_visible(RenderTL) {
                self.state.damage(self.node_absolute_position(RenderTL));
            }
        }
        self.location.set(ws.location());
        if update_visible {
            self.set_visible(ws.float_visible());
        }
        if update_pinned && let Some(pl) = &*self.pinned_link.borrow_mut() {
            ws.node_state[LiveTL]
                .output
                .get()
                .pinned
                .add_last_existing(pl);
        }
    }

    pub fn after_ws_move(self: &Rc<Self>, output: &Rc<OutputNode>) {
        if let Some(pinned) = &*self.pinned_link.borrow() {
            output.pinned.add_last_existing(pinned);
        }
    }

    pub fn ensure_on_output(self: &Rc<Self>, output: &Rc<OutputNode>) {
        if output.is_dummy {
            return;
        }
        let ns = &self.node_state[LiveTL];
        let pos = ns.position.get();
        let opos = output.node_state[LiveTL].pos.get();
        if pos.intersects(&opos) {
            return;
        }
        let bw = self.state.theme.sizes.border_width.get(LiveTL);
        let th = self.state.theme.title_height(LiveTL);
        let mut x1 = pos.x1();
        let mut x2 = pos.x2();
        let mut y1 = pos.y1();
        let mut y2 = pos.y2();
        const DELTA: i32 = 100;
        let delta = bw + DELTA;
        macro_rules! adjust {
            ($z1:ident, $z2:ident) => {
                if $z1 > opos.$z2() - delta {
                    $z1 = (opos.$z2() - delta).max(opos.$z1());
                    $z2 += $z1 - pos.$z1();
                } else if $z2 < opos.$z1() + delta {
                    $z2 = (opos.$z1() + delta).min(opos.$z2());
                    $z1 += $z2 - pos.$z2();
                }
            };
        }
        adjust!(x1, x2);
        adjust!(y1, y2);
        if y1 + bw + th <= opos.y1() {
            y1 = opos.y1();
            y2 += y1 - pos.y1();
        }
        let new_pos = Rect::new_saturating(x1, y1, x2, y2);
        self.set_ns_position(new_pos);
        self.schedule_layout();
    }

    pub fn move_(self: &Rc<Self>, dx: i32, dy: i32) {
        let ns = &self.node_state[LiveTL];
        let old_pos = ns.position.get();
        let new_pos = old_pos.move_(dx, dy);
        self.set_ns_position(new_pos);
        self.schedule_layout();
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
        if self.node_state[LiveTL].active.get() != active {
            self.set_ns_active(active);
            self.schedule_render_titles();
        }
    }

    fn pull_child_properties(self: &Rc<Self>) {
        let child = match self.node_state[LiveTL].child.get() {
            None => return,
            Some(c) => c,
        };
        let data = child.tl_data();
        let activation_requested = data.wants_attention.get();
        self.set_ns_attention_requested(activation_requested);
        if activation_requested {
            self.workspace
                .get()
                .cnode_child_attention_request_changed(&**self, true);
        }
        self.update_child_title(&data.title.borrow());
        self.update_child_active(data.active());
    }

    fn discard_child_properties(&self) {
        if self.node_state[LiveTL].attention_requested.get() {
            self.workspace
                .get()
                .cnode_child_attention_request_changed(self, false);
        }
    }

    fn restack(self: &Rc<Self>) {
        let dl = &*self.display_link.borrow();
        if let Some(link) = &dl.link {
            if link.next().is_none() {
                return;
            }
            let ns = &self.node_state[LiveTL];
            if self.node_visible(RenderTL) {
                self.state.damage(self.node_absolute_position(RenderTL));
            }
            dl.restack();
            if let Some(tl) = ns.child.get() {
                tl.tl_restack_popups();
            }
            self.state.tree_changed();
        }
    }

    fn toggle_pinned(self: &Rc<Self>) {
        let pl = &mut *self.pinned_link.borrow_mut();
        *pl = if pl.is_some() {
            None
        } else {
            let output = self.workspace.get().node_state[LiveTL].output.get();
            Some(output.pinned.add_last(self.clone()))
        };
        if let Some(tl) = self.node_state[LiveTL].child.get() {
            tl.tl_data().pinned.set(pl.is_some());
        }
        self.set_ns_pinned(pl.is_some());
        self.schedule_render_titles();
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
        let ns = &self.node_state[LiveTL];
        let bw = self.state.theme.sizes.border_width.get(LiveTL);
        let th = self.state.theme.title_height(LiveTL);
        let mut is_icon_press = false;
        if pressed && cursor_data.x >= bw && cursor_data.y >= bw && cursor_data.y < bw + th {
            enum FloatIcon {
                Overlay,
                Pin,
            }
            let mut icons = ArrayVec::<FloatIcon, 2>::new();
            if self.node_state[LiveTL].workspace_ty.get() == WorkspaceType::Overlay {
                icons.push(FloatIcon::Overlay);
            }
            if self.state.show_pin_icon.get() || self.pinned_link.borrow().is_some() {
                icons.push(FloatIcon::Pin);
            }
            let mut x2 = bw + th;
            let icon = 'icon: {
                for icon in icons {
                    if cursor_data.x < x2 {
                        break 'icon Some(icon);
                    }
                    x2 += th;
                }
                None
            };
            if let Some(icon) = icon {
                is_icon_press = true;
                match icon {
                    FloatIcon::Pin => self.toggle_pinned(),
                    FloatIcon::Overlay => {}
                }
            }
        }
        if !cursor_data.op_active {
            if !pressed {
                return;
            }
            if cursor_data.op_type == OpType::Move
                && let Some(tl) = ns.child.get()
            {
                tl.node_do_focus_dyn(seat, Direction::Unspecified);
            }
            if cursor_data.double_click_state.click(
                &self.state,
                time_usec,
                cursor_data.x,
                cursor_data.y,
            ) && cursor_data.op_type == OpType::Move
                && !is_icon_press
                && let Some(tl) = ns.child.get()
            {
                drop(cursors);
                toplevel_set_floating(&self.state, tl, false);
                return;
            }
            cursor_data.op_active = true;
            let pos = ns.position.get();
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
            self.set_workspace_(&ws, true, true);
        }
    }

    pub fn tile_drag_destination(
        self: &Rc<Self>,
        source: NodeId,
        abs_x: i32,
        abs_y: i32,
    ) -> Option<TileDragDestination> {
        let ns = &self.node_state[LiveTL];
        let child = ns.child.get()?;
        let theme = &self.state.theme.sizes;
        let bw = theme.border_width.get(LiveTL);
        let tpuh = self.state.theme.title_plus_underline_height(LiveTL);
        let pos = ns.position.get();
        let body = Rect::new(
            pos.x1() + bw,
            pos.y1() + bw + tpuh,
            pos.x2() - bw,
            pos.y2() - bw,
        )?;
        child.tl_tile_drag_destination(source, None, body, abs_x, abs_y)
    }

    fn set_ns_visible(self: &Rc<Self>, v: bool) -> bool {
        self.add_transaction_op(FloatTransactionOp::SetVisible(v));
        self.node_state[LiveTL].visible.replace(v)
    }

    pub fn set_ns_position(self: &Rc<Self>, v: Rect) {
        self.add_transaction_op(FloatTransactionOp::SetPosition(v));
        self.node_state[LiveTL].position.set(v);
    }

    fn set_ns_child(self: &Rc<Self>, child: Option<&Rc<dyn ToplevelNode>>) {
        self.add_transaction_op(FloatTransactionOp::SetChild(child.cloned()));
        self.node_state[LiveTL].child.set(child.cloned());
    }

    fn set_ns_workspace_type(self: &Rc<Self>, v: WorkspaceType) {
        self.add_transaction_op(FloatTransactionOp::SetWorkspaceType(v));
        self.node_state[LiveTL].workspace_ty.set(v);
    }

    fn set_ns_title_rect(self: &Rc<Self>, v: Rect) {
        self.add_transaction_op(FloatTransactionOp::SetTitleRect(v));
        self.node_state[LiveTL].title_rect.set(v);
    }

    fn set_ns_active(self: &Rc<Self>, v: bool) {
        self.add_transaction_op(FloatTransactionOp::SetActive(v));
        self.node_state[LiveTL].active.set(v);
    }

    fn set_ns_attention_requested(self: &Rc<Self>, v: bool) {
        self.add_transaction_op(FloatTransactionOp::SetAttentionRequested(v));
        self.node_state[LiveTL].attention_requested.set(v);
    }

    fn set_ns_pinned(self: &Rc<Self>, v: bool) {
        self.add_transaction_op(FloatTransactionOp::SetPinned(v));
        self.node_state[LiveTL].pinned.set(v);
    }
}

impl Debug for FloatNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FloatNode").finish_non_exhaustive()
    }
}

impl NodeBase for FloatNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_float(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.node_state[LiveTL].child.get() {
            c.node_visit_dyn(visitor);
        }
    }

    fn node_visible(&self, tl: TreeTimeline) -> bool {
        self.node_state[tl].visible.get()
    }

    fn node_absolute_position(&self, tl: TreeTimeline) -> Rect {
        self.node_state[tl].position.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        Some(self.workspace.get().node_state[LiveTL].output.get())
    }

    fn node_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        Some(self.workspace.get())
    }

    fn node_location(&self) -> Option<NodeLocation> {
        Some(self.location.get())
    }

    fn node_layer(&self) -> NodeLayerLink {
        let Some(l) = self.display_link.borrow().link.as_ref().map(|l| l.to_ref()) else {
            return NodeLayerLink::Display;
        };
        match self.node_state[LiveTL].workspace_ty.get() {
            WorkspaceType::Normal => NodeLayerLink::Stacked(l),
            WorkspaceType::Overlay => NodeLayerLink::OverlayStacked(l),
        }
    }

    fn node_child_title_changed(self: Rc<Self>, _child: &dyn Node, title: &str) {
        self.update_child_title(title);
    }

    fn node_accepts_focus(&self) -> bool {
        if let Some(c) = self.node_state[LiveTL].child.get() {
            return c.tl_accepts_keyboard_focus();
        }
        false
    }

    fn node_do_focus(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if let Some(c) = self.node_state[LiveTL].child.get() {
            c.node_do_focus_dyn(seat, direction);
        }
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        let theme = &self.state.theme;
        let tpuh = theme.title_plus_underline_height(LiveTL);
        let bw = theme.sizes.border_width.get(LiveTL);
        let ns = &self.node_state[LiveTL];
        let pos = ns.position.get();
        if x < bw || x >= pos.width() - bw {
            return FindTreeResult::AcceptsInput;
        }
        if y < bw + tpuh || y >= pos.height() - bw {
            return FindTreeResult::AcceptsInput;
        }
        let child = match ns.child.get() {
            Some(c) => c,
            _ => return FindTreeResult::Other,
        };
        let x = x - bw;
        let y = y - bw - tpuh;
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

    fn node_make_visible(self: &Rc<Self>) {
        if self.node_state[LiveTL].visible.get() {
            return;
        }
        self.workspace.get().cnode_make_visible(&**self);
    }

    fn node_grab_workspace_changed(
        self: Rc<Self>,
        _seat: &Rc<WlSeatGlobal>,
        output: &Rc<OutputNode>,
        ws: Option<&Rc<WorkspaceNode>>,
        _reason: WorkspaceChangeReason,
    ) {
        if ws.map(|ws| ws.id) == Some(self.workspace.get().id) {
            return;
        }
        let ws = ws.cloned().unwrap_or_else(|| output.ensure_workspace());
        self.set_workspace_(&ws, true, false);
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        button: u32,
        state: ButtonState,
        _serial: u64,
    ) {
        if button == BTN_RIGHT && state == ButtonState::Pressed {
            self.toggle_pinned();
        }
        if button != BTN_LEFT {
            return;
        }
        self.button(
            CursorType::Seat(seat.id()),
            seat.pointer_cursor(),
            seat,
            time_usec,
            state == ButtonState::Pressed,
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
        if let Some(changes) = changes
            && let Some(pressed) = changes.down
        {
            self.button(
                CursorType::TabletTool(tool.id),
                tool.cursor(),
                tool.seat(),
                time_usec,
                pressed,
            );
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
        let ns = &self.node_state[LiveTL];
        self.discard_child_properties();
        self.set_ns_child(Some(&new));
        new.tl_set_parent(self.clone());
        new.tl_update_icon(&self.icon);
        self.pull_child_properties();
        new.tl_set_visible(ns.visible.get());
        self.schedule_layout();
    }

    fn cnode_remove_child2(self: Rc<Self>, _child: &dyn Node, _preserve_focus: bool) {
        self.discard_child_properties();
        self.set_ns_visible(false);
        self.set_ns_child(None);
        self.add_transaction_op(FloatTransactionOp::ClearLink);
        self.workspace_link.set(None);
        self.pinned_link.take();
        self.set_ns_pinned(false);
    }

    fn cnode_accepts_child(&self, _node: &dyn Node) -> bool {
        true
    }

    fn cnode_child_attention_request_changed(self: Rc<Self>, _node: &dyn Node, set: bool) {
        if self.node_state[LiveTL].attention_requested.get() != set {
            self.set_ns_attention_requested(set);
            self.workspace
                .get()
                .cnode_child_attention_request_changed(&*self, set);
        }
    }

    fn cnode_workspace(self: Rc<Self>) -> Rc<WorkspaceNode> {
        self.workspace.get()
    }

    fn cnode_make_visible(self: Rc<Self>, _child: &dyn Node) {
        self.node_make_visible();
    }

    fn cnode_set_child_position(self: Rc<Self>, _child: &dyn Node, x: i32, y: i32) {
        let theme = &self.state.theme;
        let tpuh = theme.title_plus_underline_height(LiveTL);
        let bw = theme.sizes.border_width.get(LiveTL);
        let (x, y) = (x - bw, y - tpuh - bw);
        let ns = &self.node_state[LiveTL];
        let pos = ns.position.get();
        if pos.position() != (x, y) {
            let new_pos = pos.at_point(x, y);
            self.set_ns_position(new_pos);
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
        let tpuh = theme.title_plus_underline_height(LiveTL);
        let bw = theme.sizes.border_width.get(LiveTL);
        let ns = &self.node_state[LiveTL];
        let pos = ns.position.get();
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
            y1 = (v - tpuh - bw).min(y2 - bw - tpuh - bw);
        }
        if let Some(v) = new_y2 {
            y2 = (v + bw).max(y1 + bw + tpuh + bw);
        }
        let new_pos = Rect::new_saturating(x1, y1, x2, y2);
        if new_pos != pos {
            self.set_ns_position(new_pos);
            self.schedule_layout();
        }
    }

    fn cnode_pinned(&self) -> bool {
        self.pinned_link.borrow().is_some()
    }

    fn cnode_set_pinned(self: Rc<Self>, pinned: bool) {
        if self.pinned_link.borrow().is_some() == pinned {
            return;
        }
        self.toggle_pinned();
    }

    fn cnode_get_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        Some(self)
    }

    fn cnode_child_icon_changed(self: Rc<Self>, child: &dyn ToplevelNode) {
        child.tl_update_icon(&self.icon);
        self.schedule_render_titles();
    }
}

impl FloatNode {
    fn set_visible(self: &Rc<Self>, visible: bool) {
        let ns = &self.node_state[LiveTL];
        if self.set_ns_visible(visible) != visible {
            self.add_transaction_op(FloatTransactionOp::Damage);
            if visible {
                self.display_link.borrow().invalidate();
            }
        }
        if let Some(child) = ns.child.get() {
            child.tl_set_visible(visible);
        }
        self.seat_state.set_visible(&**self, visible);
    }
}

impl StackedNode for FloatNode {
    fn stacked_set_visible(self: Rc<Self>, visible: bool) {
        self.set_visible(visible);
    }

    fn stacked_has_workspace_link(&self) -> bool {
        true
    }

    fn stacked_validate(self: Rc<Self>, tl: TreeTimeline) {
        if self.node_state[tl].visible.get() {
            self.display_link.borrow_mut().add_last_visible(&self, tl);
        }
    }

    fn stacked_add_stack_op(self: Rc<Self>, op: NodeStackTransactionOp) {
        self.add_transaction_op(FloatTransactionOp::NodeStack(op));
    }
}

impl PinnedNode for FloatNode {
    fn set_workspace(self: Rc<Self>, workspace: &Rc<WorkspaceNode>, update_visible: bool) {
        self.set_workspace_(workspace, false, update_visible);
    }
}

impl dyn Node {
    pub fn node_restack(self: &Rc<Self>) {
        if let Some(tl) = self.clone().node_toplevel()
            && let Some(float) = tl.tl_data().float.get()
        {
            float.restack();
        }
    }
}

pub enum FloatTransactionOp {
    SetVisible(bool),
    SetPosition(Rect),
    SetChild(Option<Rc<dyn ToplevelNode>>),
    SetWorkspaceType(WorkspaceType),
    SetTitleRect(Rect),
    SetActive(bool),
    SetAttentionRequested(bool),
    SetPinned(bool),
    NodeStack(NodeStackTransactionOp),
    Damage,
    ScheduleRenderTitles,
    ClearLink,
}

impl Transactionable for FloatNode {
    type T = FloatTransactionOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        let s = &self.node_state[RenderTL];
        let dmg = |r| self.state.damage(r);
        let dmg_rel = |r: Rect| {
            let (x, y) = s.position.get().position();
            dmg(r.move_(x, y));
        };
        match op {
            FloatTransactionOp::SetVisible(v) => {
                if s.visible.replace(v) != v {
                    dmg(s.position.get());
                }
            }
            FloatTransactionOp::SetPosition(v) => {
                let old = s.position.replace(v);
                if s.visible.get() && old != v {
                    dmg(old);
                    dmg(v);
                }
            }
            FloatTransactionOp::SetChild(v) => {
                s.child.set(v);
                if s.visible.get() {
                    dmg(s.position.get());
                }
            }
            FloatTransactionOp::Damage => {
                dmg(s.position.get());
            }
            FloatTransactionOp::ScheduleRenderTitles => {
                if !self.render_titles_scheduled.replace(true) {
                    self.state.pending_float_titles.push(self.clone());
                }
            }
            FloatTransactionOp::SetWorkspaceType(v) => {
                if s.workspace_ty.replace(v) != v {
                    dmg_rel(s.title_rect.get());
                }
            }
            FloatTransactionOp::SetTitleRect(v) => {
                let old = s.title_rect.replace(v);
                if old != v {
                    dmg_rel(old);
                    dmg_rel(v);
                }
            }
            FloatTransactionOp::SetActive(v) => {
                if s.active.replace(v) != v {
                    dmg(s.position.get());
                }
            }
            FloatTransactionOp::SetAttentionRequested(v) => {
                if s.attention_requested.replace(v) != v {
                    dmg_rel(s.title_rect.get());
                }
            }
            FloatTransactionOp::SetPinned(v) => {
                if s.pinned.replace(v) != v {
                    dmg_rel(s.title_rect.get());
                }
            }
            FloatTransactionOp::NodeStack(v) => {
                self.display_link.borrow_mut().run_op(v);
            }
            FloatTransactionOp::ClearLink => {
                self.display_link.borrow_mut().clear();
            }
        }
    }
}
