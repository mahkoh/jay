use {
    crate::{
        backend::KeyState,
        cursor::KnownCursor,
        cursor_user::CursorUser,
        fixed::Fixed,
        ifs::wl_seat::{
            collect_kb_foci, collect_kb_foci2,
            tablet::{TabletTool, TabletToolChanges, TabletToolId},
            wl_pointer::PendingScroll,
            NodeSeatState, SeatId, WlSeatGlobal, BTN_LEFT,
        },
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        state::State,
        text::{self, TextTexture},
        tree::{
            walker::NodeVisitor, ContainingNode, Direction, FindTreeResult, FindTreeUsecase,
            FoundNode, Node, NodeId, ToplevelData, ToplevelNode, ToplevelNodeBase, WorkspaceNode,
        },
        utils::{
            clonecell::CloneCell,
            double_click_state::DoubleClickState,
            errorfmt::ErrorFmt,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
            rc_eq::rc_eq,
            scroller::Scroller,
            smallmap::{SmallMap, SmallMapMut},
            threshold_counter::ThresholdCounter,
        },
    },
    ahash::AHashMap,
    jay_config::Axis,
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        ops::{Deref, DerefMut, Sub},
        rc::Rc,
    },
};

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

#[expect(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContainerFocus {
    None,
    Child,
    Yes,
}

tree_id!(ContainerNodeId);

pub struct ContainerTitle {
    pub x: i32,
    pub y: i32,
    pub tex: TextTexture,
}

#[derive(Default)]
pub struct ContainerRenderData {
    pub title_rects: Vec<Rect>,
    pub active_title_rects: Vec<Rect>,
    pub attention_title_rects: Vec<Rect>,
    pub last_active_rect: Option<Rect>,
    pub border_rects: Vec<Rect>,
    pub underline_rects: Vec<Rect>,
    pub titles: SmallMapMut<Scale, Vec<ContainerTitle>, 2>,
}

pub struct ContainerNode {
    pub id: ContainerNodeId,
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
    pub sum_factors: Cell<f64>,
    layout_scheduled: Cell<bool>,
    compute_render_data_scheduled: Cell<bool>,
    num_children: NumCell<usize>,
    pub children: LinkedList<ContainerChild>,
    focus_history: LinkedList<NodeRef<ContainerChild>>,
    child_nodes: RefCell<AHashMap<NodeId, LinkedNode<ContainerChild>>>,
    workspace: CloneCell<Rc<WorkspaceNode>>,
    cursors: RefCell<AHashMap<CursorType, CursorState>>,
    state: Rc<State>,
    pub render_data: RefCell<ContainerRenderData>,
    scroller: Scroller,
    toplevel_data: ToplevelData,
    attention_requests: ThresholdCounter,
}

impl Debug for ContainerNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerNode").finish_non_exhaustive()
    }
}

pub struct ContainerChild {
    pub node: Rc<dyn ToplevelNode>,
    pub active: Cell<bool>,
    pub attention_requested: Cell<bool>,
    title: RefCell<String>,
    pub title_tex: SmallMap<Scale, TextTexture, 2>,
    pub title_rect: Cell<Rect>,
    focus_history: Cell<Option<LinkedNode<NodeRef<ContainerChild>>>>,

    // fields below only valid in tabbed layout
    pub body: Cell<Rect>,
    pub content: Cell<Rect>,
    factor: Cell<f64>,
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
    op: Option<SeatOp>,
    double_click_state: DoubleClickState,
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
        child: Rc<dyn ToplevelNode>,
        split: ContainerSplit,
    ) -> Rc<Self> {
        let children = LinkedList::new();
        let child_node = children.add_last(ContainerChild {
            node: child.clone(),
            active: Default::default(),
            body: Default::default(),
            content: Default::default(),
            factor: Cell::new(1.0),
            title: Default::default(),
            title_tex: Default::default(),
            title_rect: Default::default(),
            focus_history: Default::default(),
            attention_requested: Cell::new(false),
        });
        let child_node_ref = child_node.clone();
        let mut child_nodes = AHashMap::new();
        child_nodes.insert(child.node_id(), child_node);
        let slf = Rc::new(Self {
            id: state.node_ids.next(),
            split: Cell::new(split),
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
            compute_render_data_scheduled: Cell::new(false),
            num_children: NumCell::new(1),
            children,
            focus_history: Default::default(),
            child_nodes: RefCell::new(child_nodes),
            workspace: CloneCell::new(workspace.clone()),
            cursors: RefCell::new(Default::default()),
            state: state.clone(),
            render_data: Default::default(),
            scroller: Default::default(),
            toplevel_data: ToplevelData::new(state, Default::default(), None),
            attention_requests: Default::default(),
        });
        child.tl_set_parent(slf.clone());
        slf.pull_child_properties(&child_node_ref);
        slf
    }

    pub fn prepend_child(self: &Rc<Self>, new: Rc<dyn ToplevelNode>) {
        if let Some(child) = self.children.first() {
            self.add_child_before_(&child, new);
        }
    }

    pub fn append_child(self: &Rc<Self>, new: Rc<dyn ToplevelNode>) {
        if let Some(child) = self.children.last() {
            self.add_child_after_(&child, new);
        }
    }

    pub fn add_child_after(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn ToplevelNode>) {
        self.add_child_x(prev, new, |prev, new| self.add_child_after_(prev, new));
    }

    pub fn add_child_before(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn ToplevelNode>) {
        self.add_child_x(prev, new, |prev, new| self.add_child_before_(prev, new));
    }

    fn add_child_x<F>(self: &Rc<Self>, prev: &dyn Node, new: Rc<dyn ToplevelNode>, f: F)
    where
        F: FnOnce(&NodeRef<ContainerChild>, Rc<dyn ToplevelNode>),
    {
        let node = self
            .child_nodes
            .borrow()
            .get(&prev.node_id())
            .map(|n| n.to_ref());
        if let Some(node) = node {
            f(&node, new);
            return;
        }
        log::error!(
            "Tried to add a child to a container but the preceding node is not in the container"
        );
    }

    fn add_child_after_(
        self: &Rc<Self>,
        prev: &NodeRef<ContainerChild>,
        new: Rc<dyn ToplevelNode>,
    ) {
        self.add_child(|cc| prev.append(cc), new);
    }

    fn add_child_before_(
        self: &Rc<Self>,
        prev: &NodeRef<ContainerChild>,
        new: Rc<dyn ToplevelNode>,
    ) {
        self.add_child(|cc| prev.prepend(cc), new);
    }

    fn add_child<F>(self: &Rc<Self>, f: F, new: Rc<dyn ToplevelNode>)
    where
        F: FnOnce(ContainerChild) -> LinkedNode<ContainerChild>,
    {
        let new_ref = {
            let mut links = self.child_nodes.borrow_mut();
            if links.contains_key(&new.node_id()) {
                log::error!("Tried to add a child to a container that already contains the child");
                return;
            }
            let link = f(ContainerChild {
                node: new.clone(),
                active: Default::default(),
                body: Default::default(),
                content: Default::default(),
                factor: Default::default(),
                title: Default::default(),
                title_tex: Default::default(),
                title_rect: Default::default(),
                focus_history: Default::default(),
                attention_requested: Default::default(),
            });
            let r = link.to_ref();
            links.insert(new.node_id(), link);
            r
        };
        new.tl_set_parent(self.clone());
        self.pull_child_properties(&new_ref);
        new.tl_set_visible(self.toplevel_data.visible.get());
        let num_children = self.num_children.fetch_add(1) + 1;
        self.update_content_size();
        let new_child_factor = 1.0 / num_children as f64;
        let mut sum_factors = 0.0;
        for child in self.children.iter() {
            let factor = if rc_eq(&child.node, &new) {
                new_child_factor
            } else {
                child.factor.get() * (1.0 - new_child_factor)
            };
            child.factor.set(factor);
            sum_factors += factor;
        }
        self.sum_factors.set(sum_factors);
        if self.mono_child.is_some() {
            self.activate_child(&new_ref);
        }
        // log::info!("add_child");
        self.schedule_layout();
        self.cancel_seat_ops();
    }

    fn cancel_seat_ops(&self) {
        let mut seats = self.cursors.borrow_mut();
        for seat in seats.values_mut() {
            seat.op = None;
        }
    }

    pub fn on_spaces_changed(self: &Rc<Self>) {
        self.update_content_size();
        // log::info!("on_spaces_changed");
        self.schedule_layout();
    }

    pub fn on_colors_changed(self: &Rc<Self>) {
        // log::info!("on_colors_changed");
        self.schedule_compute_render_data();
    }

    fn damage(&self) {
        self.state.damage(
            Rect::new_sized(
                self.abs_x1.get(),
                self.abs_y1.get(),
                self.width.get(),
                self.height.get(),
            )
            .unwrap(),
        );
    }

    fn schedule_layout(self: &Rc<Self>) {
        if !self.layout_scheduled.replace(true) {
            self.state.pending_container_layout.push(self.clone());
            if self.toplevel_data.visible.get() {
                self.damage();
            }
        }
    }

    fn perform_layout(self: &Rc<Self>) {
        if self.num_children.get() == 0 {
            return;
        }
        self.layout_scheduled.set(false);
        if let Some(child) = self.mono_child.get() {
            self.perform_mono_layout(&child);
        } else {
            self.perform_split_layout();
        }
        self.state.tree_changed();
        // log::info!("perform_layout");
        self.schedule_compute_render_data();
    }

    fn perform_mono_layout(self: &Rc<Self>, child: &ContainerChild) {
        let mb = self.mono_body.get();
        child
            .node
            .clone()
            .tl_change_extents(&mb.move_(self.abs_x1.get(), self.abs_y1.get()));
        self.mono_content
            .set(child.content.get().at_point(mb.x1(), mb.y1()));

        let th = self.state.theme.sizes.title_height.get();
        let bw = self.state.theme.sizes.border_width.get();
        let num_children = self.num_children.get() as i32;
        let content_width = self.width.get().sub(bw * (num_children - 1)).max(0);
        let width_per_child = content_width / num_children;
        let mut rem = content_width % num_children;
        let mut pos = 0;
        for child in self.children.iter() {
            let mut width = width_per_child;
            if rem > 0 {
                width += 1;
                rem -= 1;
            }
            child
                .title_rect
                .set(Rect::new_sized(pos, 0, width, th).unwrap());
            pos += width + bw;
        }
    }

    fn perform_split_layout(self: &Rc<Self>) {
        let sum_factors = self.sum_factors.get();
        let border_width = self.state.theme.sizes.border_width.get();
        let title_height = self.state.theme.sizes.title_height.get();
        let split = self.split.get();
        let (content_size, other_content_size) = match split {
            ContainerSplit::Horizontal => (self.content_width.get(), self.content_height.get()),
            ContainerSplit::Vertical => (self.content_height.get(), self.content_width.get()),
        };
        let num_children = self.num_children.get();
        if num_children == 0 {
            return;
        }
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
                        (
                            0,
                            pos + title_height + 1,
                            other_content_size,
                            height,
                            height,
                        )
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
        for child in self.children.iter() {
            let body = child.body.get();
            child.title_rect.set(
                Rect::new_sized(
                    body.x1(),
                    body.y1() - title_height - 1,
                    body.width(),
                    title_height,
                )
                .unwrap(),
            );
            let body = body.move_(self.abs_x1.get(), self.abs_y1.get());
            child.node.clone().tl_change_extents(&body);
            child.position_content();
        }
    }

    fn update_content_size(&self) {
        let border_width = self.state.theme.sizes.border_width.get();
        let title_height = self.state.theme.sizes.title_height.get();
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
        self.mono_body.set(
            Rect::new_sized(
                0,
                title_height + 1,
                self.width.get(),
                self.height.get().sub(title_height + 1).max(0),
            )
            .unwrap(),
        );
    }

    fn pointer_move(
        self: &Rc<Self>,
        id: CursorType,
        cursor: &CursorUser,
        x: Fixed,
        y: Fixed,
        target: bool,
    ) {
        let mut x = x.round_down();
        let mut y = y.round_down();
        let title_height = self.state.theme.sizes.title_height.get();
        let mut seats = self.cursors.borrow_mut();
        let seat_state = seats.entry(id).or_insert_with(|| CursorState {
            cursor: KnownCursor::Default,
            target,
            x,
            y,
            op: None,
            double_click_state: Default::default(),
        });
        let mut changed = false;
        changed |= mem::replace(&mut seat_state.x, x) != x;
        changed |= mem::replace(&mut seat_state.y, y) != y;
        if !changed {
            return;
        }
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
                    // log::info!("pointer_move");
                    self.schedule_layout();
                }
            }
            return;
        }
        let new_cursor = if self.mono_child.is_some() {
            KnownCursor::Default
        } else if self.split.get() == ContainerSplit::Horizontal {
            if y < title_height + 1 {
                KnownCursor::Default
            } else {
                KnownCursor::EwResize
            }
        } else {
            let mut cursor = KnownCursor::Default;
            for child in self.children.iter() {
                let body = child.body.get();
                if body.y1() > y {
                    if body.y1() - y > title_height + 1 {
                        cursor = KnownCursor::NsResize
                    }
                    break;
                }
            }
            cursor
        };
        if new_cursor != mem::replace(&mut seat_state.cursor, new_cursor) {
            if seat_state.target {
                cursor.set_known(new_cursor);
            }
        }
    }

    fn update_title(&self) {
        let mut title = self.toplevel_data.title.borrow_mut();
        title.clear();
        let split = match (self.mono_child.is_some(), self.split.get()) {
            (true, _) => "T",
            (_, ContainerSplit::Horizontal) => "H",
            (_, ContainerSplit::Vertical) => "V",
        };
        title.push_str(split);
        title.push_str("[");
        for (i, c) in self.children.iter().enumerate() {
            if i > 0 {
                title.push_str(", ");
            }
            title.push_str(c.title.borrow_mut().deref());
        }
        title.push_str("]");
        drop(title);
        self.tl_title_changed();
    }

    pub fn schedule_compute_render_data(self: &Rc<Self>) {
        if !self.compute_render_data_scheduled.replace(true) {
            self.state.pending_container_render_data.push(self.clone());
        }
    }

    fn compute_render_data(&self) {
        self.compute_render_data_scheduled.set(false);
        let mut rd = self.render_data.borrow_mut();
        let rd = rd.deref_mut();
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let font = theme.font.get();
        let cwidth = self.width.get();
        let cheight = self.height.get();
        let ctx = self.state.render_ctx.get();
        for (_, v) in rd.titles.iter_mut() {
            v.clear();
        }
        rd.title_rects.clear();
        rd.active_title_rects.clear();
        rd.attention_title_rects.clear();
        rd.border_rects.clear();
        rd.underline_rects.clear();
        rd.last_active_rect.take();
        let last_active = self.focus_history.last().map(|v| v.node.node_id());
        let mono = self.mono_child.is_some();
        let split = self.split.get();
        let have_active = self.children.iter().any(|c| c.active.get());
        let scales = self.state.scales.lock();
        let abs_x = self.abs_x1.get();
        let abs_y = self.abs_y1.get();
        for (i, child) in self.children.iter().enumerate() {
            let rect = child.title_rect.get();
            if self.toplevel_data.visible.get() {
                self.state.damage(rect.move_(abs_x, abs_y));
            }
            if i > 0 {
                let rect = if mono {
                    Rect::new_sized(rect.x1() - bw, 0, bw, th)
                } else if split == ContainerSplit::Horizontal {
                    Rect::new_sized(rect.x1() - bw, 0, bw, cheight)
                } else {
                    Rect::new_sized(0, rect.y1() - bw, cwidth, bw)
                };
                rd.border_rects.push(rect.unwrap());
            }
            let color = if child.active.get() {
                rd.active_title_rects.push(rect);
                theme.colors.focused_title_text.get()
            } else if child.attention_requested.get() {
                rd.attention_title_rects.push(rect);
                theme.colors.unfocused_title_text.get()
            } else if !have_active && last_active == Some(child.node.node_id()) {
                rd.last_active_rect = Some(rect);
                theme.colors.focused_inactive_title_text.get()
            } else {
                rd.title_rects.push(rect);
                theme.colors.unfocused_title_text.get()
            };
            if !mono {
                let rect = Rect::new_sized(rect.x1(), rect.y2(), rect.width(), 1).unwrap();
                rd.underline_rects.push(rect);
            }
            let title = child.title.borrow_mut();
            for (scale, _) in scales.iter() {
                let old_tex = child.title_tex.remove(scale);
                let titles = rd.titles.get_or_default_mut(*scale);
                'render_title: {
                    let mut th = th;
                    let mut scalef = None;
                    let mut width = rect.width();
                    if *scale != 1 {
                        let scale = scale.to_f64();
                        th = (th as f64 * scale).round() as _;
                        width = (width as f64 * scale).round() as _;
                        scalef = Some(scale);
                    }
                    if th == 0 || width == 0 || title.is_empty() {
                        break 'render_title;
                    }
                    if let Some(ctx) = &ctx {
                        match text::render(
                            ctx,
                            old_tex,
                            width,
                            th,
                            &font,
                            title.deref(),
                            color,
                            scalef,
                        ) {
                            Ok(t) => {
                                child.title_tex.insert(*scale, t.clone());
                                titles.push(ContainerTitle {
                                    x: rect.x1(),
                                    y: rect.y1(),
                                    tex: t,
                                })
                            }
                            Err(e) => {
                                log::error!("Could not render title {}: {}", title, ErrorFmt(e));
                            }
                        }
                    }
                }
            }
        }
        if mono {
            rd.underline_rects
                .push(Rect::new_sized(0, th, cwidth, 1).unwrap());
        }
        rd.titles.remove_if(|_, v| v.is_empty());
    }

    fn activate_child(self: &Rc<Self>, child: &NodeRef<ContainerChild>) {
        self.activate_child2(child, false);
    }

    fn activate_child2(self: &Rc<Self>, child: &NodeRef<ContainerChild>, preserve_focus: bool) {
        if let Some(mc) = self.mono_child.get() {
            if mc.node.node_id() == child.node.node_id() {
                return;
            }
            if !preserve_focus {
                let seats = collect_kb_foci(mc.node.clone().tl_into_node());
                mc.node.tl_set_visible(false);
                for seat in seats {
                    child
                        .node
                        .clone()
                        .node_do_focus(&seat, Direction::Unspecified);
                }
            }
            self.mono_child.set(Some(child.clone()));
            if self.toplevel_data.visible.get() {
                child.node.tl_set_visible(true);
            }
            child.node.tl_restack_popups();
            // log::info!("activate_child2");
            self.schedule_layout();
        }
    }

    pub fn set_mono(self: &Rc<Self>, child: Option<&dyn ToplevelNode>) {
        if self.mono_child.is_some() == child.is_some() {
            return;
        }
        let child = {
            let children = self.child_nodes.borrow();
            match child {
                Some(c) => match children.get(&c.node_id()) {
                    Some(c) => Some(c.to_ref()),
                    _ => {
                        log::warn!("set_mono called with a node that is not a child");
                        return;
                    }
                },
                _ => None,
            }
        };
        if self.toplevel_data.visible.get() {
            if let Some(child) = &child {
                let child_id = child.node.node_id();
                let mut seats = SmallVec::<[_; 3]>::new();
                for other in self.children.iter() {
                    if other.node.node_id() != child_id {
                        collect_kb_foci2(other.node.clone().tl_into_node(), &mut seats);
                        other.node.tl_set_visible(false);
                    }
                }
                for seat in seats {
                    child
                        .node
                        .clone()
                        .node_do_focus(&seat, Direction::Unspecified);
                }
                child.node.tl_restack_popups();
            } else {
                for child in self.children.iter() {
                    child.node.tl_set_visible(true);
                }
            }
        }
        self.mono_child.set(child);
        // log::info!("set_mono");
        self.schedule_layout();
        self.update_title();
    }

    pub fn set_split(self: &Rc<Self>, split: ContainerSplit) {
        if self.split.replace(split) != split {
            self.update_content_size();
            // log::info!("set_split");
            self.schedule_layout();
            self.update_title();
        }
    }

    fn parent_container(&self) -> Option<Rc<ContainerNode>> {
        self.toplevel_data
            .parent
            .get()
            .and_then(|p| p.node_into_container())
    }

    pub fn move_focus_from_child(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        child: &dyn ToplevelNode,
        direction: Direction,
    ) {
        let child = match self.child_nodes.borrow().get(&child.node_id()) {
            Some(c) => c.to_ref(),
            _ => return,
        };
        let mc = self.mono_child.get();
        let in_line = if mc.is_some() {
            matches!(direction, Direction::Left | Direction::Right)
        } else {
            match self.split.get() {
                ContainerSplit::Horizontal => {
                    matches!(direction, Direction::Left | Direction::Right)
                }
                ContainerSplit::Vertical => matches!(direction, Direction::Up | Direction::Down),
            }
        };
        if !in_line {
            if let Some(c) = self.parent_container() {
                c.move_focus_from_child(seat, self.deref(), direction);
            }
            return;
        }
        let prev = match direction {
            Direction::Left => true,
            Direction::Down => false,
            Direction::Up => true,
            Direction::Right => false,
            Direction::Unspecified => true,
        };
        let sibling = match prev {
            true => child.prev(),
            false => child.next(),
        };
        let sibling = match sibling {
            Some(s) => s,
            None => {
                if let Some(c) = self.parent_container() {
                    c.move_focus_from_child(seat, self.deref(), direction);
                }
                return;
            }
        };
        if mc.is_some() {
            self.activate_child(&sibling);
        } else {
            sibling.node.clone().node_do_focus(seat, direction);
        }
    }

    //
    pub fn move_child(self: Rc<Self>, child: Rc<dyn ToplevelNode>, direction: Direction) {
        // CASE 1: This is the only child of the container. Replace the container by the child.
        if self.num_children.get() == 1 {
            if let Some(parent) = self.toplevel_data.parent.get() {
                if !self.toplevel_data.is_fullscreen.get()
                    && parent.cnode_accepts_child(child.tl_as_node())
                {
                    parent.cnode_replace_child(self.deref(), child.clone());
                }
            }
            return;
        }
        let (split, prev) = direction_to_split(direction);
        // CASE 2: We're moving the child within the container.
        if split == self.split.get()
            || (split == ContainerSplit::Horizontal && self.mono_child.is_some())
        {
            let cc = match self.child_nodes.borrow().get(&child.node_id()) {
                Some(l) => l.to_ref(),
                None => return,
            };
            let neighbor = match prev {
                true => cc.prev(),
                false => cc.next(),
            };
            if let Some(neighbor) = neighbor {
                if let Some(cn) = neighbor.node.clone().node_into_container() {
                    if cn.cnode_accepts_child(child.tl_as_node()) {
                        if let Some(mc) = self.mono_child.get() {
                            if mc.node.node_id() == child.node_id() {
                                self.activate_child2(&neighbor, true);
                            }
                        }
                        self.cnode_remove_child2(child.tl_as_node(), true);
                        cn.insert_child(child, direction);
                        return;
                    }
                }
                match prev {
                    true => neighbor.prepend_existing(&cc),
                    false => neighbor.append_existing(&cc),
                }
                // log::info!("move_child");
                self.schedule_layout();
                return;
            }
        }
        // CASE 3: We're moving the child out of the container.
        let mut neighbor = self.clone();
        let mut parent_opt = self.parent_container();
        while let Some(parent) = &parent_opt {
            if parent.split.get() == split {
                break;
            }
            neighbor = parent.clone();
            parent_opt = parent.parent_container();
        }
        let parent = match parent_opt {
            Some(p) => p,
            _ => return,
        };
        self.cnode_remove_child2(child.tl_as_node(), true);
        match prev {
            true => parent.add_child_before(&*neighbor, child.clone()),
            false => parent.add_child_after(&*neighbor, child.clone()),
        }
    }

    pub fn insert_child(self: &Rc<Self>, node: Rc<dyn ToplevelNode>, direction: Direction) {
        let (split, right) = direction_to_split(direction);
        if split != self.split.get() || right {
            self.append_child(node);
        } else {
            self.prepend_child(node);
        }
    }

    fn update_child_title(self: &Rc<Self>, child: &ContainerChild, title: &str) {
        {
            let mut ct = child.title.borrow_mut();
            if ct.deref() == title {
                return;
            }
            ct.clear();
            ct.push_str(title);
        }
        self.update_title();
        // log::info!("node_child_title_changed");
        self.schedule_compute_render_data();
    }

    fn update_child_active(
        self: &Rc<Self>,
        node: &NodeRef<ContainerChild>,
        active: bool,
        depth: u32,
    ) {
        if depth == 1 {
            node.active.set(active);
        }
        if active {
            node.focus_history
                .set(Some(self.focus_history.add_last(node.clone())));
        }
        // log::info!("node_child_active_changed");
        self.schedule_compute_render_data();
        if let Some(parent) = self.toplevel_data.parent.get() {
            parent.node_child_active_changed(self.deref(), active, depth + 1);
        }
    }

    fn update_child_size(&self, node: &NodeRef<ContainerChild>, width: i32, height: i32) {
        let rect = Rect::new(0, 0, width, height).unwrap();
        node.content.set(rect);
        node.position_content();
        if let Some(mono) = self.mono_child.get() {
            if mono.node.node_id() == node.node.node_id() {
                let body = self.mono_body.get();
                self.mono_content.set(rect.at_point(body.x1(), body.y1()));
            }
        }
    }

    fn pull_child_properties(self: &Rc<Self>, child: &NodeRef<ContainerChild>) {
        let data = child.node.tl_data();
        {
            let attention_requested = data.wants_attention.get();
            child.attention_requested.set(attention_requested);
            if attention_requested {
                self.mod_attention_requests(true);
            }
        }
        self.update_child_title(child, &data.title.borrow());
        self.update_child_active(child, data.active(), 1);
        {
            let pos = data.pos.get();
            self.update_child_size(child, pos.width(), pos.height());
        }
    }

    fn discard_child_properties(&self, child: &ContainerChild) {
        if child.attention_requested.get() {
            self.mod_attention_requests(false);
        }
    }

    fn mod_attention_requests(&self, set: bool) {
        let propagate = self.attention_requests.adj(set);
        if set || propagate {
            self.toplevel_data.wants_attention.set(set);
        }
        if propagate {
            if let Some(parent) = self.toplevel_data.parent.get() {
                parent.cnode_child_attention_request_changed(self, set);
            }
        }
    }

    fn button(
        self: Rc<Self>,
        id: CursorType,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        pressed: bool,
    ) {
        let mut seat_datas = self.cursors.borrow_mut();
        let seat_data = match seat_datas.get_mut(&id) {
            Some(s) => s,
            _ => return,
        };
        if seat_data.op.is_none() {
            if !pressed {
                return;
            }
            let (kind, child) = 'res: {
                let mono = self.mono_child.is_some();
                for child in self.children.iter() {
                    let rect = child.title_rect.get();
                    if rect.contains(seat_data.x, seat_data.y) {
                        self.activate_child(&child);
                        child
                            .node
                            .clone()
                            .node_do_focus(seat, Direction::Unspecified);
                        break 'res (SeatOpKind::Move, child);
                    } else if !mono {
                        if self.split.get() == ContainerSplit::Horizontal {
                            if seat_data.x < rect.x1() {
                                break 'res (
                                    SeatOpKind::Resize {
                                        dist_left: seat_data.x
                                            - child.prev().unwrap().body.get().x2(),
                                        dist_right: child.body.get().x1() - seat_data.x,
                                    },
                                    child,
                                );
                            }
                        } else {
                            if seat_data.y < rect.y1() {
                                break 'res (
                                    SeatOpKind::Resize {
                                        dist_left: seat_data.y
                                            - child.prev().unwrap().body.get().y2(),
                                        dist_right: child.body.get().y1() - seat_data.y,
                                    },
                                    child,
                                );
                            }
                        }
                    }
                }
                return;
            };
            if seat_data
                .double_click_state
                .click(&self.state, time_usec, seat_data.x, seat_data.y)
                && kind == SeatOpKind::Move
            {
                drop(seat_datas);
                seat.set_tl_floating(child.node.clone(), true);
                return;
            }
            seat_data.op = Some(SeatOp { child, kind })
        } else if !pressed {
            let op = seat_data.op.take().unwrap();
            drop(seat_datas);
            if op.kind == SeatOpKind::Move {
                // todo
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

pub async fn container_layout(state: Rc<State>) {
    loop {
        let container = state.pending_container_layout.pop().await;
        if container.layout_scheduled.get() {
            container.perform_layout();
        }
    }
}

pub async fn container_render_data(state: Rc<State>) {
    loop {
        let container = state.pending_container_render_data.pop().await;
        if container.compute_render_data_scheduled.get() {
            container.compute_render_data();
        }
    }
}

impl Node for ContainerNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.toplevel_data.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_container(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        for child in self.children.iter() {
            child.node.clone().node_visit(visitor);
        }
    }

    fn node_visible(&self) -> bool {
        self.toplevel_data.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        Rect::new_sized(
            self.abs_x1.get(),
            self.abs_y1.get(),
            self.width.get(),
            self.height.get(),
        )
        .unwrap()
    }

    fn node_child_title_changed(self: Rc<Self>, child: &dyn Node, title: &str) {
        if let Some(child) = self.child_nodes.borrow().get(&child.node_id()) {
            self.update_child_title(child, title);
        }
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        let node = if let Some(cn) = self.mono_child.get() {
            Some(cn)
        } else {
            let split = self.split.get();
            match (direction, split) {
                (Direction::Left, ContainerSplit::Horizontal) => self.children.last(),
                (Direction::Down, ContainerSplit::Vertical) => self.children.first(),
                (Direction::Up, ContainerSplit::Vertical) => self.children.last(),
                (Direction::Right, ContainerSplit::Horizontal) => self.children.first(),
                _ => match self.focus_history.last() {
                    Some(n) => Some(n.deref().clone()),
                    None => self.children.last(),
                },
            }
        };
        if let Some(node) = node {
            node.node.clone().node_do_focus(seat, direction);
        }
    }

    fn node_active_changed(&self, active: bool) {
        self.toplevel_data.update_self_active(self, active);
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        let mut recurse = |content: Rect, child: NodeRef<ContainerChild>| {
            if content.contains(x, y) {
                let (x, y) = content.translate(x, y);
                tree.push(FoundNode {
                    node: child.node.clone().tl_into_node(),
                    x,
                    y,
                });
                child.node.node_find_tree_at(x, y, tree, usecase);
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

    fn node_child_size_changed(&self, child: &dyn Node, width: i32, height: i32) {
        let cn = self.child_nodes.borrow();
        if let Some(node) = cn.get(&child.node_id()) {
            self.update_child_size(node, width, height);
        }
    }

    fn node_child_active_changed(self: Rc<Self>, child: &dyn Node, active: bool, depth: u32) {
        if let Some(l) = self.child_nodes.borrow().get(&child.node_id()) {
            self.update_child_active(l, active, depth);
        }
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_container(self, x, y);
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn crate::tree::ToplevelNode>> {
        Some(self)
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        _serial: u32,
    ) {
        if button != BTN_LEFT {
            return;
        }
        let id = CursorType::Seat(seat.id());
        self.button(id, seat, time_usec, state == KeyState::Pressed);
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        let mut seat_datas = self.cursors.borrow_mut();
        let id = CursorType::Seat(seat.id());
        let seat_data = match seat_datas.get_mut(&id) {
            Some(s) => s,
            _ => return,
        };
        if seat_data.y > self.state.theme.sizes.title_height.get() {
            return;
        }
        let cur_mc = match self.mono_child.get() {
            Some(mc) => mc,
            _ => return,
        };
        let discrete = match self.scroller.handle(event) {
            Some(d) => d,
            _ => return,
        };
        let mut new_mc = cur_mc.clone();
        for _ in 0..discrete.abs() {
            let new = if discrete < 0 {
                new_mc.prev()
            } else {
                new_mc.next()
            };
            new_mc = match new {
                Some(n) => n,
                None => break,
            };
        }
        self.activate_child(&new_mc);
        new_mc
            .node
            .clone()
            .node_do_focus(seat, Direction::Unspecified);
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        // log::info!("node_on_pointer_enter");
        self.pointer_move(
            CursorType::Seat(seat.id()),
            seat.pointer_cursor(),
            x,
            y,
            false,
        );
    }

    fn node_on_pointer_unfocus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("unfocus");
        let mut seats = self.cursors.borrow_mut();
        let id = CursorType::Seat(seat.id());
        if let Some(seat_state) = seats.get_mut(&id) {
            seat_state.target = false;
        }
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("container focus");
        let mut seats = self.cursors.borrow_mut();
        let id = CursorType::Seat(seat.id());
        if let Some(seat_state) = seats.get_mut(&id) {
            seat_state.target = true;
            seat.pointer_cursor().set_known(seat_state.cursor);
        }
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        // log::info!("node_on_pointer_motion");
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
        let id = CursorType::TabletTool(tool.id);
        self.pointer_move(id, tool.cursor(), x, y, false);
        if let Some(changes) = changes {
            if let Some(pressed) = changes.down {
                self.button(id, tool.seat(), time_usec, pressed);
            }
        }
    }

    fn node_into_container(self: Rc<Self>) -> Option<Rc<ContainerNode>> {
        Some(self.clone())
    }

    fn node_into_containing_node(self: Rc<Self>) -> Option<Rc<dyn ContainingNode>> {
        Some(self)
    }

    fn node_into_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        Some(self)
    }

    fn node_is_container(&self) -> bool {
        true
    }
}

impl ContainingNode for ContainerNode {
    fn cnode_replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn ToplevelNode>) {
        let node = match self.child_nodes.borrow_mut().remove(&old.node_id()) {
            Some(c) => c,
            None => {
                log::error!("Trying to replace a node that isn't a child of this container");
                return;
            }
        };
        let (have_mc, was_mc) = match self.mono_child.get() {
            None => (false, false),
            Some(mc) => (true, mc.node.node_id() == old.node_id()),
        };
        self.discard_child_properties(&node);
        let link = node.append(ContainerChild {
            node: new.clone(),
            active: Cell::new(false),
            body: Cell::new(node.body.get()),
            content: Default::default(),
            factor: Cell::new(node.factor.get()),
            title: Default::default(),
            title_tex: Default::default(),
            title_rect: Cell::new(node.title_rect.get()),
            focus_history: Cell::new(None),
            attention_requested: Cell::new(false),
        });
        if let Some(fh) = node.focus_history.take() {
            link.focus_history.set(Some(fh.append(link.to_ref())));
        }
        let visible = node.node.node_visible();
        drop(node);
        let mut body = None;
        if was_mc {
            self.mono_child.set(Some(link.to_ref()));
            link.node.tl_restack_popups();
            body = Some(self.mono_body.get());
        } else if !have_mc {
            body = Some(link.body.get());
        };
        let link_ref = link.to_ref();
        self.child_nodes.borrow_mut().insert(new.node_id(), link);
        new.tl_set_parent(self.clone());
        self.pull_child_properties(&link_ref);
        new.tl_set_visible(visible);
        if let Some(body) = body {
            let body = body.move_(self.abs_x1.get(), self.abs_y1.get());
            new.clone().tl_change_extents(&body);
            self.state.damage(body);
        }
    }

    fn cnode_remove_child2(self: Rc<Self>, child: &dyn Node, preserve_focus: bool) {
        let node = match self.child_nodes.borrow_mut().remove(&child.node_id()) {
            Some(c) => c,
            None => return,
        };
        node.focus_history.set(None);
        self.discard_child_properties(&node);
        if let Some(mono) = self.mono_child.get() {
            if mono.node.node_id() == child.node_id() {
                let mut new = self.focus_history.last().map(|n| n.deref().clone());
                if new.is_none() {
                    new = node.next();
                    if new.is_none() {
                        new = node.prev();
                    }
                }
                if let Some(child) = &new {
                    self.activate_child2(child, preserve_focus);
                }
            }
        }
        let node = {
            let node = node;
            node.focus_history.set(None);
            node.to_ref()
        };
        let num_children = self.num_children.fetch_sub(1) - 1;
        if num_children == 0 {
            self.tl_destroy();
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
        // log::info!("cnode_remove_child2");
        self.schedule_layout();
        self.cancel_seat_ops();
    }

    fn cnode_accepts_child(&self, _node: &dyn Node) -> bool {
        true
    }

    fn cnode_child_attention_request_changed(self: Rc<Self>, child: &dyn Node, set: bool) {
        let children = self.child_nodes.borrow();
        let child = match children.get(&child.node_id()) {
            Some(c) => c,
            _ => return,
        };
        if child.attention_requested.replace(set) == set {
            return;
        }
        self.mod_attention_requests(set);
        self.schedule_compute_render_data();
    }

    fn cnode_workspace(self: Rc<Self>) -> Rc<WorkspaceNode> {
        self.workspace.get()
    }

    fn cnode_set_child_position(self: Rc<Self>, child: &dyn Node, x: i32, y: i32) {
        let Some(parent) = self.toplevel_data.parent.get() else {
            return;
        };
        let th = self.state.theme.sizes.title_height.get();
        if self.mono_child.is_some() {
            parent.cnode_set_child_position(&*self, x, y - th - 1);
        } else {
            let children = self.child_nodes.borrow();
            let Some(child) = children.get(&child.node_id()) else {
                return;
            };
            let pos = child.body.get();
            let (x, y) = pos.translate(x, y);
            parent.cnode_set_child_position(&*self, x, y);
        }
    }

    fn cnode_resize_child(
        self: Rc<Self>,
        child: &dyn Node,
        new_x1: Option<i32>,
        new_y1: Option<i32>,
        new_x2: Option<i32>,
        new_y2: Option<i32>,
    ) {
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let mut left_outside = false;
        let mut right_outside = false;
        let mut top_outside = false;
        let mut bottom_outside = false;
        if self.mono_child.is_some() {
            top_outside = true;
            right_outside = true;
            bottom_outside = true;
            left_outside = true;
        } else {
            let children = self.child_nodes.borrow();
            let Some(child) = children.get(&child.node_id()) else {
                return;
            };
            let pos = child.body.get();
            let split = self.split.get();
            let mut changed_any = false;
            let (mut i1, mut i2, new_i1, new_i2, mut ci) = match split {
                ContainerSplit::Horizontal => {
                    top_outside = true;
                    bottom_outside = true;
                    (pos.x1(), pos.x2(), new_x1, new_x2, self.content_width.get())
                }
                ContainerSplit::Vertical => {
                    right_outside = true;
                    left_outside = true;
                    (
                        pos.y1(),
                        pos.y2(),
                        new_y1,
                        new_y2,
                        self.content_height.get(),
                    )
                }
            };
            if ci == 0 {
                ci = 1;
            }
            let (new_delta, between) = match split {
                ContainerSplit::Horizontal => (self.abs_x1.get(), bw),
                ContainerSplit::Vertical => (self.abs_y1.get(), bw + th + 1),
            };
            let new_i1 = new_i1.map(|v| v - new_delta);
            let new_i2 = new_i2.map(|v| v - new_delta);
            let (orig_i1, orig_i2) = (i1, i2);
            let mut sum_factors = self.sum_factors.get();
            if let Some(new_i1) = new_i1 {
                if let Some(peer) = child.prev() {
                    let peer_pos = peer.body.get();
                    let peer_i1 = match self.split.get() {
                        ContainerSplit::Horizontal => peer_pos.x1(),
                        ContainerSplit::Vertical => peer_pos.y1(),
                    };
                    i1 = new_i1.max(peer_i1 + between).min(i2);
                    if i1 != orig_i1 {
                        let peer_factor = (i1 - between - peer_i1) as f64 / ci as f64;
                        sum_factors = sum_factors - peer.factor.get() + peer_factor;
                        peer.factor.set(peer_factor);
                        changed_any = true;
                    }
                } else {
                    match split {
                        ContainerSplit::Horizontal => left_outside = true,
                        ContainerSplit::Vertical => top_outside = true,
                    }
                }
            }
            if let Some(new_i2) = new_i2 {
                if let Some(peer) = child.next() {
                    let peer_pos = peer.body.get();
                    let peer_i2 = match self.split.get() {
                        ContainerSplit::Horizontal => peer_pos.x2(),
                        ContainerSplit::Vertical => peer_pos.y2(),
                    };
                    i2 = new_i2.min(peer_i2 - between).max(i1);
                    if i2 != orig_i2 {
                        let peer_factor = (peer_i2 - between - i2) as f64 / ci as f64;
                        sum_factors = sum_factors - peer.factor.get() + peer_factor;
                        peer.factor.set(peer_factor);
                        changed_any = true;
                    }
                } else {
                    match split {
                        ContainerSplit::Horizontal => right_outside = true,
                        ContainerSplit::Vertical => bottom_outside = true,
                    }
                }
            }
            if changed_any {
                let factor = (i2 - i1) as f64 / ci as f64;
                sum_factors = sum_factors - child.factor.get() + factor;
                child.factor.set(factor);
                self.sum_factors.set(sum_factors);
                self.schedule_layout();
            }
        }
        let pos = self.node_absolute_position();
        let mut x1 = None;
        let mut x2 = None;
        let mut y1 = None;
        let mut y2 = None;
        if left_outside {
            x1 = new_x1.map(|v| v.min(pos.x2()));
        }
        if right_outside {
            x2 = new_x2.map(|v| v.max(x1.unwrap_or(pos.x1())));
        }
        if top_outside {
            y1 = new_y1.map(|v| (v - th - 1).min(pos.y2() - th - 1));
        }
        if bottom_outside {
            y2 = new_y2.map(|v| v.max(y1.unwrap_or(pos.y1()) + th + 1));
        }
        if (x1.is_some() && x1 != Some(pos.x1()))
            || (x2.is_some() && x2 != Some(pos.x2()))
            || (y1.is_some() && y1 != Some(pos.y1()))
            || (y2.is_some() && y2 != Some(pos.y2()))
        {
            if let Some(parent) = self.toplevel_data.parent.get() {
                parent.cnode_resize_child(&*self, x1, y1, x2, y2);
            }
        }
    }
}

impl ToplevelNodeBase for ContainerNode {
    fn tl_data(&self) -> &ToplevelData {
        &self.toplevel_data
    }

    fn tl_default_focus_child(&self) -> Option<Rc<dyn Node>> {
        self.focus_history
            .last()
            .map(|v| v.node.clone())
            .or_else(|| self.children.first().map(|c| c.node.clone()))
            .map(|tl| tl.tl_into_node())
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        for child in self.children.iter() {
            child.node.clone().tl_set_workspace(ws);
        }
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect) {
        self.toplevel_data.pos.set(*rect);
        self.abs_x1.set(rect.x1());
        self.abs_y1.set(rect.y1());
        let mut size_changed = false;
        size_changed |= self.width.replace(rect.width()) != rect.width();
        size_changed |= self.height.replace(rect.height()) != rect.height();
        if size_changed {
            self.update_content_size();
            // log::info!("tl_change_extents");
            self.perform_layout();
            self.cancel_seat_ops();
            if let Some(parent) = self.toplevel_data.parent.get() {
                parent.node_child_size_changed(self.deref(), rect.width(), rect.height());
            }
        } else {
            if let Some(c) = self.mono_child.get() {
                let body = self
                    .mono_body
                    .get()
                    .move_(self.abs_x1.get(), self.abs_y1.get());
                c.node.clone().tl_change_extents(&body);
            } else {
                for child in self.children.iter() {
                    let body = child.body.get().move_(self.abs_x1.get(), self.abs_y1.get());
                    child.node.clone().tl_change_extents(&body);
                }
            }
        }
    }

    fn tl_close(self: Rc<Self>) {
        for child in self.children.iter() {
            child.node.clone().tl_close();
        }
    }

    fn tl_set_visible_impl(&self, visible: bool) {
        if let Some(mc) = self.mono_child.get() {
            mc.node.tl_set_visible(visible);
        } else {
            for child in self.children.iter() {
                child.node.tl_set_visible(visible);
            }
        }
    }

    fn tl_destroy_impl(&self) {
        mem::take(self.cursors.borrow_mut().deref_mut());
        let mut cn = self.child_nodes.borrow_mut();
        for n in cn.drain_values() {
            n.node.tl_destroy();
        }
    }

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
        if let Some(last) = self.focus_history.last() {
            return last.node.clone().tl_last_active_child();
        }
        self
    }

    fn tl_restack_popups(&self) {
        if let Some(mc) = self.mono_child.get() {
            mc.node.tl_restack_popups();
        } else {
            for child in self.children.iter() {
                child.node.tl_restack_popups();
            }
        }
    }

    fn tl_admits_children(&self) -> bool {
        true
    }
}

fn direction_to_split(dir: Direction) -> (ContainerSplit, bool) {
    match dir {
        Direction::Left => (ContainerSplit::Horizontal, true),
        Direction::Down => (ContainerSplit::Vertical, false),
        Direction::Up => (ContainerSplit::Vertical, true),
        Direction::Right => (ContainerSplit::Horizontal, false),
        Direction::Unspecified => (ContainerSplit::Horizontal, true),
    }
}
