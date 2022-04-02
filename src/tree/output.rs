use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use crate::rect::Rect;
use crate::render::{Renderer, Texture};
use crate::state::State;
use crate::text;
use crate::theme::Color;
use crate::tree::walker::NodeVisitor;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::clonecell::CloneCell;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::linkedlist::LinkedList;
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, Sub};
use std::rc::Rc;

tree_id!(OutputNodeId);
pub struct OutputNode {
    pub id: OutputNodeId,
    pub position: Cell<Rect>,
    pub global: Rc<WlOutputGlobal>,
    pub workspaces: LinkedList<Rc<WorkspaceNode>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub seat_state: NodeSeatState,
    pub layers: [LinkedList<Rc<ZwlrLayerSurfaceV1>>; 4],
    pub render_data: RefCell<OutputRenderData>,
    pub state: Rc<State>,
    pub is_dummy: bool,
}

impl OutputNode {
    pub fn update_render_data(&self) {
        let mut rd = self.render_data.borrow_mut();
        rd.titles.clear();
        rd.inactive_workspaces.clear();
        let mut pos = 0;
        let font = self.state.theme.font.borrow_mut();
        let th = self.state.theme.title_height.get();
        let active_id = self.workspace.get().map(|w| w.id);
        for ws in self.workspaces.iter() {
            let mut title_width = th;
            'create_texture: {
                if let Some(ctx) = self.state.render_ctx.get() {
                    if th == 0 || ws.name.is_empty() {
                        break 'create_texture;
                    }
                    let title = match text::render_fitting(&ctx, th, &font, &ws.name, Color::GREY) {
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
                rd.active_workspace = rect;
            } else {
                rd.inactive_workspaces.push(rect);
            }
            pos += title_width;
        }
    }

    pub fn show_workspace(&self, ws: &Rc<WorkspaceNode>) {
        self.workspace.set(Some(ws.clone()));
        ws.clone().change_extents(&self.workspace_rect());
    }

    fn workspace_rect(&self) -> Rect {
        let rect = self.position.get();
        let th = self.state.theme.title_height.get();
        Rect::new_sized(
            rect.x1(),
            rect.y1() + th,
            rect.width(),
            rect.height().sub(th).max(0),
        )
        .unwrap()
    }

    pub fn change_size(&self, width: i32, height: i32) {
        let pos = self.position.get();
        let rect = Rect::new_sized(pos.x1(), pos.y1(), width, height).unwrap();
        self.position.set(rect);
        if let Some(c) = self.workspace.get() {
            c.change_extents(&self.workspace_rect());
        }
        for layer in &self.layers {
            for surface in layer.iter() {
                surface.deref().clone().change_extents(&rect);
            }
        }
    }
}

pub struct OutputTitle {
    pub x: i32,
    pub y: i32,
    pub tex: Rc<Texture>,
}

#[derive(Default)]
pub struct OutputRenderData {
    pub active_workspace: Rect,
    pub inactive_workspaces: Vec<Rect>,
    pub titles: Vec<OutputTitle>,
}

impl Debug for OutputNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputNode").finish_non_exhaustive()
    }
}

impl Node for OutputNode {
    fn id(&self) -> NodeId {
        self.id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, detach: bool) {
        if detach {
            self.state.root.clone().remove_child(self);
        }
        self.workspace.set(None);
        let workspaces: Vec<_> = self.workspaces.iter().map(|e| e.deref().clone()).collect();
        for workspace in workspaces {
            workspace.destroy_node(false);
        }
        self.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_output(&self);
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

    fn absolute_position(&self) -> Rect {
        self.position.get()
    }

    fn find_tree_at(&self, x: i32, mut y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let th = self.state.theme.title_height.get();
        if y > th {
            y -= th;
            if let Some(ws) = self.workspace.get() {
                tree.push(FoundNode {
                    node: ws.clone(),
                    x,
                    y,
                });
                ws.find_tree_at(x, y, tree);
            }
        }
        FindTreeResult::AcceptsInput
    }

    fn remove_child(self: Rc<Self>, _child: &dyn Node) {
        unimplemented!();
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        seat.leave_output();
    }

    fn pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_output(&self)
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

    fn into_output(self: Rc<Self>) -> Option<Rc<OutputNode>> {
        Some(self)
    }
}
