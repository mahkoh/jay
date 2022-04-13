use {
    crate::{
        backend::Mode,
        cursor::KnownCursor,
        ifs::{
            wl_output::WlOutputGlobal,
            wl_seat::{collect_kb_foci2, NodeSeatState, WlSeatGlobal},
            wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
            zwlr_layer_shell_v1::{BACKGROUND, BOTTOM},
        },
        rect::Rect,
        render::{Renderer, Texture},
        state::State,
        text,
        theme::Color,
        tree::{
            walker::NodeVisitor, FindTreeResult, FoundNode, Node, NodeId, SizedNode, WorkspaceNode,
        },
        utils::{clonecell::CloneCell, errorfmt::ErrorFmt, linkedlist::LinkedList},
    },
    jay_config::Direction,
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
}

impl OutputNode {
    pub fn update_render_data(&self) {
        let mut rd = self.render_data.borrow_mut();
        rd.titles.clear();
        rd.inactive_workspaces.clear();
        rd.active_workspace = None;
        rd.status = None;
        let mut pos = 0;
        let font = self.state.theme.font.borrow_mut();
        let th = self.state.theme.title_height.get();
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
                    let title =
                        match text::render_fitting(&ctx, th, &font, &ws.name, Color::GREY, false) {
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
                        x,
                        y: 0,
                        tex: title,
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
            let title = match text::render_fitting(&ctx, th, &font, &status, Color::GREY, true) {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Could not render status {}: {}", status, ErrorFmt(e));
                    break 'set_status;
                }
            };
            let pos = width - title.width() - 1;
            rd.status = Some(OutputTitle {
                x: pos,
                y: 0,
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
            old.node_set_visible(false);
        }
        ws.node_set_visible(true);
        ws.change_extents(&self.workspace_rect());
        let node = ws.last_active_child();
        for seat in seats {
            node.clone().node_do_focus(&seat, Direction::Unspecified);
        }
        true
    }

    fn workspace_rect(&self) -> Rect {
        let rect = self.global.pos.get();
        let th = self.state.theme.title_height.get();
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
        if let Some(c) = self.workspace.get() {
            c.node_change_extents(&self.workspace_rect());
        }
        for layer in &self.layers {
            for surface in layer.iter() {
                surface.deref().clone().node_change_extents(&rect);
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
}

pub struct OutputTitle {
    pub x: i32,
    pub y: i32,
    pub tex: Rc<Texture>,
}

#[derive(Default)]
pub struct OutputRenderData {
    pub active_workspace: Option<Rect>,
    pub underline: Rect,
    pub inactive_workspaces: Vec<Rect>,
    pub titles: Vec<OutputTitle>,
    pub status: Option<OutputTitle>,
}

impl Debug for OutputNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputNode").finish_non_exhaustive()
    }
}

impl SizedNode for OutputNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.state.root.remove_child(self);
        }
        self.workspace.set(None);
        let workspaces: Vec<_> = self.workspaces.iter().map(|e| e.deref().clone()).collect();
        for workspace in workspaces {
            workspace.node_destroy(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_output(self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        for ws in self.workspaces.iter() {
            visitor.visit_workspace(ws.deref());
        }
        for layers in &self.layers {
            for surface in layers.iter() {
                visitor.visit_layer_surface(surface.deref());
            }
        }
    }

    fn visible(&self) -> bool {
        true
    }

    fn last_active_child(self: &Rc<Self>) -> Rc<dyn Node> {
        if let Some(ws) = self.workspace.get() {
            return ws.last_active_child();
        }
        self.clone()
    }

    fn absolute_position(&self) -> Rect {
        self.global.pos.get()
    }

    fn find_tree_at(&self, x: i32, mut y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let bar_height = self.state.theme.title_height.get() + 1;
        if y > bar_height {
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

    fn remove_child(self: &Rc<Self>, _child: &dyn Node) {
        unimplemented!();
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_output(self, x, y);
    }

    fn is_output(&self) -> bool {
        true
    }

    fn into_output(self: &Rc<Self>) -> Option<Rc<OutputNode>> {
        Some(self.clone())
    }
}
