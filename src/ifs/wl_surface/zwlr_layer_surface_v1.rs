use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_seat::NodeSeatState,
            wl_surface::{PendingState, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError},
            zwlr_layer_shell_v1::{ZwlrLayerShellV1, OVERLAY},
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        renderer::Renderer,
        tree::{FindTreeResult, FoundNode, Node, NodeId, NodeVisitor, OutputNode},
        utils::{
            bitflags::BitflagsExt,
            buffd::{MsgParser, MsgParserError},
            cell_ext::CellExt,
            linkedlist::LinkedNode,
            numcell::NumCell,
            option_ext::OptionExt,
        },
        wire::{zwlr_layer_surface_v1::*, WlSurfaceId, ZwlrLayerSurfaceV1Id},
    },
    std::{
        cell::{Cell, RefMut},
        mem,
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

const KI_NONE: u32 = 0;
#[allow(dead_code)]
const KI_EXCLUSIVE: u32 = 1;
const KI_ON_DEMAND: u32 = 2;

const TOP: u32 = 1;
const BOTTOM: u32 = 2;
const LEFT: u32 = 4;
const RIGHT: u32 = 8;

tree_id!(ZwlrLayerSurfaceV1NodeId);
pub struct ZwlrLayerSurfaceV1 {
    pub id: ZwlrLayerSurfaceV1Id,
    node_id: ZwlrLayerSurfaceV1NodeId,
    pub shell: Rc<ZwlrLayerShellV1>,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub output: Rc<OutputNode>,
    pub namespace: String,
    pub tracker: Tracker<Self>,
    output_pos: Cell<Rect>,
    pos: Cell<Rect>,
    mapped: Cell<bool>,
    layer: Cell<u32>,
    requested_serial: NumCell<u32>,
    acked_serial: Cell<Option<u32>>,
    size: Cell<(i32, i32)>,
    anchor: Cell<u32>,
    exclusive_zone: Cell<i32>,
    margin: Cell<(i32, i32, i32, i32)>,
    keyboard_interactivity: Cell<u32>,
    link: Cell<Option<LinkedNode<Rc<Self>>>>,
    seat_state: NodeSeatState,
}

#[derive(Default)]
pub struct PendingLayerSurfaceData {
    size: Option<(i32, i32)>,
    anchor: Option<u32>,
    exclusive_zone: Option<i32>,
    margin: Option<(i32, i32, i32, i32)>,
    keyboard_interactivity: Option<u32>,
    layer: Option<u32>,
    any: bool,
}

impl PendingLayerSurfaceData {
    pub fn merge(&mut self, next: &mut Self) {
        macro_rules! opt {
            ($name:ident) => {
                if let Some(n) = next.$name.take() {
                    self.$name = Some(n);
                }
            };
        }
        opt!(size);
        opt!(anchor);
        opt!(exclusive_zone);
        opt!(margin);
        opt!(keyboard_interactivity);
        opt!(layer);
        self.any |= mem::take(&mut next.any);
    }
}

impl ZwlrLayerSurfaceV1 {
    pub fn new(
        id: ZwlrLayerSurfaceV1Id,
        shell: &Rc<ZwlrLayerShellV1>,
        surface: &Rc<WlSurface>,
        output: &Rc<OutputNode>,
        layer: u32,
        namespace: &str,
    ) -> Self {
        Self {
            id,
            node_id: shell.client.state.node_ids.next(),
            shell: shell.clone(),
            client: shell.client.clone(),
            surface: surface.clone(),
            output: output.clone(),
            namespace: namespace.to_string(),
            tracker: Default::default(),
            output_pos: Default::default(),
            pos: Default::default(),
            mapped: Cell::new(false),
            layer: Cell::new(layer),
            requested_serial: Default::default(),
            acked_serial: Cell::new(None),
            size: Cell::new((0, 0)),
            anchor: Cell::new(0),
            exclusive_zone: Cell::new(0),
            margin: Cell::new((0, 0, 0, 0)),
            keyboard_interactivity: Cell::new(0),
            link: Cell::new(None),
            seat_state: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        self.surface.set_role(SurfaceRole::ZwlrLayerSurface)?;
        if self.surface.ext.get().is_some() {
            return Err(ZwlrLayerSurfaceV1Error::AlreadyAttached(self.surface.id));
        }
        self.surface.ext.set(self.clone());
        self.surface.set_output(&self.output);
        Ok(())
    }

    fn send_configure(&self, serial: u32, width: u32, height: u32) {
        self.client.event(Configure {
            self_id: self.id,
            serial,
            width,
            height,
        });
    }

    pub fn send_closed(&self) {
        self.client.event(Closed { self_id: self.id });
    }

    fn pending(&self) -> RefMut<Box<PendingLayerSurfaceData>> {
        RefMut::map(self.surface.pending.borrow_mut(), |m| {
            m.layer_surface.get_or_insert_default_ext()
        })
    }

    fn set_size(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: SetSize = self.client.parse(self, parser)?;
        if req.width > u16::MAX as u32 || req.height > u16::MAX as u32 {
            return Err(ZwlrLayerSurfaceV1Error::ExcessiveSize);
        }
        let mut pending = self.pending();
        pending.size = Some((req.width as _, req.height as _));
        pending.any = true;
        Ok(())
    }

    fn set_anchor(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: SetAnchor = self.client.parse(self, parser)?;
        if req.anchor & !(LEFT | RIGHT | TOP | BOTTOM) != 0 {
            return Err(ZwlrLayerSurfaceV1Error::UnknownAnchor(req.anchor));
        }
        let mut pending = self.pending();
        pending.anchor = Some(req.anchor);
        pending.any = true;
        Ok(())
    }

    fn set_exclusive_zone(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: SetExclusiveZone = self.client.parse(self, parser)?;
        let mut pending = self.pending();
        pending.exclusive_zone = Some(req.zone);
        pending.any = true;
        Ok(())
    }

    fn set_margin(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: SetMargin = self.client.parse(self, parser)?;
        let mut pending = self.pending();
        pending.margin = Some((req.top, req.right, req.bottom, req.left));
        pending.any = true;
        Ok(())
    }

    fn set_keyboard_interactivity(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: SetKeyboardInteractivity = self.client.parse(self, parser)?;
        if req.keyboard_interactivity > KI_ON_DEMAND {
            return Err(ZwlrLayerSurfaceV1Error::UnknownKi(
                req.keyboard_interactivity,
            ));
        }
        let mut pending = self.pending();
        pending.keyboard_interactivity = Some(req.keyboard_interactivity);
        pending.any = true;
        Ok(())
    }

    fn get_popup(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let _req: GetPopup = self.client.parse(self, parser)?;
        Ok(())
    }

    fn ack_configure(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: AckConfigure = self.client.parse(self, parser)?;
        self.acked_serial.set(Some(req.serial));
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.destroy_node();
        self.client.remove_obj(self)?;
        self.surface.unset_ext();
        Ok(())
    }

    fn set_layer(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let req: SetLayer = self.client.parse(self, parser)?;
        if req.layer > OVERLAY {
            return Err(ZwlrLayerSurfaceV1Error::UnknownLayer(req.layer));
        }
        let mut pending = self.pending();
        pending.layer = Some(req.layer);
        pending.any = true;
        Ok(())
    }

    fn pre_commit(&self, pending: &mut PendingState) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let pending = pending.layer_surface.get_or_insert_default_ext();
        let mut send_configure = mem::replace(&mut pending.any, false);
        if let Some(size) = pending.size.take() {
            self.size.set(size);
        }
        if let Some(anchor) = pending.anchor.take() {
            self.anchor.set(anchor);
        }
        if let Some(ez) = pending.exclusive_zone.take() {
            self.exclusive_zone.set(ez);
        }
        if let Some(margin) = pending.margin.take() {
            self.margin.set(margin);
        }
        if let Some(ki) = pending.keyboard_interactivity.take() {
            self.keyboard_interactivity.set(ki);
        }
        if let Some(layer) = pending.layer.take() {
            self.layer.set(layer);
        }
        {
            let (mut width, mut height) = self.size.get();
            let anchor = self.anchor.get();
            if width == 0 {
                if !anchor.contains(LEFT | RIGHT) {
                    return Err(ZwlrLayerSurfaceV1Error::WidthZero);
                }
                send_configure = true;
                width = self.output.global.position().width();
            }
            if height == 0 {
                if !anchor.contains(TOP | BOTTOM) {
                    return Err(ZwlrLayerSurfaceV1Error::HeightZero);
                }
                send_configure = true;
                height = self.output.global.position().height();
            }
            self.size.set((width, height));
        }
        if self.acked_serial.is_none() {
            send_configure = true;
        }
        if send_configure {
            let (width, height) = self.size.get();
            let serial = self.requested_serial.fetch_add(1) + 1;
            self.send_configure(serial, width as _, height as _);
        }
        Ok(())
    }

    pub fn output_position(&self) -> Rect {
        self.output_pos.get()
    }

    pub fn position(&self) -> Rect {
        self.pos.get()
    }

    pub fn compute_position(&self) {
        let (width, height) = self.size.get();
        let mut anchor = self.anchor.get();
        if anchor == 0 {
            anchor = LEFT | RIGHT | TOP | BOTTOM;
        }
        let opos = self.output.global.pos.get();
        let mut x1 = 0;
        let mut y1 = 0;
        if anchor.contains(LEFT) {
            if anchor.contains(RIGHT) {
                x1 += (opos.width() - width) / 2;
            }
        } else if anchor.contains(RIGHT) {
            x1 += opos.width() - width;
        }
        if anchor.contains(TOP) {
            if anchor.contains(BOTTOM) {
                y1 += (opos.height() - height) / 2;
            }
        } else if anchor.contains(BOTTOM) {
            y1 += opos.height() - height;
        }
        let rect = Rect::new_sized(x1, y1, width, height).unwrap();
        self.output_pos.set(rect);
        self.pos.set(rect.move_(opos.x1(), opos.y1()));
        self.surface.set_absolute_position(x1, y1);
        self.client.state.tree_changed();
    }

    pub fn destroy_node(&self) {
        self.link.set(None);
        self.mapped.set(false);
        self.surface.destroy_node();
        self.seat_state.destroy_node(self);
        self.client.state.tree_changed();
    }
}

impl SurfaceExt for ZwlrLayerSurfaceV1 {
    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        self.deref().pre_commit(pending)?;
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>, _pending: &mut PendingState) {
        let buffer = self.surface.buffer.get();
        if self.mapped.get() {
            if buffer.is_none() {
                self.destroy_node();
            } else {
                let pos = self.pos.get();
                let (width, height) = self.size.get();
                if width != pos.width() || height != pos.height() {
                    self.compute_position();
                }
            }
        } else if buffer.is_some() {
            let layer = &self.output.layers[self.layer.get() as usize];
            self.link.set(Some(layer.add_last(self.clone())));
            self.mapped.set(true);
            self.compute_position();
        }
        if self.mapped.get() {
            match self.keyboard_interactivity.get() {
                KI_NONE => {
                    let was_active = self.surface.seat_state.is_active();
                    self.surface.seat_state.release_kb_focus();
                    if was_active {
                        self.surface.node_active_changed(false);
                    }
                }
                KI_ON_DEMAND => self.surface.seat_state.release_kb_grab(),
                KI_EXCLUSIVE => {
                    let seats = self.client.state.globals.seats.lock();
                    for seat in seats.values() {
                        seat.grab(self.surface.clone());
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        if self.keyboard_interactivity.get() != KI_NONE {
            Some(self.surface.clone())
        } else {
            None
        }
    }
}

impl Node for ZwlrLayerSurfaceV1 {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_layer_surface(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self.surface);
    }

    fn node_visible(&self) -> bool {
        true
    }

    fn node_absolute_position(&self) -> Rect {
        self.pos.get()
    }

    fn node_find_tree_at(&self, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        self.surface.find_tree_at_(x, y, tree)
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_layer_surface(self, x, y);
    }
}

object_base! {
    self = ZwlrLayerSurfaceV1;

    SET_SIZE                   => set_size,
    SET_ANCHOR                 => set_anchor,
    SET_EXCLUSIVE_ZONE         => set_exclusive_zone,
    SET_MARGIN                 => set_margin,
    SET_KEYBOARD_INTERACTIVITY => set_keyboard_interactivity,
    GET_POPUP                  => get_popup,
    ACK_CONFIGURE              => ack_configure,
    DESTROY                    => destroy,
    SET_LAYER                  => set_layer if self.shell.version >= 2,
}

impl Object for ZwlrLayerSurfaceV1 {
    fn break_loops(&self) {
        self.destroy_node();
        self.link.set(None);
    }
}

simple_add_obj!(ZwlrLayerSurfaceV1);

#[derive(Debug, Error)]
pub enum ZwlrLayerSurfaceV1Error {
    #[error("Surface {0} cannot be turned into a zwlr_layer_surface because it already has an attached zwlr_layer_surface")]
    AlreadyAttached(WlSurfaceId),
    #[error("Width was set to 0 but anchor did not contain LEFT and RIGHT")]
    WidthZero,
    #[error("Height was set to 0 but anchor did not contain TOP and BOTTOM")]
    HeightZero,
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown layer {0}")]
    UnknownLayer(u32),
    #[error("Surface size must not be larger than 65535x65535")]
    ExcessiveSize,
    #[error("Unknown anchor {0}")]
    UnknownAnchor(u32),
    #[error("Unknown keyboard interactivity {0}")]
    UnknownKi(u32),
}
efrom!(ZwlrLayerSurfaceV1Error, WlSurfaceError);
efrom!(ZwlrLayerSurfaceV1Error, MsgParserError);
efrom!(ZwlrLayerSurfaceV1Error, ClientError);
