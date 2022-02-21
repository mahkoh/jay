use crate::client::{Client, ClientError};
use crate::cursor::KnownCursor;
use crate::fixed::Fixed;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceError, XdgSurfaceExt};
use crate::ifs::xdg_positioner::{XdgPositioned, XdgPositioner, CA};
use crate::leaks::Tracker;
use crate::object::Object;
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::walker::NodeVisitor;
use crate::tree::{FindTreeResult, FoundNode, Node, NodeId, WorkspaceNode};
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::LinkedNode;
use crate::wire::xdg_popup::*;
use crate::wire::XdgPopupId;
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[allow(dead_code)]
const INVALID_GRAB: u32 = 1;

tree_id!(PopupId);

pub struct XdgPopup {
    id: XdgPopupId,
    node_id: PopupId,
    pub xdg: Rc<XdgSurface>,
    pub(super) parent: CloneCell<Option<Rc<XdgSurface>>>,
    relative_position: Cell<Rect>,
    display_link: RefCell<Option<LinkedNode<Rc<dyn Node>>>>,
    workspace_link: RefCell<Option<LinkedNode<Rc<dyn Node>>>>,
    pos: RefCell<XdgPositioned>,
    pub tracker: Tracker<Self>,
}

impl Debug for XdgPopup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XdgPopup").finish_non_exhaustive()
    }
}

impl XdgPopup {
    pub fn new(
        id: XdgPopupId,
        xdg: &Rc<XdgSurface>,
        parent: Option<&Rc<XdgSurface>>,
        pos: &Rc<XdgPositioner>,
    ) -> Result<Self, XdgPopupError> {
        let pos = pos.value();
        if !pos.is_complete() {
            return Err(XdgPopupError::Incomplete);
        }
        Ok(Self {
            id,
            node_id: xdg.surface.client.state.node_ids.next(),
            xdg: xdg.clone(),
            parent: CloneCell::new(parent.cloned()),
            relative_position: Cell::new(Default::default()),
            display_link: RefCell::new(None),
            workspace_link: RefCell::new(None),
            pos: RefCell::new(pos),
            tracker: Default::default(),
        })
    }

    fn send_configure(&self, x: i32, y: i32, width: i32, height: i32) {
        self.xdg.surface.client.event(Configure {
            self_id: self.id,
            x,
            y,
            width,
            height,
        })
    }

    fn send_repositioned(&self, token: u32) {
        self.xdg.surface.client.event(Repositioned {
            self_id: self.id,
            token,
        })
    }

    fn send_popup_done(&self) {
        self.xdg
            .surface
            .client
            .event(PopupDone { self_id: self.id })
    }

    fn update_position(&self, parent: &XdgSurface) -> Result<(), XdgPopupError> {
        // let parent = parent.extents.get();
        let positioner = self.pos.borrow_mut();
        // if !parent.contains_rect(&positioner.ar) {
        //     return Err(XdgPopupError::AnchorRectOutside);
        // }
        let parent_abs = parent.absolute_desired_extents.get();
        let mut rel_pos = positioner.get_position(false, false);
        let mut abs_pos = rel_pos.move_(parent_abs.x1(), parent_abs.y1());
        if let Some(ws) = parent.workspace.get() {
            let output_pos = ws.output.get().position.get();
            let mut overflow = output_pos.get_overflow(&abs_pos);
            if !overflow.is_contained() {
                let mut flip_x = positioner.ca.contains(CA::FLIP_X) && overflow.x_overflow();
                let mut flip_y = positioner.ca.contains(CA::FLIP_Y) && overflow.y_overflow();
                if flip_x || flip_y {
                    let mut adj_rel = positioner.get_position(flip_x, flip_y);
                    let mut adj_abs = adj_rel.move_(parent_abs.x1(), parent_abs.y1());
                    let mut adj_overflow = output_pos.get_overflow(&adj_abs);
                    let mut recalculate = false;
                    if flip_x && adj_overflow.x_overflow() {
                        flip_x = false;
                        recalculate = true;
                    }
                    if flip_y && adj_overflow.y_overflow() {
                        flip_y = false;
                        recalculate = true;
                    }
                    if flip_x || flip_y {
                        if recalculate {
                            adj_rel = positioner.get_position(flip_x, flip_y);
                            adj_abs = adj_rel.move_(parent_abs.x1(), parent_abs.y1());
                            adj_overflow = output_pos.get_overflow(&adj_abs);
                        }
                        rel_pos = adj_rel;
                        abs_pos = adj_abs;
                        overflow = adj_overflow;
                    }
                }
                let (mut dx, mut dy) = (0, 0);
                if positioner.ca.contains(CA::SLIDE_X) && overflow.x_overflow() {
                    dx = if overflow.left > 0 || overflow.left + overflow.right > 0 {
                        parent_abs.x1() - abs_pos.x1()
                    } else {
                        parent_abs.x2() - abs_pos.x2()
                    };
                }
                if positioner.ca.contains(CA::SLIDE_Y) && overflow.y_overflow() {
                    dy = if overflow.top > 0 || overflow.top + overflow.bottom > 0 {
                        parent_abs.y1() - abs_pos.y1()
                    } else {
                        parent_abs.y2() - abs_pos.y2()
                    };
                }
                if dx != 0 || dy != 0 {
                    rel_pos = rel_pos.move_(dx, dy);
                    abs_pos = rel_pos.move_(parent_abs.x1(), parent_abs.y1());
                    overflow = output_pos.get_overflow(&abs_pos);
                }
                let (mut dx1, mut dx2, mut dy1, mut dy2) = (0, 0, 0, 0);
                if positioner.ca.contains(CA::RESIZE_X) {
                    dx1 = overflow.left.max(0);
                    dx2 = -overflow.right.max(0);
                }
                if positioner.ca.contains(CA::RESIZE_Y) {
                    dy1 = overflow.top.max(0);
                    dy2 = -overflow.bottom.max(0);
                }
                if dx1 > 0 || dx2 < 0 || dy1 > 0 || dy2 < 0 {
                    abs_pos = Rect::new(
                        abs_pos.x1() + dx1,
                        abs_pos.y1() + dy1,
                        abs_pos.x2() + dx2,
                        abs_pos.y2() + dy2,
                    )
                    .unwrap();
                    rel_pos = Rect::new_sized(
                        abs_pos.x1() - parent_abs.x1(),
                        abs_pos.y1() - parent_abs.y1(),
                        abs_pos.width(),
                        abs_pos.height(),
                    )
                    .unwrap();
                }
            }
        }
        self.relative_position.set(rel_pos);
        self.xdg.set_absolute_desired_extents(&abs_pos);
        Ok(())
    }

    pub fn update_absolute_position(&self) {
        if let Some(parent) = self.parent.get() {
            let rel = self.relative_position.get();
            let parent = parent.absolute_desired_extents.get();
            self.xdg
                .set_absolute_desired_extents(&rel.move_(parent.x1(), parent.y1()));
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.xdg.surface.client.parse(self, parser)?;
        self.destroy_node(true);
        {
            if let Some(parent) = self.parent.take() {
                parent.popups.remove(&self.id);
            }
        }
        self.xdg.ext.set(None);
        self.xdg.surface.client.remove_obj(self)?;
        *self.display_link.borrow_mut() = None;
        *self.workspace_link.borrow_mut() = None;
        Ok(())
    }

    fn grab(&self, parser: MsgParser<'_, '_>) -> Result<(), GrabError> {
        let _req: Grab = self.xdg.surface.client.parse(self, parser)?;
        Ok(())
    }

    fn reposition(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), RepositionError> {
        let req: Reposition = self.xdg.surface.client.parse(&**self, parser)?;
        *self.pos.borrow_mut() = self.xdg.surface.client.lookup(req.positioner)?.value();
        if let Some(parent) = self.parent.get() {
            self.update_position(&parent)?;
            let rel = self.relative_position.get();
            self.send_repositioned(req.token);
            self.send_configure(rel.x1(), rel.y1(), rel.width(), rel.height());
            self.xdg.do_send_configure();
        }
        Ok(())
    }

    fn get_parent_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.parent.get()?.workspace.get()
    }
}

object_base! {
    XdgPopup, XdgPopupError;

    DESTROY => destroy,
    GRAB => grab,
    REPOSITION => reposition,
}

impl Object for XdgPopup {
    fn num_requests(&self) -> u32 {
        let last_req = match self.xdg.base.version {
            0..=2 => GRAB,
            _ => REPOSITION,
        };
        last_req + 1
    }

    fn break_loops(&self) {
        self.destroy_node(true);
        self.parent.set(None);
        *self.display_link.borrow_mut() = None;
        *self.workspace_link.borrow_mut() = None;
    }
}

simple_add_obj!(XdgPopup);

impl Node for XdgPopup {
    fn id(&self) -> NodeId {
        self.node_id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.xdg.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let _v = self.display_link.borrow_mut().take();
        let _v = self.workspace_link.borrow_mut().take();
        self.xdg.destroy_node();
        self.xdg.seat_state.destroy_node(self);
    }

    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_popup(&self);
    }

    fn visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.xdg.surface);
    }

    fn absolute_position(&self) -> Rect {
        self.xdg.absolute_desired_extents.get()
    }

    fn absolute_position_constrains_input(&self) -> bool {
        false
    }

    fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.xdg.find_tree_at(x, y, tree)
    }

    fn pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_popup(&self);
    }

    fn pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        seat.set_known_cursor(KnownCursor::Default);
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_xdg_surface(&self.xdg, x, y)
    }

    fn set_workspace(self: Rc<Self>, ws: &Rc<WorkspaceNode>) {
        self.xdg.set_workspace(ws);
    }

    fn client(&self) -> Option<Rc<Client>> {
        Some(self.xdg.surface.client.clone())
    }
}

impl XdgSurfaceExt for XdgPopup {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        if let Some(parent) = self.parent.get() {
            self.update_position(&parent)?;
            let rel = self.relative_position.get();
            self.send_configure(rel.x1(), rel.y1(), rel.width(), rel.height());
        }
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        let mut wl = self.workspace_link.borrow_mut();
        let mut dl = self.display_link.borrow_mut();
        let ws = match self.get_parent_workspace() {
            Some(ws) => ws,
            _ => {
                log::info!("no ws");
                return;
            }
        };
        let surface = &self.xdg.surface;
        let state = &surface.client.state;
        if surface.buffer.get().is_some() {
            if wl.is_none() {
                self.xdg.set_workspace(&ws);
                *wl = Some(ws.stacked.add_last(self.clone()));
                *dl = Some(state.root.stacked.add_last(self.clone()));
                state.tree_changed();
            }
        } else {
            if wl.take().is_some() {
                self.destroy_node(true);
                self.send_popup_done();
            }
        }
    }

    fn into_node(self: Rc<Self>) -> Option<Rc<dyn Node>> {
        Some(self)
    }

    fn extents_changed(&self) {
        self.xdg.surface.client.state.tree_changed();
    }
}

#[derive(Debug, Error)]
pub enum XdgPopupError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `grab` request")]
    GrabError(#[from] GrabError),
    #[error("Could not process `reposition` request")]
    RepositionError(#[from] RepositionError),
    #[error("The `xdg_positioner` is incomplete")]
    Incomplete,
    #[error("The anchor rectangle of the `xdg_positioner` extends outside the parent")]
    #[allow(dead_code)]
    AnchorRectOutside,
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum GrabError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GrabError, ParseFailed, MsgParserError);
efrom!(GrabError, ClientError);

#[derive(Debug, Error)]
pub enum RepositionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    XdgPopupError(Box<XdgPopupError>),
}
efrom!(RepositionError, ParseFailed, MsgParserError);
efrom!(RepositionError, ClientError);
efrom!(RepositionError, XdgPopupError);
