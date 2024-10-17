use {
    crate::{
        client::{Client, ClientError, ClientId},
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::{
                xdg_surface::xdg_popup::{XdgPopup, XdgPopupParent},
                PendingState, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        tree::{
            FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeVisitor, OutputNode,
            StackedNode,
        },
        utils::{
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode},
            numcell::NumCell,
        },
        wire::{ext_tray_item_v1::*, ExtTrayItemV1Id, XdgPopupId},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

tree_id!(TrayItemNodeId);
pub struct ExtTrayItemV1 {
    pub id: ExtTrayItemV1Id,
    node_id: TrayItemNodeId,
    seat_state: NodeSeatState,
    pub client: Rc<Client>,
    visible: Cell<bool>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    version: Version,
    output: Rc<OutputGlobalOpt>,
    attached: Cell<bool>,
    sent_serial: NumCell<u32>,
    ack_serial: NumCell<u32>,
    linked_node: Cell<Option<LinkedNode<Rc<Self>>>>,
    popups: CopyHashMap<XdgPopupId, Rc<Popup>>,
    abs_pos: Cell<Rect>,
    pub rel_pos: Cell<Rect>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum FocusHint {
    None,
    OnDemand,
    Immediate,
}

impl ExtTrayItemV1 {
    pub fn new(
        id: ExtTrayItemV1Id,
        version: Version,
        surface: &Rc<WlSurface>,
        output: &Rc<OutputGlobalOpt>,
    ) -> Self {
        Self {
            id,
            node_id: surface.client.state.node_ids.next(),
            seat_state: Default::default(),
            client: surface.client.clone(),
            visible: Cell::new(surface.client.state.root_visible()),
            surface: surface.clone(),
            tracker: Default::default(),
            version,
            output: output.clone(),
            attached: Default::default(),
            sent_serial: Default::default(),
            ack_serial: Default::default(),
            linked_node: Default::default(),
            popups: Default::default(),
            abs_pos: Default::default(),
            rel_pos: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), ExtTrayItemV1Error> {
        self.surface.set_role(SurfaceRole::TrayItem)?;
        if self.surface.ext.get().is_some() {
            return Err(ExtTrayItemV1Error::Exists);
        }
        self.surface.ext.set(self.clone());
        self.surface.set_visible(false);
        if let Some(node) = self.output.node() {
            self.surface.set_output(&node);
            self.send_current_configure();
        }
        Ok(())
    }

    pub fn set_position(&self, abs_pos: Rect, rel_pos: Rect) {
        self.surface
            .set_absolute_position(abs_pos.x1(), abs_pos.y1());
        self.rel_pos.set(rel_pos);
        if self.abs_pos.replace(abs_pos) != abs_pos {
            for popup in self.popups.lock().values() {
                popup.popup.update_absolute_position();
            }
        }
    }

    pub fn send_current_configure(&self) {
        let size = self
            .client
            .state
            .theme
            .sizes
            .title_height
            .get()
            .saturating_sub(2)
            .max(1);
        self.send_configure_size(size, size);
        self.send_configure();
    }

    fn send_configure_size(&self, width: i32, height: i32) {
        self.client.event(ConfigureSize {
            self_id: self.id,
            width,
            height,
        });
    }

    fn send_configure(&self) {
        self.client.event(Configure {
            self_id: self.id,
            serial: self.sent_serial.add_fetch(1),
        });
    }

    pub fn destroy_popups(&self) {
        for popup in self.popups.lock().drain_values() {
            popup.popup.destroy_node();
        }
    }

    pub fn destroy_node(&self) {
        self.linked_node.take();
        self.attached.set(false);
        self.destroy_popups();
        self.surface.destroy_node();
        self.seat_state.destroy_node(self);
        self.client.state.tree_changed();
        if let Some(node) = self.output.node() {
            node.update_tray_positions();
        }
    }

    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        let visible = visible && self.surface.buffer.is_some();
        self.surface.set_visible(visible);
        if !visible {
            self.destroy_popups();
        }
    }
}

impl ExtTrayItemV1RequestHandler for ExtTrayItemV1 {
    type Error = ExtTrayItemV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.popups.is_not_empty() {
            return Err(ExtTrayItemV1Error::HasPopups);
        }
        self.destroy_node();
        self.surface.unset_ext();
        Ok(())
    }

    fn ack_configure(&self, req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().tray_item_ack_serial = Some(req.serial);
        Ok(())
    }

    fn get_popup(&self, req: GetPopup, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let popup = self.client.lookup(req.popup)?;
        let seat = self.client.lookup(req.seat)?;
        let seat = &seat.global;
        let Some(serial) = self.client.map_serial(req.serial) else {
            return Err(ExtTrayItemV1Error::InvalidSerial);
        };
        if popup.parent.is_some() {
            return Err(ExtTrayItemV1Error::PopupHasParent);
        }
        let focus = match req.keyboard_focus {
            0 => FocusHint::None,
            1 => FocusHint::OnDemand,
            2 => FocusHint::Immediate,
            n => return Err(ExtTrayItemV1Error::InvalidFocusHint(n)),
        };
        let Some(node) = self.output.node() else {
            popup.destroy_node();
            return Ok(());
        };
        seat.add_tray_item_popup(slf, &popup);
        let stack = self.client.state.root.stacked_above_layers.clone();
        popup.xdg.set_popup_stack(&stack);
        popup.xdg.set_output(&node);
        let user = Rc::new(Popup {
            parent: slf.clone(),
            popup: popup.clone(),
            seat: seat.clone(),
            serial,
            focus,
            stack,
            stack_link: Default::default(),
        });
        popup.parent.set(Some(user.clone()));
        self.popups.set(popup.id, user);
        Ok(())
    }
}

struct Popup {
    parent: Rc<ExtTrayItemV1>,
    popup: Rc<XdgPopup>,
    seat: Rc<WlSeatGlobal>,
    serial: u64,
    focus: FocusHint,
    stack: Rc<LinkedList<Rc<dyn StackedNode>>>,
    stack_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
}

impl XdgPopupParent for Popup {
    fn position(&self) -> Rect {
        self.parent.abs_pos.get()
    }

    fn remove_popup(&self) {
        self.seat.remove_tray_item_popup(&self.parent, &self.popup);
        self.parent.popups.remove(&self.popup.id);
    }

    fn output(&self) -> Rc<OutputNode> {
        self.parent.surface.output.get()
    }

    fn has_workspace_link(&self) -> bool {
        false
    }

    fn post_commit(&self) {
        let mut dl = self.stack_link.borrow_mut();
        let surface = &self.popup.xdg.surface;
        let state = &surface.client.state;
        if surface.buffer.is_some() {
            if dl.is_none() {
                if self.parent.surface.visible.get() {
                    self.popup.set_visible(true);
                    *dl = Some(self.stack.add_last(self.popup.clone()));
                    state.tree_changed();
                    if self.focus == FocusHint::Immediate {
                        self.seat.handle_focus_request(
                            &self.parent.client,
                            self.popup.xdg.surface.clone(),
                            self.serial,
                        );
                    }
                } else {
                    self.popup.destroy_node();
                }
            }
        } else {
            if dl.take().is_some() {
                drop(dl);
                self.popup.set_visible(false);
                self.popup.destroy_node();
            }
        }
    }

    fn tray_item(&self) -> Option<Rc<ExtTrayItemV1>> {
        Some(self.parent.clone())
    }

    fn allow_popup_focus(&self) -> bool {
        match self.focus {
            FocusHint::None => false,
            FocusHint::OnDemand => true,
            FocusHint::Immediate => true,
        }
    }
}

impl SurfaceExt for ExtTrayItemV1 {
    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        if let Some(serial) = pending.tray_item_ack_serial.take() {
            self.ack_serial.set(serial);
        }
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>) {
        if self.ack_serial.get() != self.sent_serial.get() {
            return;
        }
        if self.surface.visible.get() {
            if self.surface.buffer.is_none() {
                self.destroy_node();
            }
        } else {
            if self.surface.buffer.is_some() {
                self.surface.set_visible(self.visible.get());
                if let Some(node) = self.output.node() {
                    if !self.attached.replace(true) {
                        let link = node.tray_items.add_last(self.clone());
                        self.linked_node.set(Some(link));
                        node.update_tray_positions();
                    }
                }
            }
        }
    }

    fn extents_changed(&self) {
        if self.surface.visible.get() {
            self.client.state.tree_changed();
        }
    }

    fn tray_item(self: Rc<Self>) -> Option<Rc<ExtTrayItemV1>> {
        Some(self)
    }
}

impl Node for ExtTrayItemV1 {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_tray_item(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        self.surface.clone().node_visit(visitor);
    }

    fn node_visible(&self) -> bool {
        self.surface.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.surface.node_absolute_position()
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        _usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        self.surface.find_tree_at_(x, y, tree)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.client.clone())
    }

    fn node_client_id(&self) -> Option<ClientId> {
        Some(self.client.id)
    }
}

object_base! {
    self = ExtTrayItemV1;
    version = self.version;
}

impl Object for ExtTrayItemV1 {
    fn break_loops(&self) {
        self.destroy_node();
    }
}

simple_add_obj!(ExtTrayItemV1);

#[derive(Debug, Error)]
pub enum ExtTrayItemV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a tray item role object")]
    Exists,
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
    #[error("Popup already has a parent")]
    PopupHasParent,
    #[error("Surface still has popups")]
    HasPopups,
    #[error("The serial is not valid")]
    InvalidSerial,
    #[error("The focus hint {} is invalid", .0)]
    InvalidFocusHint(u32),
}
efrom!(ExtTrayItemV1Error, ClientError);
