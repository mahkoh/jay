use {
    crate::{
        client::{Client, ClientError},
        configurable::{Configurable, ConfigurableData, ConfigurableDataCore, ConfigurableExt},
        fixed::Fixed,
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::{SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError},
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::{Rect, Size},
        transactions::EnabledSurfaceTransactions,
        tree::{
            FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeBase, NodeId, NodeLayerLink,
            NodeLocation, NodeVisitor, OutputNode, TreeSerial,
            TreeTimeline::{self, LiveTL},
            WorkspaceNode,
        },
        wire::{ExtSessionLockSurfaceV1Id, WlSurfaceId, ext_session_lock_surface_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ExtSessionLockSurfaceV1 {
    pub id: ExtSessionLockSurfaceV1Id,
    pub node_id: ExtSessionLockSurfaceV1NodeId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub output: Rc<OutputGlobalOpt>,
    pub seat_state: NodeSeatState,
    pub version: Version,
    pub destroyed: Cell<bool>,
    pub configurable_data: ConfigurableData<Size>,
    pub desired_size: Cell<Size>,
    pub _enabled_transactions: EnabledSurfaceTransactions,
}

impl ExtSessionLockSurfaceV1 {
    pub fn install(self: &Rc<Self>) -> Result<(), ExtSessionLockSurfaceV1Error> {
        self.surface.set_role(SurfaceRole::ExtSessionLockSurface)?;
        if self.surface.ext.get().is_some() {
            return Err(ExtSessionLockSurfaceV1Error::AlreadyAttached(
                self.surface.id,
            ));
        }
        self.surface.ext.set(self.clone());
        Ok(())
    }

    pub fn change_extents(self: &Rc<Self>, rect: Rect) {
        self.surface.set_absolute_position(rect.x1(), rect.y1());
        self.desired_size.set(rect.size2());
        self.schedule_configure();
    }

    fn send_configure(&self, serial: TreeSerial, width: i32, height: i32) {
        self.client.event(Configure {
            self_id: self.id,
            serial: serial.raw() as _,
            width: width as _,
            height: height as _,
        });
    }

    pub fn set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
        self.seat_state.set_visible(self, visible);
    }
}

impl ExtSessionLockSurfaceV1RequestHandler for ExtSessionLockSurfaceV1 {
    type Error = ExtSessionLockSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy_node();
        self.destroyed.set(true);
        self.configurable_data.ready();
        self.surface.unset_ext();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn ack_configure(&self, req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let serial = self.client.state.map_tree_serial32(req.serial);
        self.surface.pending.borrow_mut().serial = Some(serial);
        Ok(())
    }
}

impl ExtSessionLockSurfaceV1 {
    pub fn destroy_node(&self) {
        if let Some(output) = &self.output.node()
            && let Some(ls) = output.node_state[LiveTL].lock_surface.get()
            && ls.node_id == self.node_id
        {
            output.set_lock_surface(None);
            self.client.state.tree_changed();
        }
        self.surface.destroy_node();
        self.seat_state.destroy_node(self);
    }
}

impl SurfaceExt for ExtSessionLockSurfaceV1 {
    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Lock
    }

    fn extents_changed(&self) {
        self.client.state.tree_changed();
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        Some(self.surface.clone())
    }

    fn configurable_data(&self) -> Option<&ConfigurableDataCore> {
        Some(self.configurable_data.core())
    }

    fn workspace(&self) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn unmap(self: Rc<Self>) {
        // nothing
    }
}

tree_id!(ExtSessionLockSurfaceV1NodeId);
impl NodeBase for ExtSessionLockSurfaceV1 {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: &Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_lock_surface(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.surface);
    }

    fn node_visible(&self, _tl: TreeTimeline) -> bool {
        true
    }

    fn node_absolute_position(&self, tl: TreeTimeline) -> Rect {
        self.surface.node_absolute_position(tl)
    }

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.output.node()
    }

    fn node_workspace(&self) -> Option<Rc<WorkspaceNode>> {
        None
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.surface.node_location()
    }

    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Lock
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

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.focus_node_with_serial(self.surface.clone(), self.client.next_serial());
    }
}

object_base! {
    self = ExtSessionLockSurfaceV1;
    version = self.version;
}

impl Object for ExtSessionLockSurfaceV1 {
    fn break_loops(self: Rc<Self>) {
        self.destroy_node();
        self.destroyed.set(true);
        self.configurable_data.ready();
    }
}

simple_add_obj!(ExtSessionLockSurfaceV1);

impl Configurable for ExtSessionLockSurfaceV1 {
    type T = Size;

    fn data(&self) -> &ConfigurableData<Self::T> {
        &self.configurable_data
    }

    fn configure_data(&self) -> Self::T {
        self.desired_size.get()
    }

    fn merge(first: &mut Self::T, second: Self::T) {
        *first = second;
    }

    fn visible(&self) -> bool {
        self.node_visible(LiveTL)
    }

    fn destroyed(&self) -> bool {
        self.destroyed.get()
    }

    fn surface(&self) -> &Rc<WlSurface> {
        &self.surface
    }

    fn flush(&self, serial: TreeSerial, data: Self::T) {
        self.send_configure(serial, data.width(), data.height());
    }
}

#[derive(Debug, Error)]
pub enum ExtSessionLockSurfaceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
    #[error(
        "Surface {0} cannot be turned into an ext_session_lock_surface because it already has an attached ext_session_lock_surface"
    )]
    AlreadyAttached(WlSurfaceId),
}
efrom!(ExtSessionLockSurfaceV1Error, ClientError);
