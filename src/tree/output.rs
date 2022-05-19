use {
    crate::{
        backend::{KeyState, Mode},
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_output::WlOutputGlobal,
            wl_seat::{
                collect_kb_foci2, wl_pointer::PendingScroll, NodeSeatState, SeatId, WlSeatGlobal,
                BTN_LEFT,
            },
            wl_surface::{
                ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
                zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
            },
            zwlr_layer_shell_v1::{BACKGROUND, BOTTOM, OVERLAY, TOP},
        },
        rect::Rect,
        render::{Renderer, Texture},
        state::State,
        text,
        tree::{
            walker::NodeVisitor, Direction, FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode,
        },
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt,
            linkedlist::LinkedList, scroller::Scroller,
        },
    },
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        ops::{Deref, Sub},
        rc::Rc,
    },
};

tree_id!(OutputNodeId);
pub struct OutputNode {
    pub id: OutputNodeId,
    pub global: Rc<WlOutputGlobal>,
    pub workspaces: LinkedList<Rc<WorkspaceNode>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub seat_state: NodeSeatState,
    pub layers: [LinkedList<Rc<ZwlrLayerSurfaceV1>>; 4],
    pub render_data: RefCell<OutputRenderData>,
    pub state: Rc<State>,
    pub is_dummy: bool,
    pub status: CloneCell<Rc<String>>,
    pub scroll: Scroller,
    pub pointer_positions: CopyHashMap<SeatId, (i32, i32)>,
    pub lock_surface: CloneCell<Option<Rc<ExtSessionLockSurfaceV1>>>,
}

impl OutputNode {
    pub fn clear(&self) {
        self.global.clear();
        self.workspace.set(None);
        let workspaces: Vec<_> = self.workspaces.iter().collect();
        for workspace in workspaces {
            workspace.clear();
        }
        self.render_data.borrow_mut().titles.clear();
        self.lock_surface.take();
    }

    pub fn on_spaces_changed(self: &Rc<Self>) {
        self.update_render_data();
        if let Some(c) = self.workspace.get() {
            c.change_extents(&self.workspace_rect());
        }
    }

    pub fn update_render_data(&self) {
        let mut rd = self.render_data.borrow_mut();
        rd.titles.clear();
        rd.inactive_workspaces.clear();
        rd.active_workspace = None;
        rd.status = None;
        let mut pos = 0;
        let font = self.state.theme.font.borrow_mut();
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let active_id = self.workspace.get().map(|w| w.id);
        let width = self.global.pos.get().width();
        rd.underline = Rect::new_sized(0, th, width, 1).unwrap();
        for ws in self.workspaces.iter() {
            let mut title_width = th;
            'create_texture: {
                if let Some(ctx) = self.state.render_ctx.get() {
                    if th == 0 || ws.name.is_empty() {
                        break 'create_texture;
                    }
                    let tc = match active_id == Some(ws.id) {
                        true => theme.colors.focused_title_text.get(),
                        false => theme.colors.unfocused_title_text.get(),
                    };
                    let title = match text::render_fitting(&ctx, th, &font, &ws.name, tc, false) {
                        Ok(t) => t,
                        Err(e) => {
                            log::error!("Could not render title {}: {}", ws.name, ErrorFmt(e));
                            break 'create_texture;
                        }
                    };
                    let mut x = pos + 1;
                    if title.width() + 2 > title_width {
                        title_width = title.width() + 2;
                    } else {
                        x = pos + (title_width - title.width()) / 2;
                    }
                    rd.titles.push(OutputTitle {
                        x1: pos,
                        x2: pos + title_width,
                        tex_x: x,
                        tex_y: 0,
                        tex: title,
                        ws: ws.deref().clone(),
                    });
                }
            }
            let rect = Rect::new_sized(pos, 0, title_width, th).unwrap();
            if Some(ws.id) == active_id {
                rd.active_workspace = Some(rect);
            } else {
                rd.inactive_workspaces.push(rect);
            }
            pos += title_width;
        }
        'set_status: {
            let ctx = match self.state.render_ctx.get() {
                Some(ctx) => ctx,
                _ => break 'set_status,
            };
            let status = self.status.get();
            if status.is_empty() {
                break 'set_status;
            }
            let tc = self.state.theme.colors.bar_text.get();
            let title = match text::render_fitting(&ctx, th, &font, &status, tc, true) {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Could not render status {}: {}", status, ErrorFmt(e));
                    break 'set_status;
                }
            };
            let pos = width - title.width() - 1;
            rd.status = Some(OutputStatus {
                tex_x: pos,
                tex_y: 0,
                tex: title,
            });
        }
    }

    pub fn ensure_workspace(self: &Rc<Self>) -> Rc<WorkspaceNode> {
        if let Some(ws) = self.workspace.get() {
            return ws;
        }
        let name = 'name: {
            for i in 1.. {
                let name = i.to_string();
                if !self.state.workspaces.contains(&name) {
                    break 'name name;
                }
            }
            unreachable!();
        };
        let workspace = Rc::new(WorkspaceNode {
            id: self.state.node_ids.next(),
            output: CloneCell::new(self.clone()),
            position: Default::default(),
            container: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
            name: name.clone(),
            output_link: Default::default(),
            visible: Cell::new(true),
            fullscreen: Default::default(),
        });
        self.state.workspaces.set(name, workspace.clone());
        workspace
            .output_link
            .set(Some(self.workspaces.add_last(workspace.clone())));
        self.show_workspace(&workspace);
        self.update_render_data();
        workspace
    }

    pub fn show_workspace(&self, ws: &Rc<WorkspaceNode>) -> bool {
        let mut seats = SmallVec::new();
        if let Some(old) = self.workspace.set(Some(ws.clone())) {
            if old.id == ws.id {
                return false;
            }
            collect_kb_foci2(old.clone(), &mut seats);
            old.set_visible(false);
            if old.is_empty() {
                old.clear();
                self.state.workspaces.remove(&old.name);
            }
        }
        ws.set_visible(true);
        if let Some(fs) = ws.fullscreen.get() {
            fs.tl_change_extents(&self.global.pos.get());
        }
        ws.change_extents(&self.workspace_rect());
        for seat in seats {
            ws.clone().node_do_focus(&seat, Direction::Unspecified);
        }
        true
    }

    pub fn create_workspace(self: &Rc<Self>, name: &str) -> Rc<WorkspaceNode> {
        let ws = Rc::new(WorkspaceNode {
            id: self.state.node_ids.next(),
            output: CloneCell::new(self.clone()),
            position: Cell::new(Default::default()),
            container: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
            name: name.to_string(),
            output_link: Cell::new(None),
            visible: Cell::new(false),
            fullscreen: Default::default(),
        });
        ws.output_link
            .set(Some(self.workspaces.add_last(ws.clone())));
        self.state.workspaces.set(name.to_string(), ws.clone());
        if self.workspace.get().is_none() {
            self.show_workspace(&ws);
        }
        self.update_render_data();
        ws
    }

    fn workspace_rect(&self) -> Rect {
        let rect = self.global.pos.get();
        let th = self.state.theme.sizes.title_height.get();
        Rect::new_sized(
            rect.x1(),
            rect.y1() + th + 1,
            rect.width(),
            rect.height().sub(th + 1).max(0),
        )
        .unwrap()
    }

    pub fn set_position(&self, x: i32, y: i32) {
        let pos = self.global.pos.get();
        if (pos.x1(), pos.y1()) == (x, y) {
            return;
        }
        let rect = pos.at_point(x, y);
        self.change_extents_(&rect);
    }

    pub fn update_mode(&self, mode: Mode) {
        if self.global.mode.get() == mode {
            return;
        }
        self.global.mode.set(mode);
        let pos = self.global.pos.get();
        let rect = Rect::new_sized(pos.x1(), pos.y1(), mode.width, mode.height).unwrap();
        self.change_extents_(&rect);
    }

    fn change_extents_(&self, rect: &Rect) {
        self.global.pos.set(*rect);
        self.state.root.update_extents();
        self.update_render_data();
        if let Some(ls) = self.lock_surface.get() {
            ls.change_extents(*rect);
        }
        if let Some(c) = self.workspace.get() {
            if let Some(fs) = c.fullscreen.get() {
                fs.tl_change_extents(rect);
            }
            c.change_extents(&self.workspace_rect());
        }
        for layer in &self.layers {
            for surface in layer.iter() {
                surface.compute_position();
            }
        }
        self.global.send_mode();
    }

    pub fn find_layer_surface_at(
        &self,
        x: i32,
        y: i32,
        layers: &[u32],
        tree: &mut Vec<FoundNode>,
    ) -> FindTreeResult {
        let len = tree.len();
        for layer in layers.iter().copied() {
            for surface in self.layers[layer as usize].rev_iter() {
                let pos = surface.output_position();
                if pos.contains(x, y) {
                    let (x, y) = pos.translate(x, y);
                    if surface.node_find_tree_at(x, y, tree) == FindTreeResult::AcceptsInput {
                        return FindTreeResult::AcceptsInput;
                    }
                    tree.truncate(len);
                }
            }
        }
        FindTreeResult::Other
    }

    pub fn set_status(&self, status: &Rc<String>) {
        self.status.set(status.clone());
        self.update_render_data();
    }

    fn pointer_move(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>, x: i32, y: i32) {
        self.pointer_positions.set(seat.id(), (x, y));
    }
}

pub struct OutputTitle {
    pub x1: i32,
    pub x2: i32,
    pub tex_x: i32,
    pub tex_y: i32,
    pub tex: Rc<Texture>,
    pub ws: Rc<WorkspaceNode>,
}

pub struct OutputStatus {
    pub tex_x: i32,
    pub tex_y: i32,
    pub tex: Rc<Texture>,
}

#[derive(Default)]
pub struct OutputRenderData {
    pub active_workspace: Option<Rect>,
    pub underline: Rect,
    pub inactive_workspaces: Vec<Rect>,
    pub titles: Vec<OutputTitle>,
    pub status: Option<OutputStatus>,
}

impl Debug for OutputNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputNode").finish_non_exhaustive()
    }
}

impl Node for OutputNode {
    fn node_id(&self) -> NodeId {
        self.id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_output(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        if let Some(ls) = self.lock_surface.get() {
            visitor.visit_lock_surface(&ls);
        }
        for ws in self.workspaces.iter() {
            visitor.visit_workspace(ws.deref());
        }
        for layers in &self.layers {
            for surface in layers.iter() {
                visitor.visit_layer_surface(surface.deref());
            }
        }
    }

    fn node_visible(&self) -> bool {
        true
    }

    fn node_absolute_position(&self) -> Rect {
        self.global.pos.get()
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, direction: Direction) {
        if self.state.lock.locked.get() {
            if let Some(lock) = self.lock_surface.get() {
                seat.focus_node(lock.surface.clone());
            }
            return;
        }
        if let Some(ws) = self.workspace.get() {
            ws.node_do_focus(seat, direction);
        }
    }

    fn node_find_tree_at(&self, x: i32, mut y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if self.state.lock.locked.get() {
            if let Some(ls) = self.lock_surface.get() {
                tree.push(FoundNode {
                    node: ls.clone(),
                    x,
                    y,
                });
                return ls.node_find_tree_at(x, y, tree);
            }
            return FindTreeResult::AcceptsInput;
        }
        if let Some(ws) = self.workspace.get() {
            if let Some(fs) = ws.fullscreen.get() {
                tree.push(FoundNode {
                    node: fs.clone().tl_into_node(),
                    x,
                    y,
                });
                return fs.tl_as_node().node_find_tree_at(x, y, tree);
            }
        }
        {
            let res = self.find_layer_surface_at(x, y, &[OVERLAY, TOP], tree);
            if res.accepts_input() {
                return res;
            }
        }
        {
            let (x_abs, y_abs) = self.global.pos.get().translate_inv(x, y);
            for stacked in self.state.root.stacked.rev_iter() {
                let ext = stacked.node_absolute_position();
                if stacked.stacked_absolute_position_constrains_input()
                    && !ext.contains(x_abs, y_abs)
                {
                    // TODO: make constrain always true
                    continue;
                }
                let (x, y) = ext.translate(x_abs, y_abs);
                let idx = tree.len();
                tree.push(FoundNode {
                    node: stacked.deref().clone().stacked_into_node(),
                    x,
                    y,
                });
                match stacked.node_find_tree_at(x, y, tree) {
                    FindTreeResult::AcceptsInput => {
                        return FindTreeResult::AcceptsInput;
                    }
                    FindTreeResult::Other => {
                        tree.truncate(idx);
                    }
                }
            }
        }
        let bar_height = self.state.theme.sizes.title_height.get() + 1;
        if y >= bar_height {
            y -= bar_height;
            let len = tree.len();
            if let Some(ws) = self.workspace.get() {
                tree.push(FoundNode {
                    node: ws.clone(),
                    x,
                    y,
                });
                ws.node_find_tree_at(x, y, tree);
            }
            if tree.len() == len {
                self.find_layer_surface_at(x, y, &[BOTTOM, BACKGROUND], tree);
            }
        }
        FindTreeResult::AcceptsInput
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_output(self, x, y);
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        button: u32,
        state: KeyState,
        _serial: u32,
    ) {
        if state != KeyState::Pressed || button != BTN_LEFT {
            return;
        }
        let (x, y) = match self.pointer_positions.get(&seat.id()) {
            Some(p) => p,
            _ => return,
        };
        if y >= self.state.theme.sizes.title_height.get() {
            return;
        }
        let ws = 'ws: {
            let rd = self.render_data.borrow_mut();
            for title in &rd.titles {
                if x >= title.x1 && x < title.x2 {
                    break 'ws title.ws.clone();
                }
            }
            return;
        };
        self.show_workspace(&ws);
        self.update_render_data();
        self.state.tree_changed();
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        let steps = match self.scroll.handle(event) {
            Some(e) => e,
            _ => return,
        };
        if steps == 0 {
            return;
        }
        let ws = match self.workspace.get() {
            Some(ws) => ws,
            _ => return,
        };
        let mut ws = 'ws: {
            for r in self.workspaces.iter() {
                if r.id == ws.id {
                    break 'ws r;
                }
            }
            return;
        };
        for _ in 0..steps.abs() {
            let new = if steps < 0 { ws.prev() } else { ws.next() };
            ws = match new {
                Some(n) => n,
                None => break,
            };
        }
        if !self.show_workspace(&ws) {
            return;
        }
        ws.deref()
            .clone()
            .node_do_focus(seat, Direction::Unspecified);
        self.update_render_data();
        self.state.tree_changed();
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("output focus");
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        self.pointer_move(seat, x.round_down(), y.round_down());
    }

    fn node_into_output(self: Rc<Self>) -> Option<Rc<OutputNode>> {
        Some(self.clone())
    }

    fn node_is_output(&self) -> bool {
        true
    }
}
