use crate::backend::{KeyState};
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, SeatId, WlSeatGlobal, BTN_LEFT};
use crate::rect::Rect;
use crate::render::{Renderer, Texture};
use crate::theme::Color;
use crate::tree::walker::NodeVisitor;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::{LinkedList, LinkedNode, NodeRef};
use crate::{text, ErrorFmt, NumCell, State};
use ahash::AHashMap;
use i4config::{Axis, Direction};
use std::cell::{Cell, RefCell};

use std::fmt::{Debug, Formatter};
use std::mem;
use std::ops::{Deref, DerefMut, Sub};
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

pub struct ContainerNode {
    pub id: ContainerNodeId,
    pub parent: CloneCell<Rc<dyn Node>>,
    active: Cell<bool>,
    pub split: Cell<ContainerSplit>,
    title: RefCell<String>,
    pub mono_child: CloneCell<Option<NodeRef<ContainerChild>>>,
    pub mono_body: Cell<Rect>,
    pub mono_content: Cell<Rect>,
    pub abs_x1: Cell<i32>,
    pub abs_y1: Cell<i32>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub content_width: Cell<i32>,
    pub content_height: Cell<i32>,
    pub sum_factors: Cell<f64>,
    layout_scheduled: Cell<bool>,
    render_titles_scheduled: Cell<bool>,
    num_children: NumCell<usize>,
    pub children: LinkedList<ContainerChild>,
    child_nodes: RefCell<AHashMap<NodeId, LinkedNode<ContainerChild>>>,
    seat_state: NodeSeatState,
    workspace: CloneCell<Rc<WorkspaceNode>>,
    seats: RefCell<AHashMap<SeatId, SeatState>>,
    state: Rc<State>,
}

impl Debug for ContainerNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerNode").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct ContainerChild {
    pub node: Rc<dyn Node>,
    pub active: Cell<bool>,
    pub body: Cell<Rect>,
    pub content: Cell<Rect>,
    factor: Cell<f64>,
    pub focus: Cell<ContainerFocus>,
    title: RefCell<String>,
    pub title_texture: CloneCell<Option<Rc<Texture>>>,
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
        state: &Rc<State>,
        workspace: &Rc<WorkspaceNode>,
        parent: Rc<dyn Node>,
        child: Rc<dyn Node>,
        split: ContainerSplit,
    ) -> Self {
        child.clone().set_workspace(workspace);
        let children = LinkedList::new();
        let mut child_nodes = AHashMap::new();
        child_nodes.insert(
            child.id(),
            children.add_last(ContainerChild {
                node: child,
                active: Cell::new(false),
                body: Cell::new(Default::default()),
                content: Cell::new(Default::default()),
                factor: Cell::new(1.0),
                focus: Cell::new(ContainerFocus::None),
                title: Default::default(),
                title_texture: Default::default(),
            }),
        );
        Self {
            id: state.node_ids.next(),
            parent: CloneCell::new(parent),
            active: Cell::new(false),
            split: Cell::new(split),
            title: Default::default(),
            mono_child: CloneCell::new(None),
            mono_body: Cell::new(Default::default()),
            mono_content: Cell::new(Default::default()),
            abs_x1: Cell::new(0),
            abs_y1: Cell::new(0),
            width: Cell::new(0),
            height: Cell::new(0),
            content_width: Cell::new(0),
            content_height: Cell::new(0),
            sum_factors: Cell::new(1.0),
            layout_scheduled: Cell::new(false),
            render_titles_scheduled: Cell::new(false),
            num_children: NumCell::new(1),
            children,
            child_nodes: RefCell::new(child_nodes),
            seat_state: Default::default(),
            workspace: CloneCell::new(workspace.clone()),
            seats: RefCell::new(Default::default()),
            state: state.clone(),
        }
    }

    pub fn num_children(&self) -> usize {
        self.num_children.get()
    }

    pub fn prepend_child(self: &Rc<Self>, new: Rc<dyn Node>) {
        if let Some(child) = self.children.first() {
            self.add_child_before_(&child, new);
            return;
        }
    }

    pub fn append_child(self: &Rc<Self>, new: Rc<dyn Node>) {
        if let Some(child) = self.children.last() {
            self.add_child_after_(&child, new);
            return;
        }
    }

    pub fn add_child_after(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn Node>) {
        self.add_child_x(prev, new, |prev, new| self.add_child_after_(prev, new));
    }

    pub fn add_child_before(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn Node>) {
        self.add_child_x(prev, new, |prev, new| self.add_child_before_(prev, new));
    }

    fn add_child_x<F>(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn Node>, f: F)
    where
        F: FnOnce(&NodeRef<ContainerChild>, Rc<dyn Node>),
    {
        let node = self
            .child_nodes
            .borrow()
            .get(&prev.id())
            .map(|n| n.to_ref());
        if let Some(node) = node {
            f(&node, new);
            return;
        }
        log::error!(
            "Tried to add a child to a container but the preceding node is not in the container"
        );
    }

    fn add_child_after_(self: &Rc<Self>, prev: &NodeRef<ContainerChild>, new: Rc<dyn Node>) {
        self.add_child(|cc| prev.append(cc), new);
    }

    fn add_child_before_(self: &Rc<Self>, prev: &NodeRef<ContainerChild>, new: Rc<dyn Node>) {
        self.add_child(|cc| prev.prepend(cc), new);
    }

    fn add_child<F>(self: &Rc<Self>, f: F, new: Rc<dyn Node>)
    where
        F: FnOnce(ContainerChild) -> LinkedNode<ContainerChild>,
    {
        {
            let mut links = self.child_nodes.borrow_mut();
            if links.contains_key(&new.id()) {
                log::error!("Tried to add a child to a container that already contains the child");
                return;
            }
            links.insert(
                new.id(),
                f(ContainerChild {
                    node: new.clone(),
                    active: Cell::new(false),
                    body: Default::default(),
                    content: Default::default(),
                    factor: Cell::new(0.0),
                    focus: Cell::new(ContainerFocus::None),
                    title: Default::default(),
                    title_texture: Default::default(),
                }),
            );
        }
        new.clone().set_workspace(&self.workspace.get());
        new.clone().set_parent(self.clone());
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
        self.sum_factors.set(sum_factors);
        self.schedule_layout();
        self.cancel_seat_ops();
    }

    fn cancel_seat_ops(&self) {
        let mut seats = self.seats.borrow_mut();
        for seat in seats.values_mut() {
            seat.op = None;
        }
    }

    pub fn on_theme_changed(self: &Rc<Self>) {
        self.update_content_size();
        self.schedule_layout();
    }

    fn schedule_layout(self: &Rc<Self>) {
        if !self.layout_scheduled.replace(true) {
            self.state.pending_container_layout.push(self.clone());
        }
    }

    fn perform_layout(self: &Rc<Self>) {
        let sum_factors = self.sum_factors.get();
        let border_width = self.state.theme.border_width.get();
        let title_height = self.state.theme.title_height.get();
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
                    (pos, title_height + 1, body_size, other_content_size)
                }
                _ => (0, pos + title_height + 1, other_content_size, body_size),
            };
            let body = Rect::new_sized(x1, y1, width, height).unwrap();
            child.body.set(body);
            pos += body_size + border_width;
            if split == ContainerSplit::Vertical {
                pos += title_height + 1;
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
                        (pos, title_height + 1, width, other_content_size, width)
                    }
                    _ => {
                        let height = body.height() + add;
                        (0, pos, other_content_size, height, height)
                    }
                };
                body = Rect::new_sized(x1, y1, width, height).unwrap();
                child.body.set(body);
                pos += size + border_width;
                if split == ContainerSplit::Vertical {
                    pos += title_height + 1;
                }
            }
        }
        self.sum_factors.set(1.0);
        self.layout_scheduled.set(false);
        for child in self.children.iter() {
            let body = child.body.get().move_(self.abs_x1.get(), self.abs_y1.get());
            child.node.clone().change_extents(&body);
            child.position_content();
        }
        self.schedule_render_titles();
    }

    fn update_content_size(&self) {
        let border_width = self.state.theme.border_width.get();
        let title_height = self.state.theme.title_height.get();
        let nc = self.num_children.get();
        match self.split.get() {
            ContainerSplit::Horizontal => {
                let new_content_size = self.width.get().sub((nc - 1) as i32 * border_width).max(0);
                self.content_width.set(new_content_size);
                self.content_height
                    .set(self.height.get().sub(title_height + 1).max(0));
            }
            ContainerSplit::Vertical => {
                let new_content_size = self
                    .height
                    .get()
                    .sub(title_height + 1 + (nc - 1) as i32 * (border_width + title_height + 1))
                    .max(0);
                self.content_height.set(new_content_size);
                self.content_width.set(self.width.get());
            }
        }
    }

    fn pointer_move(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, mut x: i32, mut y: i32) {
        let title_height = self.state.theme.title_height.get();
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
                            x = x
                                .max(prev_body.x1() + dist_left)
                                .min(child_body.x2() - dist_right);
                            let prev_factor = (x - prev_body.x1() - dist_left) as f64 / cw as f64;
                            let child_factor =
                                (child_body.x2() - x - dist_right) as f64 / cw as f64;
                            (prev_factor, child_factor)
                        }
                        ContainerSplit::Vertical => {
                            let ch = self.content_height.get();
                            y = y
                                .max(prev_body.y1() + dist_left)
                                .min(child_body.y2() - dist_right);
                            let prev_factor = (y - prev_body.y1() - dist_left) as f64 / ch as f64;
                            let child_factor =
                                (child_body.y2() - y - dist_right) as f64 / ch as f64;
                            (prev_factor, child_factor)
                        }
                    };
                    let sum_factors =
                        self.sum_factors.get() - prev.factor.get() - op.child.factor.get()
                            + prev_factor
                            + child_factor;
                    prev.factor.set(prev_factor);
                    op.child.factor.set(child_factor);
                    self.sum_factors.set(sum_factors);
                    self.schedule_layout();
                }
            }
            return;
        }
        let new_cursor = if self.mono_child.get().is_some() {
            KnownCursor::Default
        } else if self.split.get() == ContainerSplit::Horizontal {
            if y < title_height + 1 {
                KnownCursor::Default
            } else {
                KnownCursor::ResizeLeftRight
            }
        } else {
            let mut cursor = KnownCursor::Default;
            for child in self.children.iter() {
                let body = child.body.get();
                if body.y1() > y {
                    if body.y1() - y > title_height + 1 {
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

    fn update_title(self: &Rc<Self>) {
        let split = match self.split.get() {
            ContainerSplit::Horizontal => "H",
            ContainerSplit::Vertical => "V",
        };
        let mut title = self.title.borrow_mut();
        title.clear();
        title.push_str(split);
        title.push_str("[");
        for (i, c) in self.children.iter().enumerate() {
            if i > 0 {
                title.push_str(", ");
            }
            title.push_str(c.title.borrow_mut().deref());
        }
        title.push_str("]");
        self.parent.get().child_title_changed(&**self, &title);
        self.schedule_render_titles();
    }

    fn schedule_render_titles(self: &Rc<Self>) {
        if !self.render_titles_scheduled.replace(true) {
            self.state.pending_container_titles.push(self.clone());
        }
    }

    fn render_titles(&self) {
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let font = theme.font.borrow_mut();
        for c in self.children.iter() {
            c.title_texture.set(None);
            let title = c.title.borrow_mut();
            let body = c.body.get();
            if title.is_empty() || th == 0 || body.width() == 0 {
                continue;
            }
            let ctx = match self.state.render_ctx.get() {
                Some(c) => c,
                _ => continue,
            };
            let texture = match text::render(&ctx, body.width(), th, &font, &title, Color::GREY) {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Could not render title {}: {}", title, ErrorFmt(e));
                    continue;
                }
            };
            c.title_texture.set(Some(texture));
        }
        self.render_titles_scheduled.set(false);
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

pub async fn container_layout(state: Rc<State>) {
    loop {
        let container = state.pending_container_layout.pop().await;
        if container.layout_scheduled.get() {
            container.perform_layout();
        }
    }
}

pub async fn render_titles(state: Rc<State>) {
    loop {
        let container = state.pending_container_titles.pop().await;
        if container.render_titles_scheduled.get() {
            container.render_titles();
        }
    }
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

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_container(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        for child in self.children.iter() {
            child.node.clone().visit(visitor);
        }
    }

    fn child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        let child = match self.child_nodes.borrow_mut().get(&child.id()) {
            Some(cn) => cn.to_ref(),
            _ => return,
        };
        {
            let mut ct = child.title.borrow_mut();
            if ct.deref() == title {
                return;
            }
            ct.clear();
            ct.push_str(title);
        }
        self.update_title();
    }

    fn get_split(&self) -> Option<ContainerSplit> {
        Some(self.split.get())
    }

    fn set_split(self: Rc<Self>, split: ContainerSplit) {
        if self.split.replace(split) != split {
            self.update_content_size();
            self.schedule_layout();
            self.update_title();
        }
    }

    fn focus_self(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        seat.focus_node(self);
    }

    fn move_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if direction == Direction::Down {
            self.do_focus(seat, direction);
            return;
        }
        self.parent
            .get()
            .move_focus_from_child(seat, &*self, direction);
    }

    fn move_self(self: Rc<Self>, direction: Direction) {
        self.parent.get().move_child(self, direction);
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
        let child = match self.child_nodes.borrow_mut().get(&child.id()) {
            Some(c) => c.to_ref(),
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
    //
    fn move_child(self: Rc<Self>, child: Rc<dyn Node>, direction: Direction) {
        // CASE 1: This is the only child of the container. Replace the container by the child.
        if self.num_children.get() == 1 {
            let parent = self.parent.get();
            if parent.accepts_child(&*child) {
                parent.replace_child(&*self, child.clone());
            }
            return;
        }
        let (split, prev) = direction_to_split(direction);
        // CASE 2: We're moving the child within the container.
        if split == self.split.get() {
            let cc = match self.child_nodes.borrow_mut().get(&child.id()) {
                Some(l) => l.to_ref(),
                None => return,
            };
            let neighbor = match prev {
                true => cc.prev(),
                false => cc.next(),
            };
            if let Some(neighbor) = neighbor {
                if neighbor.node.accepts_child(&*child) {
                    self.remove_child(&*child);
                    neighbor.node.clone().insert_child(child, direction);
                    return;
                }
                let cc = cc.deref().clone();
                let link = match prev {
                    true => neighbor.prepend(cc),
                    false => neighbor.append(cc),
                };
                self.child_nodes.borrow_mut().insert(child.id(), link);
                self.schedule_layout();
                return;
            }
        }
        // CASE 3: We're moving the child out of the container.
        let mut neighbor = self.clone();
        let mut parent_opt = self.parent.get().into_container();
        while let Some(parent) = &parent_opt {
            if parent.split.get() == split {
                break;
            }
            neighbor = parent.clone();
            parent_opt = parent.parent.get().into_container();
        }
        let parent = match parent_opt {
            Some(p) => p,
            _ => return,
        };
        self.clone().remove_child(&*child);
        match prev {
            true => parent.add_child_before(&*neighbor, child.clone()),
            false => parent.add_child_after(&*neighbor, child.clone()),
        }
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

    fn active_changed(&self, active: bool) {
        self.active.set(active);
        self.parent.get().child_active_changed(self, active);
    }

    fn button(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        if button != BTN_LEFT {
            return;
        }
        let title_height = self.state.theme.title_height.get();
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
                            let op = if seat_data.y < body.y1() - title_height - 1 {
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

    fn focus_parent(&self, seat: &Rc<WlSeatGlobal>) {
        self.parent.get().focus_self(seat);
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

    fn replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn Node>) {
        let node = match self.child_nodes.borrow_mut().remove(&old.id()) {
            Some(c) => c,
            None => return,
        };
        let link = node.append(ContainerChild {
            node: new.clone(),
            active: Cell::new(false),
            body: Cell::new(node.body.get()),
            content: Cell::new(node.content.get()),
            factor: Cell::new(node.factor.get()),
            focus: Cell::new(node.focus.get()),
            title: Default::default(),
            title_texture: Default::default(),
        });
        let body = link.body.get();
        drop(node);
        self.child_nodes.borrow_mut().insert(new.id(), link);
        new.clone().set_parent(self.clone());
        new.clone().set_workspace(&self.workspace.get());
        let body = body.move_(self.abs_x1.get(), self.abs_y1.get());
        new.clone().change_extents(&body);
    }

    fn remove_child(self: Rc<Self>, child: &dyn Node) {
        let node = match self.child_nodes.borrow_mut().remove(&child.id()) {
            Some(c) => c.to_ref(),
            None => return,
        };
        let num_children = self.num_children.fetch_sub(1) - 1;
        if num_children == 0 {
            self.seats.borrow_mut().clear();
            self.parent.get().remove_child(&*self);
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
        self.sum_factors.set(sum);
        self.update_title();
        self.schedule_layout();
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

    fn child_active_changed(&self, child: &dyn Node, active: bool) {
        let node = match self.child_nodes.borrow_mut().get(&child.id()) {
            Some(l) => l.to_ref(),
            None => return,
        };
        node.active.set(active);
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

    fn motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_container(self, x, y);
    }

    fn into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>> {
        Some(self)
    }

    fn is_container(&self) -> bool {
        true
    }

    fn accepts_child(&self, _node: &dyn Node) -> bool {
        true
    }

    fn insert_child(self: Rc<Self>, node: Rc<dyn Node>, direction: Direction) {
        let (split, right) = direction_to_split(direction);
        if split != self.split.get() || right {
            self.append_child(node);
        } else {
            self.prepend_child(node);
        }
    }

    fn change_extents(self: Rc<Self>, rect: &Rect) {
        self.abs_x1.set(rect.x1());
        self.abs_y1.set(rect.y1());
        let mut size_changed = false;
        size_changed |= self.width.replace(rect.width()) != rect.width();
        size_changed |= self.height.replace(rect.height()) != rect.height();
        if size_changed {
            self.update_content_size();
            self.perform_layout();
            self.cancel_seat_ops();
            self.parent
                .get()
                .child_size_changed(&*self, rect.width(), rect.height());
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

    fn set_parent(self: Rc<Self>, parent: Rc<dyn Node>) {
        self.parent.set(parent.clone());
        parent.child_active_changed(&*self, self.active.get());
        parent.child_size_changed(&*self, self.width.get(), self.height.get());
        parent
            .clone()
            .child_title_changed(&*self, self.title.borrow_mut().deref());
    }
}

fn direction_to_split(dir: Direction) -> (ContainerSplit, bool) {
    match dir {
        Direction::Left => (ContainerSplit::Horizontal, true),
        Direction::Down => (ContainerSplit::Vertical, false),
        Direction::Up => (ContainerSplit::Vertical, true),
        Direction::Right => (ContainerSplit::Horizontal, false),
    }
}
