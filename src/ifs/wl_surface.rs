pub mod cursor;
pub mod ext_session_lock_surface_v1;
pub mod wl_subsurface;
pub mod wp_fractional_scale_v1;
pub mod wp_tearing_control_v1;
pub mod wp_viewport;
pub mod x_surface;
pub mod xdg_surface;
pub mod xwayland_shell_v1;
pub mod zwlr_layer_surface_v1;
pub mod zwp_idle_inhibitor_v1;

use {
    crate::{
        backend::KeyState,
        client::{Client, ClientError, RequestParser},
        fixed::Fixed,
        gfx_api::{BufferPoint, BufferPoints},
        ifs::{
            wl_buffer::WlBuffer,
            wl_callback::WlCallback,
            wl_output::{
                TF_180, TF_270, TF_90, TF_FLIPPED, TF_FLIPPED_180, TF_FLIPPED_270, TF_FLIPPED_90,
                TF_NORMAL,
            },
            wl_seat::{
                wl_pointer::PendingScroll, zwp_pointer_constraints_v1::SeatConstraint, Dnd,
                NodeSeatState, SeatId, WlSeatGlobal,
            },
            wl_surface::{
                cursor::CursorSurface, wl_subsurface::WlSubsurface,
                wp_fractional_scale_v1::WpFractionalScaleV1,
                wp_tearing_control_v1::WpTearingControlV1, wp_viewport::WpViewport,
                x_surface::XSurface, xdg_surface::XdgSurfaceError,
                zwlr_layer_surface_v1::ZwlrLayerSurfaceV1Error,
            },
            wp_content_type_v1::ContentType,
            wp_presentation_feedback::WpPresentationFeedback,
        },
        leaks::Tracker,
        object::Object,
        rect::{Rect, Region},
        renderer::Renderer,
        tree::{
            FindTreeResult, FoundNode, Node, NodeId, NodeVisitor, NodeVisitorBase, OutputNode,
            ToplevelNode,
        },
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::LinkedList,
            numcell::NumCell,
            smallmap::SmallMap,
        },
        wire::{wl_surface::*, WlOutputId, WlSurfaceId, ZwpIdleInhibitorV1Id},
        xkbcommon::ModifierState,
        xwayland::XWaylandEvent,
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    thiserror::Error,
    zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
};

#[allow(dead_code)]
const INVALID_SCALE: u32 = 0;
#[allow(dead_code)]
const INVALID_TRANSFORM: u32 = 1;
#[allow(dead_code)]
const INVALID_SIZE: u32 = 2;

const OFFSET_SINCE: u32 = 5;
const BUFFER_SCALE_SINCE: u32 = 6;

#[derive(Copy, Clone, Debug, PartialEq)]
enum Transform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl Transform {
    fn swaps_dimensions(self) -> bool {
        match self {
            Transform::Normal => false,
            Transform::Rotate90 => true,
            Transform::Rotate180 => false,
            Transform::Rotate270 => true,
            Transform::Flipped => false,
            Transform::Flipped90 => true,
            Transform::Flipped180 => false,
            Transform::Flipped270 => true,
        }
    }
}

impl Transform {
    fn apply_inv_sized(self, x1: f32, y1: f32, width: f32, height: f32) -> BufferPoints {
        let x2 = x1 + width;
        let y2 = y1 + height;
        self.apply_inv(x1, y1, x2, y2)
    }

    fn apply_inv(self, x1: f32, y1: f32, x2: f32, y2: f32) -> BufferPoints {
        macro_rules! bp {
            (
                $tl_x:expr, $tl_y:expr,
                $tr_x:expr, $tr_y:expr,
                $br_x:expr, $br_y:expr,
                $bl_x:expr, $bl_y:expr,
            ) => {
                BufferPoints {
                    top_left: BufferPoint { x: $tl_x, y: $tl_y },
                    top_right: BufferPoint { x: $tr_x, y: $tr_y },
                    bottom_right: BufferPoint { x: $br_x, y: $br_y },
                    bottom_left: BufferPoint { x: $bl_x, y: $bl_y },
                }
            };
        }
        use Transform::*;
        match self {
            Normal => bp! {
                x1, y1,
                x2, y1,
                x2, y2,
                x1, y2,
            },
            Rotate90 => bp! {
                y1, x2,
                y1, x1,
                y2, x1,
                y2, x2,
            },
            Rotate180 => bp! {
                x2, y2,
                x1, y2,
                x1, y1,
                x2, y1,
            },
            Rotate270 => bp! {
                y2, x1,
                y2, x2,
                y1, x2,
                y1, x1,
            },
            Flipped => bp! {
                x2, y1,
                x1, y1,
                x1, y2,
                x2, y2,
            },
            Flipped90 => bp! {
                y1, x1,
                y1, x2,
                y2, x2,
                y2, x1,
            },
            Flipped180 => bp! {
                x1, y2,
                x2, y2,
                x2, y1,
                x1, y1,
            },
            Flipped270 => bp! {
                y2, x2,
                y2, x1,
                y1, x1,
                y1, x2,
            },
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceRole {
    None,
    Subsurface,
    XdgSurface,
    Cursor,
    DndIcon,
    ZwlrLayerSurface,
    XSurface,
    ExtSessionLockSurface,
}

impl SurfaceRole {
    fn name(self) -> &'static str {
        match self {
            SurfaceRole::None => "none",
            SurfaceRole::Subsurface => "subsurface",
            SurfaceRole::XdgSurface => "xdg_surface",
            SurfaceRole::Cursor => "cursor",
            SurfaceRole::DndIcon => "dnd_icon",
            SurfaceRole::ZwlrLayerSurface => "zwlr_layer_surface",
            SurfaceRole::XSurface => "xwayland surface",
            SurfaceRole::ExtSessionLockSurface => "ext_session_lock_surface",
        }
    }
}

pub struct SurfaceSendPreferredScaleVisitor;
impl NodeVisitorBase for SurfaceSendPreferredScaleVisitor {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        node.on_scale_change();
        node.node_visit_children(self);
    }
}

pub struct WlSurface {
    pub id: WlSurfaceId,
    pub node_id: SurfaceNodeId,
    pub client: Rc<Client>,
    visible: Cell<bool>,
    role: Cell<SurfaceRole>,
    pending: PendingState,
    input_region: Cell<Option<Rc<Region>>>,
    opaque_region: Cell<Option<Rc<Region>>>,
    buffer_points: RefCell<BufferPoints>,
    pub buffer_points_norm: RefCell<BufferPoints>,
    buffer_transform: Cell<Transform>,
    buffer_scale: Cell<i32>,
    src_rect: Cell<Option<[Fixed; 4]>>,
    dst_size: Cell<Option<(i32, i32)>>,
    pub extents: Cell<Rect>,
    pub buffer_abs_pos: Cell<Rect>,
    pub need_extents_update: Cell<bool>,
    pub buffer: CloneCell<Option<Rc<WlBuffer>>>,
    pub buf_x: NumCell<i32>,
    pub buf_y: NumCell<i32>,
    pub children: RefCell<Option<Box<ParentData>>>,
    ext: CloneCell<Rc<dyn SurfaceExt>>,
    pub frame_requests: RefCell<Vec<Rc<WlCallback>>>,
    pub presentation_feedback: RefCell<Vec<Rc<WpPresentationFeedback>>>,
    seat_state: NodeSeatState,
    toplevel: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    cursors: SmallMap<SeatId, Rc<CursorSurface>, 1>,
    pub dnd_icons: SmallMap<SeatId, Rc<WlSeatGlobal>, 1>,
    pub tracker: Tracker<Self>,
    idle_inhibitors: CopyHashMap<ZwpIdleInhibitorV1Id, Rc<ZwpIdleInhibitorV1>>,
    viewporter: CloneCell<Option<Rc<WpViewport>>>,
    output: CloneCell<Rc<OutputNode>>,
    fractional_scale: CloneCell<Option<Rc<WpFractionalScaleV1>>>,
    pub constraints: SmallMap<SeatId, Rc<SeatConstraint>, 1>,
    xwayland_serial: Cell<Option<u64>>,
    tearing_control: CloneCell<Option<Rc<WpTearingControlV1>>>,
    tearing: Cell<bool>,
    version: u32,
    pub has_content_type_manager: Cell<bool>,
    content_type: Cell<Option<ContentType>>,
}

impl Debug for WlSurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WlSurface").finish_non_exhaustive()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CommitContext {
    RootCommit,
    ChildCommit,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CommitAction {
    ContinueCommit,
    AbortCommit,
}

trait SurfaceExt {
    fn pre_commit(self: Rc<Self>, ctx: CommitContext) -> Result<CommitAction, WlSurfaceError> {
        let _ = ctx;
        Ok(CommitAction::ContinueCommit)
    }

    fn post_commit(self: Rc<Self>) {
        // nothing
    }

    fn is_some(&self) -> bool {
        true
    }

    fn is_none(&self) -> bool {
        !self.is_some()
    }

    fn on_surface_destroy(&self) -> Result<(), WlSurfaceError> {
        if self.is_some() {
            Err(WlSurfaceError::ReloObjectStillExists)
        } else {
            Ok(())
        }
    }

    fn update_subsurface_parent_extents(&self) {
        // nothing
    }

    fn subsurface_parent(&self) -> Option<Rc<WlSurface>> {
        None
    }

    fn extents_changed(&self) {
        // nothing
    }

    fn into_subsurface(self: Rc<Self>) -> Option<Rc<WlSubsurface>> {
        None
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        None
    }

    fn into_xsurface(self: Rc<Self>) -> Option<Rc<XSurface>> {
        None
    }
}

pub struct NoneSurfaceExt;

impl SurfaceExt for NoneSurfaceExt {
    fn is_some(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct PendingState {
    buffer: Cell<Option<Option<Rc<WlBuffer>>>>,
    offset: Cell<(i32, i32)>,
    opaque_region: Cell<Option<Option<Rc<Region>>>>,
    input_region: Cell<Option<Option<Rc<Region>>>>,
    frame_request: RefCell<Vec<Rc<WlCallback>>>,
    damage: Cell<bool>,
    presentation_feedback: RefCell<Vec<Rc<WpPresentationFeedback>>>,
    src_rect: Cell<Option<Option<[Fixed; 4]>>>,
    dst_size: Cell<Option<Option<(i32, i32)>>>,
    scale: Cell<Option<i32>>,
    transform: Cell<Option<Transform>>,
    xwayland_serial: Cell<Option<u64>>,
    tearing: Cell<Option<bool>>,
    content_type: Cell<Option<Option<ContentType>>>,
}

#[derive(Default)]
pub struct ParentData {
    subsurfaces: AHashMap<WlSurfaceId, Rc<WlSubsurface>>,
    pub below: LinkedList<StackElement>,
    pub above: LinkedList<StackElement>,
}

pub struct StackElement {
    pub pending: Cell<bool>,
    pub sub_surface: Rc<WlSubsurface>,
}

impl WlSurface {
    pub fn new(id: WlSurfaceId, client: &Rc<Client>, version: u32) -> Self {
        Self {
            id,
            node_id: client.state.node_ids.next(),
            client: client.clone(),
            visible: Default::default(),
            role: Cell::new(SurfaceRole::None),
            pending: Default::default(),
            input_region: Default::default(),
            opaque_region: Default::default(),
            buffer_points: Default::default(),
            buffer_points_norm: Default::default(),
            buffer_transform: Cell::new(Transform::Normal),
            buffer_scale: Cell::new(1),
            src_rect: Cell::new(None),
            dst_size: Cell::new(None),
            extents: Default::default(),
            buffer_abs_pos: Cell::new(Default::default()),
            need_extents_update: Default::default(),
            buffer: Default::default(),
            buf_x: Default::default(),
            buf_y: Default::default(),
            children: Default::default(),
            ext: CloneCell::new(client.state.none_surface_ext.clone()),
            frame_requests: Default::default(),
            presentation_feedback: Default::default(),
            seat_state: Default::default(),
            toplevel: Default::default(),
            cursors: Default::default(),
            dnd_icons: Default::default(),
            tracker: Default::default(),
            idle_inhibitors: Default::default(),
            viewporter: Default::default(),
            output: CloneCell::new(client.state.dummy_output.get().unwrap()),
            fractional_scale: Default::default(),
            constraints: Default::default(),
            xwayland_serial: Default::default(),
            tearing_control: Default::default(),
            tearing: Cell::new(false),
            version,
            has_content_type_manager: Default::default(),
            content_type: Default::default(),
        }
    }

    fn get_xsurface(self: &Rc<Self>) -> Result<Rc<XSurface>, WlSurfaceError> {
        self.set_role(SurfaceRole::XSurface)?;
        let mut ext = self.ext.get();
        if ext.is_none() {
            let xsurface = Rc::new(XSurface {
                surface: self.clone(),
                xwindow: Default::default(),
                xwayland_surface: Default::default(),
                tracker: Default::default(),
            });
            track!(self.client, xsurface);
            self.ext.set(xsurface.clone());
            ext = xsurface;
        }
        Ok(ext.into_xsurface().unwrap())
    }

    pub fn set_output(&self, output: &Rc<OutputNode>) {
        let old = self.output.set(output.clone());
        if old.id == output.id {
            return;
        }
        output.global.send_enter(self);
        old.global.send_leave(self);
        if old.preferred_scale.get() != output.preferred_scale.get() {
            self.on_scale_change();
        }
        let children = self.children.borrow_mut();
        if let Some(children) = &*children {
            for ss in children.subsurfaces.values() {
                ss.surface.set_output(output);
            }
        }
    }

    fn on_scale_change(&self) {
        if let Some(fs) = self.fractional_scale.get() {
            fs.send_preferred_scale();
        }
        self.send_preferred_buffer_scale();
    }

    pub fn get_toplevel(&self) -> Option<Rc<dyn ToplevelNode>> {
        self.toplevel.get()
    }

    pub fn xwayland_serial(&self) -> Option<u64> {
        self.xwayland_serial.get()
    }

    fn set_absolute_position(&self, x1: i32, y1: i32) {
        self.buffer_abs_pos
            .set(self.buffer_abs_pos.get().at_point(x1, y1));
        if let Some(children) = self.children.borrow_mut().deref_mut() {
            for ss in children.subsurfaces.values() {
                let pos = ss.position.get();
                ss.surface
                    .set_absolute_position(x1 + pos.x1(), y1 + pos.y1());
            }
        }
    }

    pub fn add_presentation_feedback(&self, fb: &Rc<WpPresentationFeedback>) {
        self.pending
            .presentation_feedback
            .borrow_mut()
            .push(fb.clone());
    }

    pub fn is_cursor(&self) -> bool {
        self.role.get() == SurfaceRole::Cursor
    }

    pub fn get_cursor(
        self: &Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
    ) -> Result<Rc<CursorSurface>, WlSurfaceError> {
        if let Some(cursor) = self.cursors.get(&seat.id()) {
            return Ok(cursor);
        }
        self.set_role(SurfaceRole::Cursor)?;
        let cursor = Rc::new(CursorSurface::new(seat, self));
        track!(self.client, cursor);
        cursor.handle_buffer_change();
        self.cursors.insert(seat.id(), cursor.clone());
        Ok(cursor)
    }

    pub fn get_focus_node(&self, seat: SeatId) -> Option<Rc<dyn Node>> {
        match self.toplevel.get() {
            Some(tl) if tl.tl_accepts_keyboard_focus() => tl.tl_focus_child(seat),
            Some(_) => None,
            _ => self.ext.get().focus_node(),
        }
    }

    pub fn send_enter(&self, output: WlOutputId) {
        self.client.event(Enter {
            self_id: self.id,
            output,
        })
    }

    pub fn send_leave(&self, output: WlOutputId) {
        self.client.event(Leave {
            self_id: self.id,
            output,
        })
    }

    pub fn send_preferred_buffer_scale(&self) {
        if self.version >= BUFFER_SCALE_SINCE {
            self.client.event(PreferredBufferScale {
                self_id: self.id,
                factor: self.output.get().global.legacy_scale.get() as _,
            });
        }
    }

    fn set_toplevel(&self, tl: Option<Rc<dyn ToplevelNode>>) {
        let ch = self.children.borrow();
        if let Some(ch) = &*ch {
            for ss in ch.subsurfaces.values() {
                ss.surface.set_toplevel(tl.clone());
            }
        }
        if self.seat_state.is_active() {
            if let Some(tl) = &tl {
                tl.tl_surface_active_changed(true);
            }
        }
        self.toplevel.set(tl);
    }

    pub fn set_role(&self, role: SurfaceRole) -> Result<(), WlSurfaceError> {
        use SurfaceRole::*;
        match (self.role.get(), role) {
            (None, _) => {}
            (old, new) if old == new => {}
            (old, new) => {
                return Err(WlSurfaceError::IncompatibleRole {
                    id: self.id,
                    old,
                    new,
                })
            }
        }
        self.role.set(role);
        Ok(())
    }

    fn unset_ext(&self) {
        self.ext.set(self.client.state.none_surface_ext.clone());
    }

    fn calculate_extents(&self) {
        let old_extents = self.extents.get();
        let mut extents = self.buffer_abs_pos.get().at_point(0, 0);
        let children = self.children.borrow();
        if let Some(children) = &*children {
            for ss in children.subsurfaces.values() {
                let ce = ss.surface.extents.get();
                if !ce.is_empty() {
                    let cp = ss.position.get();
                    let ce = ce.move_(cp.x1(), cp.y1());
                    extents = if extents.is_empty() {
                        ce
                    } else {
                        extents.union(ce)
                    };
                }
            }
        }
        self.extents.set(extents);
        self.need_extents_update.set(false);
        if old_extents != extents {
            self.ext.get().extents_changed()
        }
    }

    pub fn get_root(self: &Rc<Self>) -> Rc<WlSurface> {
        let mut root = self.clone();
        loop {
            if let Some(parent) = root.ext.get().subsurface_parent() {
                root = parent;
                continue;
            }
            break;
        }
        root
    }

    fn parse<'a, T: RequestParser<'a>>(
        &self,
        parser: MsgParser<'_, 'a>,
    ) -> Result<T, MsgParserError> {
        self.client.parse(self, parser)
    }

    fn unset_cursors(&self) {
        while let Some((_, cursor)) = self.cursors.pop() {
            cursor.handle_surface_destroy();
        }
    }

    fn unset_dnd_icons(&self) {
        while let Some((_, seat)) = self.dnd_icons.pop() {
            seat.remove_dnd_icon()
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let _req: Destroy = self.parse(parser)?;
        self.unset_dnd_icons();
        self.unset_cursors();
        self.ext.get().on_surface_destroy()?;
        self.destroy_node();
        {
            let mut children = self.children.borrow_mut();
            if let Some(children) = &mut *children {
                for ss in children.subsurfaces.values() {
                    ss.surface.unset_ext();
                }
            }
            *children = None;
        }
        if let Some(buffer) = self.buffer.set(None) {
            if !buffer.destroyed() {
                buffer.send_release();
            }
        }
        if let Some(xwayland_serial) = self.xwayland_serial.get() {
            self.client
                .surfaces_by_xwayland_serial
                .remove(&xwayland_serial);
        }
        self.frame_requests.borrow_mut().clear();
        self.toplevel.set(None);
        self.client.remove_obj(self)?;
        self.idle_inhibitors.clear();
        self.constraints.take();
        Ok(())
    }

    fn attach(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let req: Attach = self.parse(parser)?;
        if self.version >= OFFSET_SINCE {
            if req.x != 0 || req.y != 0 {
                return Err(WlSurfaceError::OffsetInAttach);
            }
        } else {
            self.pending.offset.set((req.x, req.y));
        }
        let buf = if req.buffer.is_some() {
            Some(self.client.lookup(req.buffer)?)
        } else {
            None
        };
        self.pending.buffer.set(Some(buf));
        Ok(())
    }

    fn damage(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let _req: Damage = self.parse(parser)?;
        self.pending.damage.set(true);
        Ok(())
    }

    fn frame(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let req: Frame = self.parse(parser)?;
        let cb = Rc::new(WlCallback::new(req.callback, &self.client));
        track!(self.client, cb);
        self.client.add_client_obj(&cb)?;
        self.pending.frame_request.borrow_mut().push(cb);
        Ok(())
    }

    fn set_opaque_region(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let region: SetOpaqueRegion = self.parse(parser)?;
        let region = if region.region.is_some() {
            Some(self.client.lookup(region.region)?.region())
        } else {
            None
        };
        self.pending.opaque_region.set(Some(region));
        Ok(())
    }

    fn set_input_region(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let req: SetInputRegion = self.parse(parser)?;
        let region = if req.region.is_some() {
            Some(self.client.lookup(req.region)?.region())
        } else {
            None
        };
        self.pending.input_region.set(Some(region));
        Ok(())
    }

    fn do_commit(self: &Rc<Self>, ctx: CommitContext) -> Result<(), WlSurfaceError> {
        let ext = self.ext.get();
        if ext.clone().pre_commit(ctx)? == CommitAction::AbortCommit {
            return Ok(());
        }
        {
            let children = self.children.borrow();
            if let Some(children) = children.deref() {
                for child in children.subsurfaces.values() {
                    child.surface.do_commit(CommitContext::ChildCommit)?;
                }
            }
        }
        let mut scale_changed = false;
        if let Some(scale) = self.pending.scale.take() {
            scale_changed = true;
            self.buffer_scale.set(scale);
        }
        let mut buffer_transform_changed = false;
        if let Some(transform) = self.pending.transform.take() {
            buffer_transform_changed = true;
            self.buffer_transform.set(transform);
        }
        let mut viewport_changed = false;
        if let Some(dst_size) = self.pending.dst_size.take() {
            viewport_changed = true;
            self.dst_size.set(dst_size);
        }
        if let Some(src_rect) = self.pending.src_rect.take() {
            viewport_changed = true;
            self.src_rect.set(src_rect);
        }
        if viewport_changed {
            if let Some(rect) = self.src_rect.get() {
                if self.dst_size.get().is_none() {
                    if !rect[2].is_integer() || !rect[3].is_integer() {
                        return Err(WlSurfaceError::NonIntegerViewportSize);
                    }
                }
            }
        }
        let mut buffer_changed = false;
        let mut old_raw_size = None;
        let (dx, dy) = self.pending.offset.take();
        if let Some(buffer_change) = self.pending.buffer.take() {
            buffer_changed = true;
            if let Some(buffer) = self.buffer.take() {
                old_raw_size = Some(buffer.rect);
                if !buffer.destroyed() {
                    buffer.send_release();
                }
            }
            if let Some(buffer) = buffer_change {
                buffer.update_texture_or_log();
                self.buffer.set(Some(buffer));
                self.buf_x.fetch_add(dx);
                self.buf_y.fetch_add(dy);
                if (dx, dy) != (0, 0) {
                    self.need_extents_update.set(true);
                    for (_, cursor) in &self.cursors {
                        cursor.dec_hotspot(dx, dy);
                    }
                }
            } else {
                self.buf_x.set(0);
                self.buf_y.set(0);
                for (_, cursor) in &self.cursors {
                    cursor.set_hotspot(0, 0);
                }
            }
        }
        let transform_changed = viewport_changed || scale_changed || buffer_transform_changed;
        if buffer_changed || transform_changed {
            let mut buffer_points = self.buffer_points.borrow_mut();
            let mut buffer_points_norm = self.buffer_points_norm.borrow_mut();
            let mut new_size = None;
            if let Some(src_rect) = self.src_rect.get() {
                if transform_changed {
                    let [mut x1, mut y1, mut width, mut height] = src_rect.map(|v| v.to_f64() as _);
                    let scale = self.buffer_scale.get();
                    if scale != 1 {
                        let scale = scale as f32;
                        x1 *= scale;
                        y1 *= scale;
                        width *= scale;
                        height *= scale;
                    }
                    *buffer_points = self
                        .buffer_transform
                        .get()
                        .apply_inv_sized(x1, y1, width, height);
                }
                let size = match self.dst_size.get() {
                    Some(ds) => ds,
                    None => (src_rect[2].to_int(), src_rect[3].to_int()),
                };
                new_size = Some(size);
            } else if let Some(size) = self.dst_size.get() {
                new_size = Some(size);
            }
            if let Some(buffer) = self.buffer.get() {
                if new_size.is_none() {
                    let (mut width, mut height) = buffer.rect.size();
                    if self.buffer_transform.get().swaps_dimensions() {
                        mem::swap(&mut width, &mut height);
                    }
                    let scale = self.buffer_scale.get();
                    if scale != 1 {
                        width = (width + scale - 1) / scale;
                        height = (height + scale - 1) / scale;
                    }
                    new_size = Some((width, height));
                }
                if transform_changed || Some(buffer.rect) != old_raw_size {
                    if self.src_rect.get().is_none() {
                        *buffer_points = self
                            .buffer_transform
                            .get()
                            .apply_inv_sized(0.0, 0.0, 1.0, 1.0);
                        *buffer_points_norm = *buffer_points;
                    } else {
                        *buffer_points_norm = buffer_points
                            .norm(buffer.rect.width() as f32, buffer.rect.height() as f32);
                        if !buffer_points_norm.is_leq_1() {
                            return Err(WlSurfaceError::ViewportOutsideBuffer);
                        }
                    }
                }
            }
            let (width, height) = new_size.unwrap_or_default();
            if (width, height) != self.buffer_abs_pos.get().size() {
                self.need_extents_update.set(true);
            }
            self.buffer_abs_pos
                .set(self.buffer_abs_pos.get().with_size(width, height).unwrap());
        }
        {
            let mut pfr = self.pending.frame_request.borrow_mut();
            self.frame_requests.borrow_mut().extend(pfr.drain(..));
        }
        {
            let mut fbs = self.presentation_feedback.borrow_mut();
            for fb in fbs.drain(..) {
                fb.send_discarded();
                let _ = self.client.remove_obj(&*fb);
            }
            let mut pfbs = self.pending.presentation_feedback.borrow_mut();
            mem::swap(fbs.deref_mut(), pfbs.deref_mut());
        }
        {
            if let Some(region) = self.pending.input_region.take() {
                self.input_region.set(region);
            }
            if let Some(region) = self.pending.opaque_region.take() {
                self.opaque_region.set(region);
            }
        }
        if let Some(tearing) = self.pending.tearing.take() {
            self.tearing.set(tearing);
        }
        if let Some(content_type) = self.pending.content_type.take() {
            self.content_type.set(content_type);
        }
        if let Some(xwayland_serial) = self.pending.xwayland_serial.take() {
            self.xwayland_serial.set(Some(xwayland_serial));
            self.client
                .surfaces_by_xwayland_serial
                .set(xwayland_serial, self.clone());
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::SurfaceSerialAssigned(self.id));
        }
        if self.need_extents_update.get() {
            self.calculate_extents();
        }
        if buffer_changed || transform_changed {
            for (_, cursor) in &self.cursors {
                cursor.handle_buffer_change();
                cursor.update_hardware_cursor();
            }
        }
        ext.post_commit();
        self.client.state.damage();
        Ok(())
    }

    fn commit(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let _req: Commit = self.parse(parser)?;
        self.do_commit(CommitContext::RootCommit)?;
        Ok(())
    }

    fn set_buffer_transform(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let req: SetBufferTransform = self.parse(parser)?;
        use Transform::*;
        let tf = match req.transform {
            TF_NORMAL => Normal,
            TF_90 => Rotate90,
            TF_180 => Rotate180,
            TF_270 => Rotate270,
            TF_FLIPPED => Flipped,
            TF_FLIPPED_90 => Flipped90,
            TF_FLIPPED_180 => Flipped180,
            TF_FLIPPED_270 => Flipped270,
            _ => return Err(WlSurfaceError::UnknownBufferTransform(req.transform)),
        };
        self.pending.transform.set(Some(tf));
        Ok(())
    }

    fn set_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let req: SetBufferScale = self.parse(parser)?;
        if req.scale < 1 {
            return Err(WlSurfaceError::NonPositiveBufferScale);
        }
        self.pending.scale.set(Some(req.scale));
        Ok(())
    }

    fn damage_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let _req: DamageBuffer = self.parse(parser)?;
        self.pending.damage.set(true);
        Ok(())
    }

    fn offset(&self, parser: MsgParser<'_, '_>) -> Result<(), WlSurfaceError> {
        let req: Offset = self.parse(parser)?;
        self.pending.offset.set((req.x, req.y));
        Ok(())
    }

    fn find_surface_at(self: &Rc<Self>, x: i32, y: i32) -> Option<(Rc<Self>, i32, i32)> {
        let rect = self.buffer_abs_pos.get().at_point(0, 0);
        let children = self.children.borrow();
        let children = match children.deref() {
            Some(c) => c,
            _ => {
                return if rect.contains(x, y) {
                    Some((self.clone(), x, y))
                } else {
                    None
                };
            }
        };
        let ss = |c: &LinkedList<StackElement>| {
            for child in c.rev_iter() {
                if child.pending.get() {
                    continue;
                }
                let pos = child.sub_surface.position.get();
                if pos.contains(x, y) {
                    let (x, y) = pos.translate(x, y);
                    if let Some(res) = child.sub_surface.surface.find_surface_at(x, y) {
                        return Some(res);
                    }
                }
            }
            None
        };
        if let Some(res) = ss(&children.above) {
            return Some(res);
        }
        if rect.contains(x, y) {
            return Some((self.clone(), x, y));
        }
        if let Some(res) = ss(&children.below) {
            return Some(res);
        }
        None
    }

    fn find_tree_at_(self: &Rc<Self>, x: i32, y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        match self.find_surface_at(x, y) {
            Some((node, x, y)) => {
                tree.push(FoundNode { node, x, y });
                FindTreeResult::AcceptsInput
            }
            _ => FindTreeResult::Other,
        }
    }

    fn send_seat_release_events(&self) {
        self.seat_state
            .for_each_pointer_focus(|s| s.leave_surface(self));
        self.seat_state
            .for_each_kb_focus(|s| s.unfocus_surface(self));
    }

    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        for inhibitor in self.idle_inhibitors.lock().values() {
            if visible {
                inhibitor.activate();
            } else {
                inhibitor.deactivate();
            }
        }
        let children = self.children.borrow_mut();
        if let Some(children) = children.deref() {
            for child in children.subsurfaces.values() {
                child.surface.set_visible(visible);
            }
        }
        if !visible {
            self.send_seat_release_events();
        }
        self.seat_state.set_visible(self, visible);
    }

    pub fn destroy_node(&self) {
        for (_, constraint) in &self.constraints {
            constraint.deactivate();
        }
        for (_, inhibitor) in self.idle_inhibitors.lock().drain() {
            inhibitor.deactivate();
        }
        let children = self.children.borrow();
        if let Some(ch) = children.deref() {
            for ss in ch.subsurfaces.values() {
                ss.surface.destroy_node();
            }
        }
        if let Some(tl) = self.toplevel.get() {
            let data = tl.tl_data();
            let mut remove = vec![];
            for (seat, s) in data.focus_node.iter() {
                if s.node_id() == self.node_id() {
                    remove.push(seat);
                }
            }
            for seat in remove {
                data.focus_node.remove(&seat);
            }
            if self.seat_state.is_active() {
                tl.tl_surface_active_changed(false);
            }
        }
        self.send_seat_release_events();
        self.seat_state.destroy_node(self);
    }

    pub fn set_content_type(&self, content_type: Option<ContentType>) {
        self.pending.content_type.set(Some(content_type));
    }

    pub fn request_activation(&self) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_data().request_attention(tl.tl_as_node());
        }
    }
}

object_base! {
    self = WlSurface;

    DESTROY => destroy,
    ATTACH => attach,
    DAMAGE => damage,
    FRAME => frame,
    SET_OPAQUE_REGION => set_opaque_region,
    SET_INPUT_REGION => set_input_region,
    COMMIT => commit,
    SET_BUFFER_TRANSFORM => set_buffer_transform if self.version >= 2,
    SET_BUFFER_SCALE => set_buffer_scale if self.version >= 3,
    DAMAGE_BUFFER => damage_buffer if self.version >= 4,
    OFFSET => offset if self.version >= 5,
}

impl Object for WlSurface {
    fn break_loops(&self) {
        self.unset_dnd_icons();
        self.unset_cursors();
        self.destroy_node();
        *self.children.borrow_mut() = None;
        self.unset_ext();
        mem::take(self.frame_requests.borrow_mut().deref_mut());
        self.buffer.set(None);
        self.toplevel.set(None);
        self.idle_inhibitors.clear();
        self.pending.presentation_feedback.borrow_mut().clear();
        self.presentation_feedback.borrow_mut().clear();
        self.viewporter.take();
        self.fractional_scale.take();
        self.tearing_control.take();
        self.constraints.clear();
    }
}

dedicated_add_obj!(WlSurface, WlSurfaceId, surfaces);

tree_id!(SurfaceNodeId);
impl Node for WlSurface {
    fn node_id(&self) -> NodeId {
        self.node_id.into()
    }

    fn node_seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn node_visit(self: Rc<Self>, visitor: &mut dyn NodeVisitor) {
        visitor.visit_surface(&self);
    }

    fn node_visit_children(&self, visitor: &mut dyn NodeVisitor) {
        let children = self.children.borrow_mut();
        if let Some(c) = children.deref() {
            for child in c.subsurfaces.values() {
                visitor.visit_surface(&child.surface);
            }
        }
    }

    fn node_visible(&self) -> bool {
        self.visible.get()
    }

    fn node_absolute_position(&self) -> Rect {
        self.buffer_abs_pos.get()
    }

    fn node_active_changed(&self, active: bool) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_surface_active_changed(active);
        }
    }

    fn node_render(
        &self,
        renderer: &mut Renderer,
        x: i32,
        y: i32,
        max_width: i32,
        max_height: i32,
    ) {
        renderer.render_surface(self, x, y, max_width, max_height);
    }

    fn node_client(&self) -> Option<Rc<Client>> {
        Some(self.client.clone())
    }

    fn node_toplevel(self: Rc<Self>) -> Option<Rc<dyn ToplevelNode>> {
        self.toplevel.get()
    }

    fn node_on_key(&self, seat: &WlSeatGlobal, time_usec: u64, key: u32, state: u32) {
        seat.key_surface(self, time_usec, key, state);
    }

    fn node_on_mods(&self, seat: &WlSeatGlobal, mods: ModifierState) {
        seat.mods_surface(self, mods);
    }

    fn node_on_button(
        self: Rc<Self>,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        serial: u32,
    ) {
        seat.button_surface(&self, time_usec, button, state, serial);
    }

    fn node_on_axis_event(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, event: &PendingScroll) {
        seat.scroll_surface(&self, event);
    }

    fn node_on_focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_data().focus_node.insert(seat.id(), self.clone());
            tl.tl_on_activate();
        }
        seat.focus_surface(&self);
    }

    fn node_on_unfocus(&self, seat: &WlSeatGlobal) {
        seat.unfocus_surface(self);
    }

    fn node_on_leave(&self, seat: &WlSeatGlobal) {
        seat.leave_surface(self);
    }

    fn node_on_pointer_enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.enter_surface(&self, x, y)
    }

    fn node_on_pointer_motion(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.motion_surface(&self, x, y)
    }

    fn node_on_pointer_relative_motion(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        seat.relative_motion_surface(self, time_usec, dx, dy, dx_unaccelerated, dy_unaccelerated);
    }

    fn node_on_dnd_drop(&self, dnd: &Dnd) {
        dnd.seat.dnd_surface_drop(self, dnd);
    }

    fn node_on_dnd_leave(&self, dnd: &Dnd) {
        dnd.seat.dnd_surface_leave(self, dnd);
    }

    fn node_on_dnd_enter(&self, dnd: &Dnd, x: Fixed, y: Fixed, serial: u32) {
        dnd.seat.dnd_surface_enter(self, dnd, x, y, serial);
    }

    fn node_on_dnd_motion(&self, dnd: &Dnd, time_usec: u64, x: Fixed, y: Fixed) {
        dnd.seat.dnd_surface_motion(self, dnd, time_usec, x, y);
    }

    fn node_into_surface(self: Rc<Self>) -> Option<Rc<WlSurface>> {
        Some(self.clone())
    }

    fn node_is_xwayland_surface(&self) -> bool {
        self.client.is_xwayland
    }
}

#[derive(Debug, Error)]
pub enum WlSurfaceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwlrLayerSurfaceV1Error(Box<ZwlrLayerSurfaceV1Error>),
    #[error(transparent)]
    XdgSurfaceError(Box<XdgSurfaceError>),
    #[error("Surface {} cannot be assigned the role {} because it already has the role {}", .id, .new.name(), .old.name())]
    IncompatibleRole {
        id: WlSurfaceId,
        old: SurfaceRole,
        new: SurfaceRole,
    },
    #[error("Cannot destroy a `wl_surface` before its role object")]
    ReloObjectStillExists,
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("Buffer scale is not positive")]
    NonPositiveBufferScale,
    #[error("Unknown buffer transform {0}")]
    UnknownBufferTransform(i32),
    #[error("Viewport source is not integer-sized and destination size is not set")]
    NonIntegerViewportSize,
    #[error("Viewport source is not contained in the attached buffer")]
    ViewportOutsideBuffer,
    #[error("attach request must not contain offset")]
    OffsetInAttach,
}
efrom!(WlSurfaceError, ClientError);
efrom!(WlSurfaceError, XdgSurfaceError);
efrom!(WlSurfaceError, ZwlrLayerSurfaceV1Error);
efrom!(WlSurfaceError, MsgParserError);
