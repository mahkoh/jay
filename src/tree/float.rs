use crate::backend::KeyState;
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, SeatId, WlSeatGlobal, BTN_LEFT};
use crate::rect::Rect;
use crate::render::{Renderer, Texture};
use crate::state::State;
use crate::text;
use crate::theme::Color;
use crate::tree::walker::NodeVisitor;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::linkedlist::LinkedNode;
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::mem;
use std::ops::Deref;
use std::rc::Rc;

tree_id!(FloatNodeId);
pub struct FloatNode {
    pub id: FloatNodeId,
    pub state: Rc<State>,
    pub visible: Cell<bool>,
    pub position: Cell<Rect>,
    pub display_link: Cell<Option<LinkedNode<Rc<dyn Node>>>>,
    pub workspace_link: Cell<Option<LinkedNode<Rc<dyn Node>>>>,
    pub workspace: CloneCell<Rc<WorkspaceNode>>,
    pub child: CloneCell<Option<Rc<dyn Node>>>,
    pub active: Cell<bool>,
    pub seat_state: NodeSeatState,
    pub layout_scheduled: Cell<bool>,
    pub render_titles_scheduled: Cell<bool>,
    pub title: RefCell<String>,
    pub title_texture: CloneCell<Option<Rc<Texture>>>,
    seats: RefCell<AHashMap<SeatId, SeatState>>,
}

struct SeatState {
    cursor: KnownCursor,
    target: bool,
    x: i32,
    y: i32,
    op_type: OpType,
    op_active: bool,
    dist_hor: i32,
    dist_ver: i32,
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
            node.render_title();
        }
    }
}

impl FloatNode {
    pub fn new(
        state: &Rc<State>,
        ws: &Rc<WorkspaceNode>,
        position: Rect,
        child: Rc<dyn Node>,
    ) -> Rc<Self> {
        let floater = Rc::new(FloatNode {
            id: state.node_ids.next(),
            state: state.clone(),
            visible: Cell::new(true),
            position: Cell::new(position),
            display_link: Cell::new(None),
            workspace_link: Cell::new(None),
            workspace: CloneCell::new(ws.clone()),
            child: CloneCell::new(Some(child.clone())),
            active: Cell::new(false),
            seat_state: Default::default(),
            layout_scheduled: Cell::new(false),
            render_titles_scheduled: Cell::new(false),
            title: Default::default(),
            title_texture: Default::default(),
            seats: Default::default(),
        });
        floater
            .display_link
            .set(Some(state.root.stacked.add_last(floater.clone())));
        floater
            .workspace_link
            .set(Some(ws.stacked.add_last(floater.clone())));
        child.clone().set_workspace(ws);
        child.clone().set_parent(floater.clone());
        floater.schedule_layout();
        floater
    }

    pub fn on_spaces_changed(self: &Rc<Self>) {
        self.schedule_layout();
    }

    pub fn on_colors_changed(self: &Rc<Self>) {
        self.schedule_render_titles();
    }

    fn schedule_layout(self: &Rc<Self>) {
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
        let bw = theme.border_width.get();
        let th = theme.title_height.get();
        let cpos = Rect::new_sized(
            pos.x1() + bw,
            pos.y1() + bw + th + 1,
            (pos.width() - 2 * bw).max(0),
            (pos.height() - 2 * bw - th - 1).max(0),
        )
        .unwrap();
        child.clone().change_extents(&cpos);
        self.layout_scheduled.set(false);
        self.schedule_render_titles();
    }

    pub fn schedule_render_titles(self: &Rc<Self>) {
        if !self.render_titles_scheduled.replace(true) {
            self.state.pending_float_titles.push(self.clone());
        }
    }

    fn render_title(&self) {
        self.render_titles_scheduled.set(false);
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let bw = theme.border_width.get();
        let font = theme.font.borrow_mut();
        let title = self.title.borrow_mut();
        self.title_texture.set(None);
        let pos = self.position.get();
        if pos.width() <= 2 * bw || th == 0 || title.is_empty() {
            return;
        }
        let ctx = match self.state.render_ctx.get() {
            Some(c) => c,
            _ => return,
        };
        let texture = match text::render(&ctx, pos.width() - 2 * bw, th, &font, &title, Color::GREY)
        {
            Ok(t) => t,
            Err(e) => {
                log::error!("Could not render title {}: {}", title, ErrorFmt(e));
                return;
            }
        };
        self.title_texture.set(Some(texture));
    }

    fn pointer_move(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: i32, y: i32) {
        let theme = &self.state.theme;
        let bw = theme.border_width.get();
        let th = theme.title_height.get();
        let mut seats = self.seats.borrow_mut();
        let seat_state = seats.entry(seat.id()).or_insert_with(|| SeatState {
            cursor: KnownCursor::Default,
            target: false,
            x,
            y,
            op_type: OpType::Move,
            op_active: false,
            dist_hor: 0,
            dist_ver: 0,
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
            self.position.set(Rect::new(x1, y1, x2, y2).unwrap());
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
            OpType::ResizeLeft => KnownCursor::ResizeLeftRight,
            OpType::ResizeTop => KnownCursor::ResizeTopBottom,
            OpType::ResizeRight => KnownCursor::ResizeLeftRight,
            OpType::ResizeBottom => KnownCursor::ResizeTopBottom,
            OpType::ResizeTopLeft => KnownCursor::ResizeTopLeft,
            OpType::ResizeTopRight => KnownCursor::ResizeTopRight,
            OpType::ResizeBottomLeft => KnownCursor::ResizeBottomLeft,
            OpType::ResizeBottomRight => KnownCursor::ResizeBottomRight,
        };
        seat_state.op_type = op_type;
        if new_cursor != mem::replace(&mut seat_state.cursor, new_cursor) {
            if seat_state.target {
                seat.set_known_cursor(new_cursor);
            }
        }
    }
}

impl Debug for FloatNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FloatNode").finish_non_exhaustive()
    }
}

impl Node for FloatNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let _v = self.display_link.take();
        let _v = self.workspace_link.take();
        if let Some(child) = self.child.get() {
            child.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_float(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(c) = self.child.get() {
            c.visit(visitor);
        }
    }

    fn get_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        Some(self.workspace.get())
    }

    fn child_title_changed(self: Rc<Self>, _child: &dyn Node, title: &str) {
        let mut t = self.title.borrow_mut();
        if t.deref() != title {
            t.clear();
            t.push_str(title);
            self.schedule_render_titles();
        }
    }

    fn absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn button(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        if button != BTN_LEFT {
            return;
        }
        let mut seat_datas = self.seats.borrow_mut();
        let seat_data = match seat_datas.get_mut(&seat.id()) {
            Some(s) => s,
            _ => return,
        };
        if !seat_data.op_active {
            if state != KeyState::Pressed {
                return;
            }
            seat_data.op_active = true;
            let pos = self.position.get();
            match seat_data.op_type {
                OpType::Move => {
                    seat_data.dist_hor = seat_data.x;
                    seat_data.dist_ver = seat_data.y;
                }
                OpType::ResizeLeft => seat_data.dist_hor = seat_data.x,
                OpType::ResizeTop => seat_data.dist_ver = seat_data.y,
                OpType::ResizeRight => seat_data.dist_hor = pos.width() - seat_data.x,
                OpType::ResizeBottom => seat_data.dist_ver = pos.height() - seat_data.y,
                OpType::ResizeTopLeft => {
                    seat_data.dist_hor = seat_data.x;
                    seat_data.dist_ver = seat_data.y;
                }
                OpType::ResizeTopRight => {
                    seat_data.dist_hor = pos.width() - seat_data.x;
                    seat_data.dist_ver = seat_data.y;
                }
                OpType::ResizeBottomLeft => {
                    seat_data.dist_hor = seat_data.x;
                    seat_data.dist_ver = pos.height() - seat_data.y;
                }
                OpType::ResizeBottomRight => {
                    seat_data.dist_hor = pos.width() - seat_data.x;
                    seat_data.dist_ver = pos.height() - seat_data.y;
                }
            }
        } else if state == KeyState::Released {
            seat_data.op_active = false;
        }
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let bw = theme.border_width.get();
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
        child.find_tree_at(x, y, tree)
    }

    fn replace_child(self: Rc<Self>, _old: &dyn Node, new: Rc<dyn Node>) {
        self.child.set(Some(new.clone()));
        new.clone().set_parent(self.clone());
        new.clone().set_workspace(&self.workspace.get());
        self.schedule_layout();
    }

    fn remove_child(self: Rc<Self>, _child: &dyn Node) {
        self.child.set(None);
        self.display_link.set(None);
        self.workspace_link.set(None);
    }

    fn child_active_changed(self: Rc<Self>, _child: &dyn Node, active: bool) {
        self.active.set(active);
    }

    fn pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>) {
        let mut seats = self.seats.borrow_mut();
        if let Some(seat_state) = seats.get_mut(&seat.id()) {
            seat_state.target = false;
        }
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        let mut seats = self.seats.borrow_mut();
        if let Some(seat_state) = seats.get_mut(&seat.id()) {
            seat_state.target = true;
            seat.set_known_cursor(seat_state.cursor);
        }
    }

    fn pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_floating(self, x, y)
    }

    fn into_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        Some(self)
    }

    fn accepts_child(&self, _node: &dyn Node) -> bool {
        true
    }

    fn is_float(&self) -> bool {
        true
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        if let Some(c) = self.child.get() {
            c.set_workspace(ws);
        }
        self.workspace_link
            .set(Some(ws.stacked.add_last(self.clone())));
        self.workspace.set(ws.clone());
    }
}
