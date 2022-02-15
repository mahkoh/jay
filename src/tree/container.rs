use crate::backend::KeyState;
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, SeatId, WlSeatGlobal, BTN_LEFT};
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::{LinkedList, LinkedNode, NodeRef};
use crate::{NumCell, State};
use ahash::AHashMap;
use i4config::{Axis, Direction};
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::mem;
use std::ops::DerefMut;
use std::rc::Rc;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContainerSplit {
    Horizontal,
    Vertical,
}

impl From<Axis> for ContainerSplit {
    fn from(a: Axis) -> Self {
        match a {
            Axis::Horizontal => Self::Horizontal,
            Axis::Vertical => Self::Vertical,
        }
    }
}

impl Into<Axis> for ContainerSplit {
    fn into(self) -> Axis {
        match self {
            ContainerSplit::Horizontal => Axis::Horizontal,
            ContainerSplit::Vertical => Axis::Vertical,
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContainerFocus {
    None,
    Child,
    Yes,
}

tree_id!(ContainerNodeId);

pub const CONTAINER_TITLE_HEIGHT: i32 = 18;
pub const CONTAINER_BORDER: i32 = 4;

pub struct ContainerNode {
    pub id: ContainerNodeId,
    pub parent: CloneCell<Rc<dyn Node>>,
    pub split: Cell<ContainerSplit>,
    pub mono_child: CloneCell<Option<NodeRef<ContainerChild>>>,
    pub mono_body: Cell<Rect>,
    pub mono_content: Cell<Rect>,
    pub abs_x1: Cell<i32>,
    pub abs_y1: Cell<i32>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub content_width: Cell<i32>,
    pub content_height: Cell<i32>,
    num_children: NumCell<usize>,
    pub children: LinkedList<ContainerChild>,
    child_nodes: RefCell<AHashMap<NodeId, LinkedNode<ContainerChild>>>,
    seat_state: NodeSeatState,
    workspace: CloneCell<Rc<WorkspaceNode>>,
    seats: RefCell<AHashMap<SeatId, SeatState>>,
}

impl Debug for ContainerNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerNode").finish_non_exhaustive()
    }
}

pub struct ContainerChild {
    pub node: Rc<dyn Node>,
    pub body: Cell<Rect>,
    pub content: Cell<Rect>,
    factor: Cell<f64>,
    pub focus: Cell<ContainerFocus>,
}

struct SeatState {
    cursor: KnownCursor,
    target: bool,
    x: i32,
    y: i32,
    op: Option<SeatOp>,
}

impl ContainerChild {
    fn position_content(&self) {
        let mut content = self.content.get();
        let body = self.body.get();
        let width = content.width();
        let height = content.height();
        // let x1 = body.x1() + (body.width() - width) / 2;
        // let y1 = body.y1() + (body.height() - height) / 2;
        let x1 = body.x1();
        let y1 = body.y1();
        content = Rect::new_sized(x1, y1, width, height).unwrap();
        // log::debug!("body: {:?}", body);
        // log::debug!("content: {:?}", content);
        self.content.set(content);
    }
}

impl ContainerNode {
    pub fn new(
        state: &State,
        workspace: &Rc<WorkspaceNode>,
        parent: Rc<dyn Node>,
        child: Rc<dyn Node>,
    ) -> Self {
        child.clone().set_workspace(workspace);
        let children = LinkedList::new();
        let mut child_nodes = AHashMap::new();
        child_nodes.insert(
            child.id(),
            children.add_last(ContainerChild {
                node: child,
                body: Cell::new(Default::default()),
                content: Cell::new(Default::default()),
                factor: Cell::new(1.0),
                focus: Cell::new(ContainerFocus::None),
            }),
        );
        Self {
            id: state.node_ids.next(),
            parent: CloneCell::new(parent),
            split: Cell::new(ContainerSplit::Horizontal),
            mono_child: CloneCell::new(None),
            mono_body: Cell::new(Default::default()),
            mono_content: Cell::new(Default::default()),
            abs_x1: Cell::new(0),
            abs_y1: Cell::new(0),
            width: Cell::new(0),
            height: Cell::new(0),
            content_width: Cell::new(0),
            content_height: Cell::new(0),
            num_children: NumCell::new(1),
            children,
            child_nodes: RefCell::new(child_nodes),
            seat_state: Default::default(),
            workspace: CloneCell::new(workspace.clone()),
            seats: RefCell::new(Default::default()),
        }
    }

    pub fn num_children(&self) -> usize {
        self.num_children.get()
    }

    pub fn append_child(self: &Rc<Self>, new: Rc<dyn Node>) {
        if let Some(child) = self.children.last() {
            self.add_child_after_(&child, new);
            return;
        }
        log::error!("Tried to add a child to a container but container is empty");
    }

    pub fn add_child_after(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn Node>) {
        let node = self
            .child_nodes
            .borrow()
            .get(&prev.id())
            .map(|n| n.to_ref());
        if let Some(node) = node {
            self.add_child_after_(&node, new);
            return;
        }
        log::error!(
            "Tried to add a child to a container but the preceding node is not in the container"
        );
    }

    fn add_child_after_(self: &Rc<Self>, prev: &NodeRef<ContainerChild>, new: Rc<dyn Node>) {
        {
            let mut links = self.child_nodes.borrow_mut();
            if links.contains_key(&new.id()) {
                log::error!("Tried to add a child to a container that already contains the child");
                return;
            }
            links.insert(
                new.id(),
                prev.append(ContainerChild {
                    node: new.clone(),
                    body: Default::default(),
                    content: Default::default(),
                    factor: Cell::new(0.0),
                    focus: Cell::new(ContainerFocus::None),
                }),
            );
        }
        new.clone().set_workspace(&self.workspace.get());
        let num_children = self.num_children.fetch_add(1) + 1;
        self.update_content_size();
        let new_child_factor = 1.0 / num_children as f64;
        let mut sum_factors = 0.0;
        for child in self.children.iter() {
            let factor = if Rc::ptr_eq(&child.node, &new) {
                new_child_factor
            } else {
                child.factor.get() * (1.0 - new_child_factor)
            };
            child.factor.set(factor);
            sum_factors += factor;
        }
        self.apply_factors(sum_factors);
        self.cancel_seat_ops();
    }

    fn cancel_seat_ops(&self) {
        let mut seats = self.seats.borrow_mut();
        for seat in seats.values_mut() {
            seat.op = None;
        }
    }

    fn apply_factors(&self, sum_factors: f64) {
        let split = self.split.get();
        let (content_size, other_content_size) = match split {
            ContainerSplit::Horizontal => (self.content_width.get(), self.content_height.get()),
            ContainerSplit::Vertical => (self.content_height.get(), self.content_width.get()),
        };
        let num_children = self.num_children.get();
        let mut pos = 0;
        let mut remaining_content_size = content_size;
        for child in self.children.iter() {
            let factor = child.factor.get() / sum_factors;
            child.factor.set(factor);
            let mut body_size = (content_size as f64 * factor).round() as i32;
            body_size = body_size.min(remaining_content_size);
            remaining_content_size -= body_size;
            let (x1, y1, width, height) = match split {
                ContainerSplit::Horizontal => {
                    (pos, CONTAINER_TITLE_HEIGHT, body_size, other_content_size)
                }
                _ => (
                    0,
                    pos + CONTAINER_TITLE_HEIGHT,
                    other_content_size,
                    body_size,
                ),
            };
            let body = Rect::new_sized(x1, y1, width, height).unwrap();
            child.body.set(body);
            pos += body_size + CONTAINER_BORDER;
            if split == ContainerSplit::Vertical {
                pos += CONTAINER_TITLE_HEIGHT;
            }
        }
        if remaining_content_size > 0 {
            let size_per = remaining_content_size / num_children as i32;
            let mut rem = remaining_content_size % num_children as i32;
            pos = 0;
            for child in self.children.iter() {
                let mut body = child.body.get();
                let mut add = size_per;
                if rem > 0 {
                    rem -= 1;
                    add += 1;
                }
                let (x1, y1, width, height, size) = match split {
                    ContainerSplit::Horizontal => {
                        let width = body.width() + add;
                        (
                            pos,
                            CONTAINER_TITLE_HEIGHT,
                            width,
                            other_content_size,
                            width,
                        )
                    }
                    _ => {
                        let height = body.height() + add;
                        (0, pos, other_content_size, height, height)
                    }
                };
                body = Rect::new_sized(x1, y1, width, height).unwrap();
                child.body.set(body);
                pos += size + CONTAINER_BORDER;
                if split == ContainerSplit::Vertical {
                    pos += CONTAINER_TITLE_HEIGHT;
                }
            }
        }
        for child in self.children.iter() {
            let body = child.body.get().move_(self.abs_x1.get(), self.abs_y1.get());
            child.node.clone().change_extents(&body);
            child.position_content();
        }
    }

    fn update_content_size(&self) {
        let nc = self.num_children.get();
        match self.split.get() {
            ContainerSplit::Horizontal => {
                let new_content_size = self
                    .width
                    .get()
                    .saturating_sub((nc - 1) as i32 * CONTAINER_BORDER);
                self.content_width.set(new_content_size);
                self.content_height
                    .set(self.height.get().saturating_sub(CONTAINER_TITLE_HEIGHT));
            }
            ContainerSplit::Vertical => {
                let new_content_size = self.height.get().saturating_sub(
                    CONTAINER_TITLE_HEIGHT
                        + (nc - 1) as i32 * (CONTAINER_BORDER + CONTAINER_TITLE_HEIGHT),
                );
                self.content_height.set(new_content_size);
                self.content_width.set(self.width.get());
            }
        }
    }

    fn pointer_move(&self, seat: &Rc<WlSeatGlobal>, x: i32, y: i32) {
        let mut seats = self.seats.borrow_mut();
        let seat_state = seats.entry(seat.id()).or_insert_with(|| SeatState {
            cursor: KnownCursor::Default,
            target: false,
            x,
            y,
            op: None,
        });
        seat_state.x = x;
        seat_state.y = y;
        if let Some(op) = &seat_state.op {
            match op.kind {
                SeatOpKind::Move => {
                    // todo
                }
                SeatOpKind::Resize {
                    dist_left,
                    dist_right,
                } => {
                    let prev = op.child.prev().unwrap();
                    let prev_body = prev.body.get();
                    let child_body = op.child.body.get();
                    let (prev_factor, child_factor) = match self.split.get() {
                        ContainerSplit::Horizontal => {
                            let cw = self.content_width.get();
                            if prev_body.x1() + dist_left > x || x + dist_right > child_body.x2() {
                                return;
                            }
                            let prev_factor = (x - prev_body.x1() - dist_left) as f64 / cw as f64;
                            let child_factor =
                                (child_body.x2() - x - dist_right) as f64 / cw as f64;
                            (prev_factor, child_factor)
                        }
                        ContainerSplit::Vertical => {
                            let ch = self.content_height.get();
                            if prev_body.y1() + dist_left > y || y + dist_right > child_body.y2() {
                                return;
                            }
                            let prev_factor = (y - prev_body.y1() - dist_left) as f64 / ch as f64;
                            let child_factor =
                                (child_body.y2() - y - dist_right) as f64 / ch as f64;
                            (prev_factor, child_factor)
                        }
                    };
                    let sum_factors = 1.0 - prev.factor.get() - op.child.factor.get()
                        + prev_factor
                        + child_factor;
                    prev.factor.set(prev_factor);
                    op.child.factor.set(child_factor);
                    self.apply_factors(sum_factors);
                }
            }
            return;
        }
        let new_cursor = if self.mono_child.get().is_some() {
            KnownCursor::Default
        } else if self.split.get() == ContainerSplit::Horizontal {
            if y < CONTAINER_TITLE_HEIGHT {
                KnownCursor::Default
            } else {
                KnownCursor::ResizeLeftRight
            }
        } else {
            let mut cursor = KnownCursor::Default;
            for child in self.children.iter() {
                let body = child.body.get();
                if body.y1() > y {
                    if body.y1() - y > CONTAINER_TITLE_HEIGHT {
                        cursor = KnownCursor::ResizeTopBottom
                    }
                    break;
                }
            }
            cursor
        };
        if new_cursor != mem::replace(&mut seat_state.cursor, new_cursor) {
            if seat_state.target {
                seat.set_known_cursor(new_cursor);
            }
        }
    }
}

struct SeatOp {
    child: NodeRef<ContainerChild>,
    kind: SeatOpKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum SeatOpKind {
    Move,
    Resize { dist_left: i32, dist_right: i32 },
}

impl Node for ContainerNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.parent.get().remove_child(self);
        }
        mem::take(self.seats.borrow_mut().deref_mut());
        let mut cn = self.child_nodes.borrow_mut();
        for (_, n) in cn.drain() {
            n.node.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn get_split(&self) -> Option<ContainerSplit> {
        Some(self.split.get())
    }

    fn set_split(&self, split: ContainerSplit) {
        self.split.set(split);
        self.update_content_size();
        self.apply_factors(1.0);
    }

    fn do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let node = match direction {
            Direction::Left => self.children.last(),
            Direction::Down => self.children.first(),
            Direction::Up => self.children.last(),
            Direction::Right => self.children.first(),
        };
        if let Some(node) = node {
            node.node.clone().do_focus(seat, direction);
        }
    }

    fn move_focus_from_child(
        &self,
        seat: &Rc<WlSeatGlobal>,
        child: &dyn Node,
        direction: Direction,
    ) {
        let children = self.child_nodes.borrow_mut();
        let child = match children.get(&child.id()) {
            Some(c) => c,
            _ => return,
        };
        let in_line = match self.split.get() {
            ContainerSplit::Horizontal => matches!(direction, Direction::Left | Direction::Right),
            ContainerSplit::Vertical => matches!(direction, Direction::Up | Direction::Down),
        };
        if !in_line {
            self.parent
                .get()
                .move_focus_from_child(seat, self, direction);
            return;
        }
        let prev = match direction {
            Direction::Left => true,
            Direction::Down => false,
            Direction::Up => true,
            Direction::Right => false,
        };
        let sibling = match prev {
            true => child.prev(),
            false => child.next(),
        };
        let sibling = match sibling {
            Some(s) => s,
            None => {
                self.parent
                    .get()
                    .move_focus_from_child(seat, self, direction);
                return;
            }
        };
        sibling.node.clone().do_focus(seat, direction);
    }

    fn absolute_position(&self) -> Rect {
        Rect::new_sized(
            self.abs_x1.get(),
            self.abs_y1.get(),
            self.width.get(),
            self.height.get(),
        )
        .unwrap()
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
        if seat_data.op.is_none() {
            if state != KeyState::Pressed {
                return;
            }
            let (kind, child) = 'res: {
                if self.mono_child.get().is_some() {
                    let width_per_child = self.width.get() / self.num_children.get() as i32;
                    let mut width_per_child_rem = self.width.get() % self.num_children.get() as i32;
                    let mut pos = 0;
                    for child in self.children.iter() {
                        pos += width_per_child;
                        if width_per_child_rem > 0 {
                            pos += 1;
                            width_per_child_rem -= 1;
                        }
                        if pos > seat_data.x {
                            break 'res (SeatOpKind::Move, child);
                        }
                    }
                } else if self.split.get() == ContainerSplit::Horizontal {
                    for child in self.children.iter() {
                        let body = child.body.get();
                        if seat_data.x < body.x2() {
                            let op = if seat_data.x < body.x1() {
                                SeatOpKind::Resize {
                                    dist_left: seat_data.x - child.prev().unwrap().body.get().x2(),
                                    dist_right: body.x1() - seat_data.x,
                                }
                            } else {
                                SeatOpKind::Move
                            };
                            break 'res (op, child);
                        }
                    }
                } else {
                    for child in self.children.iter() {
                        let body = child.body.get();
                        if seat_data.y < body.y1() {
                            let op = if seat_data.y < body.y1() - CONTAINER_TITLE_HEIGHT {
                                SeatOpKind::Resize {
                                    dist_left: seat_data.y - child.prev().unwrap().body.get().y2(),
                                    dist_right: body.y1() - seat_data.y,
                                }
                            } else {
                                SeatOpKind::Move
                            };
                            break 'res (op, child);
                        }
                    }
                };
                return;
            };
            seat_data.op = Some(SeatOp { child, kind })
        } else if state == KeyState::Released {
            let op = seat_data.op.take().unwrap();
            drop(seat_datas);
            if op.kind == SeatOpKind::Move {
                // todo
            }
        }
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let mut recurse = |content: Rect, child: NodeRef<ContainerChild>| {
            if content.contains(x, y) {
                let (x, y) = content.translate(x, y);
                tree.push(FoundNode {
                    node: child.node.clone(),
                    x,
                    y,
                });
                child.node.find_tree_at(x, y, tree);
            }
        };
        if let Some(child) = self.mono_child.get() {
            recurse(self.mono_content.get(), child);
        } else {
            for child in self.children.iter() {
                if child.body.get().contains(x, y) {
                    recurse(child.content.get(), child);
                    break;
                }
            }
        }
        FindTreeResult::AcceptsInput
    }

    fn remove_child(&self, child: &dyn Node) {
        let node = match self.child_nodes.borrow_mut().remove(&child.id()) {
            Some(c) => c.to_ref(),
            None => return,
        };
        let num_children = self.num_children.fetch_sub(1) - 1;
        if num_children == 0 {
            self.seats.borrow_mut().clear();
            self.parent.get().remove_child(self);
            return;
        }
        self.update_content_size();
        let rem = 1.0 - node.factor.get();
        let mut sum = 0.0;
        if rem <= 0.0 {
            let factor = 1.0 / num_children as f64;
            for child in self.children.iter() {
                child.factor.set(factor)
            }
            sum = 1.0;
        } else {
            for child in self.children.iter() {
                let factor = child.factor.get() / rem;
                child.factor.set(factor);
                sum += factor;
            }
        }
        self.apply_factors(sum);
        self.cancel_seat_ops();
    }

    fn child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        let cn = self.child_nodes.borrow();
        if let Some(node) = cn.get(&child.id()) {
            let rect = Rect::new(0, 0, width, height).unwrap();
            node.content.set(rect);
            node.position_content();
        }
    }

    fn enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn pointer_untarget(&self, seat: &Rc<WlSeatGlobal>) {
        let mut seats = self.seats.borrow_mut();
        if let Some(seat_state) = seats.get_mut(&seat.id()) {
            seat_state.target = false;
        }
    }

    fn pointer_target(&self, seat: &Rc<WlSeatGlobal>) {
        let mut seats = self.seats.borrow_mut();
        if let Some(seat_state) = seats.get_mut(&seat.id()) {
            seat_state.target = true;
            seat.set_known_cursor(seat_state.cursor);
        }
    }

    fn motion(&self, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_container(self, x, y);
    }

    fn into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>> {
        Some(self)
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        self.abs_x1.set(rect.x1());
        self.abs_y1.set(rect.y1());
        let mut size_changed = false;
        size_changed |= self.width.replace(rect.width()) != rect.width();
        size_changed |= self.height.replace(rect.height()) != rect.height();
        if size_changed {
            self.update_content_size();
            self.apply_factors(1.0);
            self.cancel_seat_ops();
        } else {
            for child in self.children.iter() {
                let body = child.body.get().move_(self.abs_x1.get(), self.abs_y1.get());
                child.node.clone().change_extents(&body);
            }
        }
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        for child in self.children.iter() {
            child.node.clone().set_workspace(ws);
        }
        self.workspace.set(ws.clone());
    }
}
