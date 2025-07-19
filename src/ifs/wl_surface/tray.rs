use {
    crate::{
        client::{Client, ClientError, ClientId},
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::{
                PendingState, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError,
                xdg_surface::xdg_popup::{XdgPopup, XdgPopupParent},
            },
        },
        rect::Rect,
        tree::{
            FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeLayerLink, NodeLocation,
            NodeVisitor, OutputNode, StackedNode,
        },
        utils::{
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode},
            numcell::NumCell,
        },
        wire::{WlSeatId, XdgPopupId},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

pub mod jay_tray_item_v1;

tree_id!(TrayItemNodeId);
linear_ids!(TrayItemIds, TrayItemId, u64);

pub struct TrayItemData {
    node_id: TrayItemNodeId,
    pub tray_item_id: TrayItemId,
    seat_state: NodeSeatState,
    client: Rc<Client>,
    visible: Cell<bool>,
    pub surface: Rc<WlSurface>,
    output: Rc<OutputGlobalOpt>,
    attached: Cell<bool>,
    sent_serial: NumCell<u32>,
    ack_serial: NumCell<u32>,
    linked_node: Cell<Option<LinkedNode<Rc<dyn DynTrayItem>>>>,
    abs_pos: Cell<Rect>,
    pub rel_pos: Cell<Rect>,
}

impl TrayItemData {
    fn new(surface: &Rc<WlSurface>, output: &Rc<OutputGlobalOpt>) -> Self {
        TrayItemData {
            node_id: surface.client.state.node_ids.next(),
            tray_item_id: surface.client.state.tray_item_ids.next(),
            seat_state: Default::default(),
            client: surface.client.clone(),
            visible: Cell::new(surface.client.state.root_visible()),
            surface: surface.clone(),
            output: output.clone(),
            attached: Default::default(),
            sent_serial: Default::default(),
            ack_serial: Default::default(),
            linked_node: Default::default(),
            abs_pos: Default::default(),
            rel_pos: Default::default(),
        }
    }

    pub fn find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.surface.find_tree_at_(x, y, tree)
    }
}

pub trait DynTrayItem: Node {
    fn send_current_configure(&self);
    fn data(&self) -> &TrayItemData;
    fn set_position(&self, abs_pos: Rect, rel_pos: Rect);
    fn destroy_popups(&self);
    fn destroy_node(&self);
    fn set_visible(&self, visible: bool);
}

impl<T: TrayItem> DynTrayItem for T {
    fn send_current_configure(&self) {
        <Self as TrayItem>::send_current_configure(self)
    }

    fn data(&self) -> &TrayItemData {
        <Self as TrayItem>::data(self)
    }

    fn set_position(&self, abs_pos: Rect, rel_pos: Rect) {
        let data = self.data();
        data.surface
            .set_absolute_position(abs_pos.x1(), abs_pos.y1());
        data.rel_pos.set(rel_pos);
        if data.abs_pos.replace(abs_pos) != abs_pos {
            for popup in self.popups().lock().values() {
                popup.popup.update_absolute_position();
            }
        }
    }

    fn destroy_popups(&self) {
        for popup in self.popups().lock().drain_values() {
            popup.popup.destroy_node();
        }
    }

    fn destroy_node(&self) {
        let data = self.data();
        data.linked_node.take();
        data.attached.set(false);
        self.destroy_popups();
        data.surface.destroy_node();
        data.seat_state.destroy_node(self);
        data.client.state.tree_changed();
        if let Some(node) = data.output.node() {
            node.update_tray_positions();
        }
    }

    fn set_visible(&self, visible: bool) {
        let data = self.data();
        data.visible.set(visible);
        let visible = visible && data.surface.buffer.is_some();
        data.surface.set_visible(visible);
        if !visible {
            self.destroy_popups();
        }
    }
}

trait TrayItem: Sized + 'static {
    fn send_initial_configure(&self);
    fn send_current_configure(&self);
    fn data(&self) -> &TrayItemData;
    fn popups(&self) -> &CopyHashMap<XdgPopupId, Rc<Popup<Self>>>;
    fn visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum FocusHint {
    None,
    OnDemand,
    Immediate,
}

struct Popup<T> {
    parent: Rc<T>,
    popup: Rc<XdgPopup>,
    seat: Rc<WlSeatGlobal>,
    serial: u64,
    focus: FocusHint,
    stack: Rc<LinkedList<Rc<dyn StackedNode>>>,
    stack_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
}

impl<T: TrayItem> XdgPopupParent for Popup<T> {
    fn position(&self) -> Rect {
        self.parent.data().abs_pos.get()
    }

    fn remove_popup(&self) {
        self.seat.remove_tray_item_popup(&*self.parent, &self.popup);
        self.parent.popups().remove(&self.popup.id);
    }

    fn output(&self) -> Rc<OutputNode> {
        self.parent.data().surface.output.get()
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
                let data = self.parent.data();
                if data.surface.visible.get() {
                    self.popup.set_visible(true);
                    *dl = Some(self.stack.add_last(self.popup.clone()));
                    state.tree_changed();
                    if self.focus == FocusHint::Immediate {
                        self.seat.handle_focus_request(
                            &data.client,
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

    fn visible(&self) -> bool {
        self.parent.node_visible()
    }

    fn make_visible(self: Rc<Self>) {
        // nothing
    }

    fn node_layer(&self) -> NodeLayerLink {
        let Some(link) = self.stack_link.borrow().as_ref().map(|w| w.to_ref()) else {
            return NodeLayerLink::Display;
        };
        NodeLayerLink::Stacked(link)
    }

    fn tray_item(&self) -> Option<TrayItemId> {
        Some(self.parent.data().tray_item_id)
    }

    fn allow_popup_focus(&self) -> bool {
        match self.focus {
            FocusHint::None => false,
            FocusHint::OnDemand => true,
            FocusHint::Immediate => true,
        }
    }
}

impl<T: TrayItem> SurfaceExt for T {
    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Output
    }

    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        if let Some(serial) = pending.tray_item_ack_serial.take() {
            self.data().ack_serial.set(serial);
        }
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>) {
        let data = self.data();
        if data.surface.visible.get() {
            if data.surface.buffer.is_none() {
                self.destroy_node();
            }
        } else {
            if data.ack_serial.get() != data.sent_serial.get() {
                return;
            }
            if data.surface.buffer.is_some() {
                data.surface.set_visible(data.visible.get());
                if let Some(node) = data.output.node()
                    && !data.attached.replace(true)
                {
                    let link = node.tray_items.add_last(self.clone());
                    data.linked_node.set(Some(link));
                    node.update_tray_positions();
                }
            }
        }
    }

    fn extents_changed(&self) {
        let data = self.data();
        if data.surface.visible.get() {
            data.client.state.tree_changed();
        }
    }

    fn tray_item(self: Rc<Self>) -> Option<TrayItemId> {
        Some(self.data().tray_item_id)
    }
}

impl<T: TrayItem> Node for T {
    fn node_id(&self) -> NodeId {
        self.data().node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.data().seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        self.visit(visitor);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        self.data().surface.clone().node_visit(visitor);
    }

    fn node_visible(&self) -> bool {
        self.data().surface.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.data().surface.node_absolute_position()
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.data().output.node()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.data().surface.node_location()
    }

    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Output
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        _usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        self.data().find_tree_at(x, y, tree)
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.data().client.clone())
    }

    fn node_client_id(&self) -> Option<ClientId> {
        Some(self.data().client.id)
    }
}

fn install<T: TrayItem>(item: &Rc<T>) -> Result<(), TrayItemError> {
    let data = item.data();
    data.surface.set_role(SurfaceRole::TrayItem)?;
    if data.surface.ext.get().is_some() {
        return Err(TrayItemError::Exists);
    }
    data.surface.ext.set(item.clone());
    data.surface.set_visible(false);
    if let Some(node) = data.output.node() {
        data.surface
            .set_output(&node, NodeLocation::Output(node.id));
        item.send_initial_configure();
    }
    Ok(())
}

fn destroy<T: TrayItem>(item: &T) -> Result<(), TrayItemError> {
    if item.popups().is_not_empty() {
        return Err(TrayItemError::HasPopups);
    }
    item.destroy_node();
    item.data().surface.unset_ext();
    item.data().surface.set_visible(false);
    Ok(())
}

fn ack_configure<T: TrayItem>(item: &T, serial: u32) {
    item.data()
        .surface
        .pending
        .borrow_mut()
        .tray_item_ack_serial = Some(serial);
}

fn get_popup<T: TrayItem>(
    item: &Rc<T>,
    popup: XdgPopupId,
    seat: WlSeatId,
    serial: u32,
    focus: FocusHint,
) -> Result<(), TrayItemError> {
    let data = item.data();
    let popup = data.client.lookup(popup)?;
    let seat = data.client.lookup(seat)?;
    let seat = &seat.global;
    let Some(serial) = data.client.map_serial(serial) else {
        return Err(TrayItemError::InvalidSerial);
    };
    if popup.parent.is_some() {
        return Err(TrayItemError::PopupHasParent);
    }
    let Some(node) = data.output.node() else {
        popup.destroy_node();
        return Ok(());
    };
    seat.add_tray_item_popup(item, &popup);
    let stack = data.client.state.root.stacked.clone();
    popup.xdg.set_popup_stack(&stack, false);
    popup.xdg.set_output(&node);
    let user = Rc::new(Popup {
        parent: item.clone(),
        popup: popup.clone(),
        seat: seat.clone(),
        serial,
        focus,
        stack,
        stack_link: Default::default(),
    });
    popup.parent.set(Some(user.clone()));
    item.popups().set(popup.id, user);
    Ok(())
}

#[derive(Debug, Error)]
pub enum TrayItemError {
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
}
efrom!(TrayItemError, ClientError);
