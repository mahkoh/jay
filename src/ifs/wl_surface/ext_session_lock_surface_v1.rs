use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::{
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::{SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError},
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        tree::{FindTreeResult, FoundNode, Node, NodeId, NodeVisitor, OutputNode},
        utils::{
            buffd::{MsgParser, MsgParserError},
            numcell::NumCell,
        },
        wire::{ext_session_lock_surface_v1::*, ExtSessionLockSurfaceV1Id, WlSurfaceId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtSessionLockSurfaceV1 {
    pub id: ExtSessionLockSurfaceV1Id,
    pub node_id: ExtSessionLockSurfaceV1NodeId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub serial: NumCell<u32>,
    pub output: Option<Rc<OutputNode>>,
    pub seat_state: NodeSeatState,
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

    pub fn change_extents(&self, rect: Rect) {
        self.send_configure(rect.width(), rect.height());
        self.surface.set_absolute_position(rect.x1(), rect.x2());
    }

    fn send_configure(&self, width: i32, height: i32) {
        self.client.event(Configure {
            self_id: self.id,
            serial: self.serial.fetch_add(1),
            width: width as _,
            height: height as _,
        });
    }

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockSurfaceV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.destroy_node();
        self.surface.unset_ext();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn ack_configure(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockSurfaceV1Error> {
        let _req: AckConfigure = self.client.parse(self, msg)?;
        Ok(())
    }

    pub fn destroy_node(&self) {
        if let Some(output) = &self.output {
            if let Some(ls) = output.lock_surface.get() {
                if ls.node_id == self.node_id {
                    output.lock_surface.take();
                    self.client.state.tree_changed();
                }
            }
        }
        self.surface.destroy_node();
        self.seat_state.destroy_node(self);
    }
}

impl SurfaceExt for ExtSessionLockSurfaceV1 {
    fn extents_changed(&self) {
        self.client.state.tree_changed();
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        Some(self.surface.clone())
    }
}

tree_id!(ExtSessionLockSurfaceV1NodeId);
impl Node for ExtSessionLockSurfaceV1 {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_lock_surface(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.surface);
    }

    fn node_visible(&self) -> bool {
        true
    }

    fn node_absolute_position(&self) -> Rect {
        self.surface.node_absolute_position()
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.surface.find_tree_at_(x, y, tree)
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _x: Fixed, _y: Fixed) {
        seat.focus_node(self.surface.clone());
    }
}

object_base! {
    self = ExtSessionLockSurfaceV1;

    DESTROY => destroy,
    ACK_CONFIGURE => ack_configure,
}

impl Object for ExtSessionLockSurfaceV1 {
    fn break_loops(&self) {
        self.destroy_node();
    }
}

simple_add_obj!(ExtSessionLockSurfaceV1);

#[derive(Debug, Error)]
pub enum ExtSessionLockSurfaceV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
    #[error("Surface {0} cannot be turned into an ext_session_lock_surface because it already has an attached ext_session_lock_surface")]
    AlreadyAttached(WlSurfaceId),
}
efrom!(ExtSessionLockSurfaceV1Error, MsgParserError);
efrom!(ExtSessionLockSurfaceV1Error, ClientError);
