use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_seat::{NodeSeatState, WlSeatGlobal},
            wl_surface::{
                PendingState, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError,
                xdg_surface::xdg_popup::{XdgPopup, XdgPopupParent},
            },
            zwlr_layer_shell_v1::{OVERLAY, ZwlrLayerShellV1},
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        renderer::Renderer,
        tree::{
            Direction, FindTreeResult, FindTreeUsecase, FoundNode, Node, NodeId, NodeLayerLink,
            NodeLocation, NodeVisitor, OutputNode, StackedNode,
        },
        utils::{
            bitflags::BitflagsExt,
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode},
            numcell::NumCell,
            option_ext::OptionExt,
        },
        wire::{WlSurfaceId, XdgPopupId, ZwlrLayerSurfaceV1Id, zwlr_layer_surface_v1::*},
    },
    std::{
        cell::{Cell, RefCell, RefMut},
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

const KI_NONE: u32 = 0;
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
    pub output: Rc<OutputGlobalOpt>,
    pub _namespace: String,
    pub tracker: Tracker<Self>,
    output_extents: Cell<Rect>,
    pos: Cell<Rect>,
    mapped: Cell<bool>,
    layer: Cell<u32>,
    requested_serial: NumCell<u32>,
    size: Cell<(i32, i32)>,
    anchor: Cell<u32>,
    exclusive_zone: Cell<ExclusiveZone>,
    margin: Cell<(i32, i32, i32, i32)>,
    keyboard_interactivity: Cell<u32>,
    link: RefCell<Option<LinkedNode<Rc<Self>>>>,
    seat_state: NodeSeatState,
    last_configure: Cell<(i32, i32)>,
    exclusive_edge: Cell<Option<u32>>,
    exclusive_size: Cell<ExclusiveSize>,
    popups: CopyHashMap<XdgPopupId, Rc<Popup>>,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ExclusiveSize {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl ExclusiveSize {
    pub fn is_empty(&self) -> bool {
        *self == ExclusiveSize::default()
    }

    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    pub fn max(&self, other: &Self) -> Self {
        Self {
            top: self.top.max(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
            left: self.left.max(other.left),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExclusiveZone {
    MoveSelf,
    FixedSelf,
    Acquire(i32),
}

struct Popup {
    parent: Rc<ZwlrLayerSurfaceV1>,
    popup: Rc<XdgPopup>,
    stack: Rc<LinkedList<Rc<dyn StackedNode>>>,
    stack_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
}

#[derive(Default)]
pub struct PendingLayerSurfaceData {
    size: Option<(i32, i32)>,
    anchor: Option<u32>,
    exclusive_zone: Option<ExclusiveZone>,
    margin: Option<(i32, i32, i32, i32)>,
    keyboard_interactivity: Option<u32>,
    layer: Option<u32>,
    exclusive_edge: Option<u32>,
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
    }
}

impl ZwlrLayerSurfaceV1 {
    pub fn new(
        id: ZwlrLayerSurfaceV1Id,
        shell: &Rc<ZwlrLayerShellV1>,
        surface: &Rc<WlSurface>,
        output: &Rc<OutputGlobalOpt>,
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
            _namespace: namespace.to_string(),
            tracker: Default::default(),
            output_extents: Default::default(),
            pos: Default::default(),
            mapped: Cell::new(false),
            layer: Cell::new(layer),
            requested_serial: Default::default(),
            size: Cell::new((0, 0)),
            anchor: Cell::new(0),
            exclusive_zone: Cell::new(ExclusiveZone::MoveSelf),
            margin: Cell::new((0, 0, 0, 0)),
            keyboard_interactivity: Cell::new(0),
            link: Default::default(),
            seat_state: Default::default(),
            last_configure: Default::default(),
            exclusive_edge: Default::default(),
            exclusive_size: Default::default(),
            popups: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        self.surface.set_role(SurfaceRole::ZwlrLayerSurface)?;
        if self.surface.ext.get().is_some() {
            return Err(ZwlrLayerSurfaceV1Error::AlreadyAttached(self.surface.id));
        }
        self.surface.ext.set(self.clone());
        if let Some(output) = self.output.node() {
            self.surface
                .set_output(&output, NodeLocation::Output(output.id));
        }
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

    pub fn for_each_popup(&self, mut f: impl FnMut(&Rc<XdgPopup>)) {
        for popup in self.popups.lock().values() {
            f(&popup.popup);
        }
    }
}

impl ZwlrLayerSurfaceV1RequestHandler for ZwlrLayerSurfaceV1 {
    type Error = ZwlrLayerSurfaceV1Error;

    fn set_size(&self, req: SetSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.width > u16::MAX as u32 || req.height > u16::MAX as u32 {
            return Err(ZwlrLayerSurfaceV1Error::ExcessiveSize);
        }
        let mut pending = self.pending();
        pending.size = Some((req.width as _, req.height as _));
        Ok(())
    }

    fn set_anchor(&self, req: SetAnchor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.anchor & !(LEFT | RIGHT | TOP | BOTTOM) != 0 {
            return Err(ZwlrLayerSurfaceV1Error::UnknownAnchor(req.anchor));
        }
        let mut pending = self.pending();
        pending.anchor = Some(req.anchor);
        Ok(())
    }

    fn set_exclusive_zone(
        &self,
        req: SetExclusiveZone,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let mut pending = self.pending();
        let zone = if req.zone < 0 {
            ExclusiveZone::FixedSelf
        } else if req.zone == 0 {
            ExclusiveZone::MoveSelf
        } else if req.zone > u16::MAX as i32 {
            return Err(ZwlrLayerSurfaceV1Error::ExcessiveExclusive);
        } else {
            ExclusiveZone::Acquire(req.zone)
        };
        pending.exclusive_zone = Some(zone);
        Ok(())
    }

    fn set_margin(&self, req: SetMargin, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let mut pending = self.pending();
        for s in [req.top, req.right, req.bottom, req.left] {
            if (s as i64).abs() > u16::MAX as i64 {
                return Err(ZwlrLayerSurfaceV1Error::ExcessiveMargin);
            }
        }
        pending.margin = Some((req.top, req.right, req.bottom, req.left));
        Ok(())
    }

    fn set_keyboard_interactivity(
        &self,
        req: SetKeyboardInteractivity,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if req.keyboard_interactivity > KI_ON_DEMAND {
            return Err(ZwlrLayerSurfaceV1Error::UnknownKi(
                req.keyboard_interactivity,
            ));
        }
        let mut pending = self.pending();
        pending.keyboard_interactivity = Some(req.keyboard_interactivity);
        Ok(())
    }

    fn get_popup(&self, req: GetPopup, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let popup = self.client.lookup(req.popup)?;
        if popup.parent.is_some() {
            return Err(ZwlrLayerSurfaceV1Error::PopupHasParent);
        }
        let stack = self.client.state.root.stacked_above_layers.clone();
        popup.xdg.set_popup_stack(&stack, true);
        let user = Rc::new(Popup {
            parent: slf.clone(),
            popup: popup.clone(),
            stack,
            stack_link: Default::default(),
        });
        popup.parent.set(Some(user.clone()));
        self.popups.set(popup.id, user);
        Ok(())
    }

    fn ack_configure(&self, _req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.popups.is_not_empty() {
            return Err(ZwlrLayerSurfaceV1Error::HasPopups);
        }
        self.destroy_node();
        self.client.remove_obj(self)?;
        self.surface.unset_ext();
        Ok(())
    }

    fn set_layer(&self, req: SetLayer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.layer > OVERLAY {
            return Err(ZwlrLayerSurfaceV1Error::UnknownLayer(req.layer));
        }
        let mut pending = self.pending();
        pending.layer = Some(req.layer);
        Ok(())
    }

    fn set_exclusive_edge(
        &self,
        req: SetExclusiveEdge,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if req.edge & !(LEFT | RIGHT | TOP | BOTTOM) != 0 {
            return Err(ZwlrLayerSurfaceV1Error::UnknownAnchor(req.edge));
        }
        if req.edge.count_ones() > 1 {
            return Err(ZwlrLayerSurfaceV1Error::TooManyExclusiveEdges);
        }
        let mut pending = self.pending();
        if req.edge == 0 {
            pending.exclusive_edge = None;
        } else {
            pending.exclusive_edge = Some(req.edge);
        }
        Ok(())
    }
}

impl ZwlrLayerSurfaceV1 {
    pub fn exclusive_size(&self) -> ExclusiveSize {
        self.exclusive_size.get()
    }

    fn update_exclusive_size(&self) {
        let exclusive_edge = {
            if let Some(ee) = self.exclusive_edge.get() {
                Some(ee)
            } else {
                let anchor = self.anchor.get();
                let edges = anchor.count_ones();
                if edges == 1 {
                    Some(anchor)
                } else if edges == 3 {
                    match (!anchor) & (TOP | BOTTOM | LEFT | RIGHT) {
                        TOP => Some(BOTTOM),
                        BOTTOM => Some(TOP),
                        LEFT => Some(RIGHT),
                        RIGHT => Some(LEFT),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        };
        let mut exclusive_size = ExclusiveSize::default();
        if let (ExclusiveZone::Acquire(s), Some(edge)) = (self.exclusive_zone.get(), exclusive_edge)
        {
            match edge {
                TOP => exclusive_size.top = s,
                RIGHT => exclusive_size.right = s,
                BOTTOM => exclusive_size.bottom = s,
                LEFT => exclusive_size.left = s,
                _ => {}
            }
        }
        if self.exclusive_size.replace(exclusive_size) != exclusive_size
            && let Some(output) = self.output.node.get()
        {
            output.update_exclusive_zones();
        }
    }

    fn pre_commit(&self, pending: &mut PendingState) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let pending = pending.layer_surface.get_or_insert_default_ext();
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
        if let Some(edge) = pending.exclusive_edge.take() {
            self.exclusive_edge.set(Some(edge));
        }
        let anchor = self.anchor.get();
        let (width, height) = self.size.get();
        if width == 0 && !anchor.contains(LEFT | RIGHT) {
            return Err(ZwlrLayerSurfaceV1Error::WidthZero);
        }
        if height == 0 && !anchor.contains(TOP | BOTTOM) {
            return Err(ZwlrLayerSurfaceV1Error::HeightZero);
        }
        if let Some(ee) = self.exclusive_edge.get()
            && !self.anchor.get().contains(ee)
        {
            return Err(ZwlrLayerSurfaceV1Error::ExclusiveEdgeNotAnchored);
        }
        self.configure();
        Ok(())
    }

    fn configure(&self) {
        let Some(node) = self.output.node() else {
            return;
        };
        let (mut width, mut height) = self.size.get();
        let (mt, mr, mb, ml) = self.margin.get();
        let (mut available_width, mut available_height) = match self.exclusive_zone.get() {
            ExclusiveZone::MoveSelf => node.non_exclusive_rect.get().size(),
            _ => node.global.pos.get().size(),
        };
        let anchor = self.anchor.get();
        if anchor.contains(LEFT) {
            available_width -= ml;
        }
        if anchor.contains(RIGHT) {
            available_width -= mr;
        }
        if anchor.contains(TOP) {
            available_height -= mt;
        }
        if anchor.contains(BOTTOM) {
            available_height -= mb;
        }
        if width == 0 {
            width = available_width;
        }
        width = width.min(available_width).max(1);
        if height == 0 {
            height = available_height;
        }
        height = height.min(available_height).max(1);
        let serial = self.requested_serial.fetch_add(1) + 1;
        if self.last_configure.replace((width, height)) != (width, height) {
            self.send_configure(serial, width as _, height as _);
        }
    }

    pub fn output_extents(&self) -> Rect {
        self.output_extents.get()
    }

    fn compute_position(&self) {
        let Some(output) = self.output.node() else {
            return;
        };
        let extents = self.surface.extents.get();
        let (width, height) = extents.size();
        let mut anchor = self.anchor.get();
        if anchor == 0 {
            anchor = LEFT | RIGHT | TOP | BOTTOM;
        }
        let (mt, mr, mb, ml) = self.margin.get();
        let opos = output.global.pos.get();
        let rect = match self.exclusive_zone.get() {
            ExclusiveZone::MoveSelf => output.non_exclusive_rect.get(),
            _ => opos,
        };
        let (owidth, oheight) = rect.size();
        let mut x1 = 0;
        let mut y1 = 0;
        if anchor.contains(LEFT | RIGHT) {
            x1 = (owidth - width) / 2;
        } else if anchor.contains(LEFT) {
            x1 = ml;
        } else if anchor.contains(RIGHT) {
            x1 = owidth - width - mr;
        }
        if anchor.contains(TOP | BOTTOM) {
            y1 = (oheight - height) / 2;
        } else if anchor.contains(TOP) {
            y1 = mt;
        } else if anchor.contains(BOTTOM) {
            y1 = oheight - height - mb;
        }
        let a_rect = Rect::new_sized(x1 + rect.x1(), y1 + rect.y1(), width, height).unwrap();
        let o_rect = a_rect.move_(-opos.x1(), -opos.y1());
        self.output_extents.set(o_rect);
        let a_rect_old = self.pos.replace(a_rect);
        let abs_x = a_rect.x1() - extents.x1();
        let abs_y = a_rect.y1() - extents.y1();
        self.surface.set_absolute_position(abs_x, abs_y);
        if a_rect_old != a_rect {
            for popup in self.popups.lock().values() {
                popup.popup.update_absolute_position();
            }
        }
        self.client.state.tree_changed();
    }

    pub fn output_resized(&self) {
        self.configure();
        self.compute_position();
    }

    pub fn exclusive_zones_changed(&self) {
        if self.exclusive_zone.get() != ExclusiveZone::MoveSelf {
            return;
        }
        self.output_resized();
    }

    pub fn destroy_node(&self) {
        self.link.borrow_mut().take();
        self.mapped.set(false);
        self.surface.destroy_node();
        self.seat_state.destroy_node(self);
        self.client.state.tree_changed();
        self.last_configure.take();
        if self.exclusive_size.take().is_not_empty()
            && let Some(node) = self.output.node()
        {
            node.update_exclusive_zones();
        }
        for popup in self.popups.lock().drain_values() {
            popup.popup.destroy_node();
        }
    }

    pub fn set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
        if !visible {
            for popup in self.popups.lock().drain_values() {
                popup.popup.set_visible(false);
                popup.popup.destroy_node();
            }
        }
    }
}

impl SurfaceExt for ZwlrLayerSurfaceV1 {
    fn node_layer(&self) -> NodeLayerLink {
        let Some(link) = self.link.borrow().as_ref().map(|l| l.to_ref()) else {
            return NodeLayerLink::Display;
        };
        match self.layer.get() {
            0 => NodeLayerLink::Layer0(link),
            1 => NodeLayerLink::Layer1(link),
            2 => NodeLayerLink::Layer2(link),
            _ => NodeLayerLink::Layer3(link),
        }
    }

    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        self.deref().pre_commit(pending)?;
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>) {
        let Some(output) = self.output.node() else {
            return;
        };
        let buffer_is_some = self.surface.buffer.is_some();
        let was_mapped = self.mapped.get();
        if self.mapped.get() {
            if !buffer_is_some {
                self.destroy_node();
            } else {
                if self.surface.extents.get().size() != self.pos.get().size() {
                    self.compute_position();
                }
                self.update_exclusive_size();
            }
        } else if buffer_is_some {
            let layer = &output.layers[self.layer.get() as usize];
            *self.link.borrow_mut() = Some(layer.add_last(self.clone()));
            self.mapped.set(true);
            self.compute_position();
            self.update_exclusive_size();
        }
        if self.mapped.get() != was_mapped {
            output.update_visible();
            if self.mapped.get() {
                let (x, y) = self.surface.buffer_abs_pos.get().position();
                let extents = self.surface.extents.get().move_(x, y);
                self.client.state.damage(extents);
            }
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

    fn node_output(&self) -> Option<Rc<OutputNode>> {
        self.output.node()
    }

    fn node_location(&self) -> Option<NodeLocation> {
        self.surface.node_location()
    }

    fn node_layer(&self) -> NodeLayerLink {
        SurfaceExt::node_layer(self)
    }

    fn node_accepts_focus(&self) -> bool {
        self.keyboard_interactivity.get() != KI_NONE
    }

    fn node_do_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, _direction: Direction) {
        seat.focus_node(self.surface.clone())
    }

    fn node_find_tree_at(
        &self,
        x: i32,
        y: i32,
        tree: &mut Vec<FoundNode>,
        _usecase: FindTreeUsecase,
    ) -> FindTreeResult {
        let (dx, dy) = self.surface.extents.get().position();
        self.surface.find_tree_at_(x + dx, y + dy, tree)
    }

    fn node_render(&self, renderer: &mut Renderer, x: i32, y: i32, _bounds: Option<&Rect>) {
        renderer.render_layer_surface(self, x, y);
    }
}

impl XdgPopupParent for Popup {
    fn position(&self) -> Rect {
        self.parent.pos.get()
    }

    fn remove_popup(&self) {
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
        let output = self.output();
        let surface = &self.popup.xdg.surface;
        let state = &surface.client.state;
        if surface.buffer.is_some() {
            if dl.is_none() {
                if self.parent.surface.visible.get() {
                    self.popup.xdg.set_output(&output);
                    *dl = Some(self.stack.add_last(self.popup.clone()));
                    state.tree_changed();
                    self.popup.set_visible(self.parent.surface.visible.get());
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
        NodeLayerLink::StackedAboveLayers(link)
    }
}

object_base! {
    self = ZwlrLayerSurfaceV1;
    version = self.shell.version;
}

impl Object for ZwlrLayerSurfaceV1 {
    fn break_loops(&self) {
        self.destroy_node();
        self.link.borrow_mut().take();
    }
}

simple_add_obj!(ZwlrLayerSurfaceV1);

#[derive(Debug, Error)]
pub enum ZwlrLayerSurfaceV1Error {
    #[error(
        "Surface {0} cannot be turned into a zwlr_layer_surface because it already has an attached zwlr_layer_surface"
    )]
    AlreadyAttached(WlSurfaceId),
    #[error("Width was set to 0 but anchor did not contain LEFT and RIGHT")]
    WidthZero,
    #[error("Height was set to 0 but anchor did not contain TOP and BOTTOM")]
    HeightZero,
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown layer {0}")]
    UnknownLayer(u32),
    #[error("Surface size must not be larger than 65535x65535")]
    ExcessiveSize,
    #[error("Margin must not be larger than 65535")]
    ExcessiveMargin,
    #[error("Unknown anchor {0}")]
    UnknownAnchor(u32),
    #[error("Unknown keyboard interactivity {0}")]
    UnknownKi(u32),
    #[error("Surface is not anchored at exclusive edge")]
    ExclusiveEdgeNotAnchored,
    #[error("Request must contain exactly one edge")]
    TooManyExclusiveEdges,
    #[error("Exclusive zone not be larger than 65535")]
    ExcessiveExclusive,
    #[error("Popup already has a parent")]
    PopupHasParent,
    #[error("Surface still has popups")]
    HasPopups,
}
efrom!(ZwlrLayerSurfaceV1Error, WlSurfaceError);
efrom!(ZwlrLayerSurfaceV1Error, ClientError);
