use {
    crate::{
        client::{Client, ClientError},
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, SeatId, WlSeatGlobal, tablet::TabletTool},
            wl_surface::{
                tray::TrayItemId,
                xdg_surface::{XdgSurface, XdgSurfaceExt},
            },
            xdg_positioner::{
                CA_FLIP_X, CA_FLIP_Y, CA_RESIZE_X, CA_RESIZE_Y, CA_SLIDE_X, CA_SLIDE_Y,
                XdgPositioned, XdgPositioner,
            },
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        renderer::Renderer,
        tree::{
            Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeLayerLink,
            NodeLocation, NodeVisitor, OutputNode, StackedNode,
        },
        utils::{clonecell::CloneCell, smallmap::SmallMap},
        wire::{XdgPopupId, xdg_popup::*},
    },
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    thiserror::Error,
};

#[expect(dead_code)]
const INVALID_GRAB: u32 = 1;

tree_id!(PopupId);

pub trait XdgPopupParent {
    fn position(&self) -> Rect;
    fn remove_popup(&self);
    fn output(&self) -> Rc<OutputNode>;
    fn has_workspace_link(&self) -> bool;
    fn post_commit(&self);
    fn visible(&self) -> bool;
    fn make_visible(self: Rc<Self>);
    fn node_layer(&self) -> NodeLayerLink;
    fn tray_item(&self) -> Option<TrayItemId> {
        None
    }
    fn allow_popup_focus(&self) -> bool {
        false
    }
}

pub struct XdgPopup {
    pub id: XdgPopupId,
    node_id: PopupId,
    pub xdg: Rc<XdgSurface>,
    pub(in super::super) parent: CloneCell<Option<Rc<dyn XdgPopupParent>>>,
    relative_position: Cell<Rect>,
    pos: RefCell<XdgPositioned>,
    pub tracker: Tracker<Self>,
    seat_state: NodeSeatState,
    set_visible_prepared: Cell<bool>,
    interactive_moves: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
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
            parent: Default::default(),
            relative_position: Cell::new(Default::default()),
            pos: RefCell::new(pos),
            tracker: Default::default(),
            seat_state: Default::default(),
            set_visible_prepared: Cell::new(false),
            interactive_moves: Default::default(),
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

    fn update_position(&self, parent: &dyn XdgPopupParent) {
        let positioner = self.pos.borrow_mut();
        let parent_abs = parent.position();
        let mut rel_pos = positioner.get_position(false, false);
        let mut abs_pos = rel_pos.move_(parent_abs.x1(), parent_abs.y1());
        {
            let output_pos = parent.output().global.pos.get();
            let mut overflow = output_pos.get_overflow(&abs_pos);
            if !overflow.is_contained() {
                let mut flip_x = positioner.ca.contains(CA_FLIP_X) && overflow.x_overflow();
                let mut flip_y = positioner.ca.contains(CA_FLIP_Y) && overflow.y_overflow();
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
                if positioner.ca.contains(CA_SLIDE_X) && overflow.x_overflow() {
                    dx = if overflow.left + overflow.right > 0 {
                        parent_abs.x1() - abs_pos.x1()
                    } else if overflow.left > 0 {
                        overflow.left
                    } else {
                        -overflow.right
                    };
                }
                if positioner.ca.contains(CA_SLIDE_Y) && overflow.y_overflow() {
                    dy = if overflow.top + overflow.bottom > 0 {
                        parent_abs.y1() - abs_pos.y1()
                    } else if overflow.top > 0 {
                        overflow.top
                    } else {
                        -overflow.bottom
                    };
                }
                if dx != 0 || dy != 0 {
                    rel_pos = rel_pos.move_(dx, dy);
                    abs_pos = rel_pos.move_(parent_abs.x1(), parent_abs.y1());
                    overflow = output_pos.get_overflow(&abs_pos);
                }
                let (mut dx1, mut dx2, mut dy1, mut dy2) = (0, 0, 0, 0);
                if positioner.ca.contains(CA_RESIZE_X) {
                    dx1 = overflow.left.max(0);
                    dx2 = -overflow.right.max(0);
                }
                if positioner.ca.contains(CA_RESIZE_Y) {
                    dy1 = overflow.top.max(0);
                    dy2 = -overflow.bottom.max(0);
                }
                if dx1 > 0 || dx2 < 0 || dy1 > 0 || dy2 < 0 {
                    let maybe_abs_pos = Rect::new(
                        abs_pos.x1() + dx1,
                        abs_pos.y1() + dy1,
                        abs_pos.x2() + dx2,
                        abs_pos.y2() + dy2,
                    );
                    // If the popup is completely outside the output, this will fail. Just
                    // use its position as is.
                    if let Some(maybe_abs_pos) = maybe_abs_pos {
                        abs_pos = maybe_abs_pos;
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
        }
        self.relative_position.set(rel_pos);
        self.xdg.set_absolute_desired_extents(&abs_pos);
    }

    pub fn update_absolute_position(&self) {
        if let Some(parent) = self.parent.get() {
            let rel = self.relative_position.get();
            let parent = parent.position();
            self.xdg
                .set_absolute_desired_extents(&rel.move_(parent.x1(), parent.y1()));
        }
    }

    fn set_relative_position(&self, rel: Rect) {
        self.relative_position.set(rel);
        self.update_absolute_position();
        self.send_configure(rel.x1(), rel.y1(), rel.width(), rel.height());
        self.xdg.schedule_configure();
    }

    pub fn move_(&self, dx: i32, dy: i32) {
        let rel = self.relative_position.get().move_(dx, dy);
        self.set_relative_position(rel);
    }

    pub fn resize(&self, dx1: i32, dy1: i32, dx2: i32, dy2: i32) {
        let rel = self.relative_position.get();
        let rel = Rect::new(
            rel.x1() + dx1,
            rel.y1() + dy1,
            rel.x2() + dx2,
            rel.y2() + dy2,
        );
        let Some(rel) = rel else {
            return;
        };
        if rel.is_empty() {
            return;
        }
        self.set_relative_position(rel);
    }

    pub fn add_interactive_move(&self, seat: &Rc<WlSeatGlobal>) {
        self.interactive_moves.insert(seat.id(), seat.clone());
    }

    pub fn remove_interactive_move(&self, seat: &Rc<WlSeatGlobal>) {
        self.interactive_moves.remove(&seat.id());
    }
}

impl XdgPopupRequestHandler for XdgPopup {
    type Error = XdgPopupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy_node();
        self.xdg.unset_ext();
        self.xdg.surface.client.remove_obj(self)?;
        Ok(())
    }

    fn grab(&self, _req: Grab, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn reposition(&self, req: Reposition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.pos.borrow_mut() = self.xdg.surface.client.lookup(req.positioner)?.value();
        while let Some((_, seat)) = self.interactive_moves.pop() {
            seat.cancel_popup_move();
        }
        if let Some(parent) = self.parent.get() {
            self.update_position(&*parent);
            let rel = self.relative_position.get();
            self.send_repositioned(req.token);
            self.send_configure(rel.x1(), rel.y1(), rel.width(), rel.height());
            self.xdg.schedule_configure();
        }
        Ok(())
    }
}

impl XdgPopup {
    pub fn set_visible(&self, visible: bool) {
        let surface = &self.xdg.surface;
        let extents = surface.extents.get();
        let (x, y) = surface.buffer_abs_pos.get().position();
        surface.client.state.damage(extents.move_(x, y));

        // log::info!("set visible = {}", visible);
        self.set_visible_prepared.set(false);
        self.xdg.set_visible(visible);
        self.seat_state.set_visible(self, visible);
    }

    pub fn destroy_node(&self) {
        self.xdg.destroy_node();
        self.seat_state.destroy_node(self);
        if let Some(parent) = self.parent.take() {
            parent.remove_popup();
        }
        self.send_popup_done();
    }

    pub fn detach_node(&self) {
        self.xdg.detach_node();
        self.seat_state.destroy_node(self);
    }
}

object_base! {
    self = XdgPopup;
    version = self.xdg.base.version;
}

impl Object for XdgPopup {
    fn break_loops(&self) {
        self.destroy_node();
    }
}

dedicated_add_obj!(XdgPopup, XdgPopupId, xdg_popups);

impl Node for XdgPopup {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_popup(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.xdg.surface);
    }

    fn node_visible(&self) -> bool {
        self.xdg.surface.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.xdg.absolute_desired_extents.get()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        Some(self.xdg.surface.output.get())
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.xdg.surface.node_location()
    }

    fn node_layer(&self) -> NodeLayerLink {
        XdgSurfaceExt::node_layer(self)
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_node(self.xdg.surface.clone());
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        match usecase {
            FindTreeUsecase::None => {}
            FindTreeUsecase::SelectToplevel => return FindTreeResult::Other,
            FindTreeUsecase::SelectToplevelOrPopup => {
                let len = tree.len();
                let res = self.xdg.find_tree_at(x, y, tree);
                tree.truncate(len);
                return res;
            }
            FindTreeUsecase::SelectWorkspace => return FindTreeResult::Other,
        }
        self.xdg.find_tree_at(x, y, tree)
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, bounds: Option<&Rect>) {
        renderer.render_xdg_surface(&self.xdg, x, y, bounds)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.xdg.surface.client.clone())
    }

    fn node_make_visible(self: Rc<Self>) {
        if let Some(parent) = self.parent.get() {
            parent.make_visible();
        }
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.enter_popup(&self);
    }

    fn node_on_pointer_focus(&self, seat: &Rc<WlSeatGlobal>) {
        // log::info!("xdg-popup focus");
        seat.pointer_cursor().set_known(KnownCursor::Default);
    }

    fn node_on_tablet_tool_enter(
        self: Rc<Self>,
        tool: &Rc<TabletTool>,
        _time_usec: u64,
        _x: Fixed,
        _y: Fixed,
    ) {
        tool.cursor().set_known(KnownCursor::Default)
    }

    fn node_into_popup(self: Rc<Self>) -> Option<Rc<XdgPopup>> {
        Some(self)
    }
}

impl StackedNode for XdgPopup {
    fn stacked_prepare_set_visible(&self) {
        self.set_visible_prepared.set(true);
    }

    fn stacked_needs_set_visible(&self) -> bool {
        self.set_visible_prepared.get()
    }

    fn stacked_set_visible(&self, visible: bool) {
        if visible {
            if let Some(parent) = self.parent.get()
                && !parent.visible()
            {
                return;
            }
        }
        self.set_visible(visible);
    }

    fn stacked_has_workspace_link(&self) -> bool {
        match self.parent.get() {
            Some(p) => p.has_workspace_link(),
            _ => false,
        }
    }

    fn stacked_absolute_position_constrains_input(&self) -> bool {
        false
    }

    fn stacked_is_xdg_popup(&self) -> bool {
        true
    }
}

impl XdgSurfaceExt for XdgPopup {
    fn initial_configure(self: Rc<Self>) {
        if let Some(parent) = self.parent.get() {
            self.update_position(&*parent);
            let rel = self.relative_position.get();
            self.send_configure(rel.x1(), rel.y1(), rel.width(), rel.height());
        }
    }

    fn post_commit(self: Rc<Self>) {
        if let Some(parent) = self.parent.get() {
            parent.post_commit();
        }
    }

    fn extents_changed(&self) {
        self.xdg.surface.client.state.tree_changed();
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        if self.parent.get()?.allow_popup_focus() {
            return Some(self.xdg.surface.clone());
        }
        None
    }

    fn tray_item(&self) -> Option<TrayItemId> {
        self.parent.get()?.tray_item()
    }

    fn make_visible(self: Rc<Self>) {
        self.node_make_visible();
    }

    fn node_layer(&self) -> NodeLayerLink {
        let Some(parent) = self.parent.get() else {
            return NodeLayerLink::Display;
        };
        parent.node_layer()
    }
}

#[derive(Debug, Error)]
pub enum XdgPopupError {
    #[error("The `xdg_positioner` is incomplete")]
    Incomplete,
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(XdgPopupError, ClientError);
