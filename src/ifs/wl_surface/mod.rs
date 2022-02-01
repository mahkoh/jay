pub mod cursor;
mod types;
pub mod wl_subsurface;
pub mod xdg_surface;

use crate::backend::{KeyState, ScrollAxis, SeatId};
use crate::client::{Client, ClientId, DynEventFormatter, RequestParser};
use crate::fixed::Fixed;
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_output::WlOutputId;
use crate::ifs::wl_seat::{NodeSeatState, WlSeatGlobal};
use crate::ifs::wl_surface::cursor::CursorSurface;
use crate::ifs::wl_surface::wl_subsurface::WlSubsurface;
use crate::ifs::wl_surface::xdg_surface::{XdgSurface, XdgSurfaceRole};
use crate::object::{Interface, Object, ObjectId};
use crate::pixman::Region;
use crate::rect::Rect;
use crate::render::Renderer;
use crate::tree::{Node, NodeId};
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::utils::clonecell::CloneCell;
use crate::utils::linkedlist::LinkedList;
use crate::utils::smallmap::SmallMap;
use crate::xkbcommon::ModifierState;
use crate::NumCell;
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const ATTACH: u32 = 1;
const DAMAGE: u32 = 2;
const FRAME: u32 = 3;
const SET_OPAQUE_REGION: u32 = 4;
const SET_INPUT_REGION: u32 = 5;
const COMMIT: u32 = 6;
const SET_BUFFER_TRANSFORM: u32 = 7;
const SET_BUFFER_SCALE: u32 = 8;
const DAMAGE_BUFFER: u32 = 9;

#[allow(dead_code)]
const ENTER: u32 = 0;
#[allow(dead_code)]
const LEAVE: u32 = 1;

#[allow(dead_code)]
const INVALID_SCALE: u32 = 0;
#[allow(dead_code)]
const INVALID_TRANSFORM: u32 = 1;
#[allow(dead_code)]
const INVALID_SIZE: u32 = 2;

id!(WlSurfaceId);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceRole {
    None,
    Subsurface,
    XdgSurface,
    Cursor,
}

impl SurfaceRole {
    fn name(self) -> &'static str {
        match self {
            SurfaceRole::None => "none",
            SurfaceRole::Subsurface => "subsurface",
            SurfaceRole::XdgSurface => "xdg_surface",
            SurfaceRole::Cursor => "cursor",
        }
    }
}

pub struct WlSurface {
    pub id: WlSurfaceId,
    pub node_id: SurfaceNodeId,
    pub client: Rc<Client>,
    role: Cell<SurfaceRole>,
    pending: PendingState,
    input_region: Cell<Option<Region>>,
    opaque_region: Cell<Option<Region>>,
    pub extents: Cell<Rect>,
    pub need_extents_update: Cell<bool>,
    pub effective_extents: Cell<Rect>,
    pub buffer: CloneCell<Option<Rc<WlBuffer>>>,
    pub buf_x: NumCell<i32>,
    pub buf_y: NumCell<i32>,
    pub children: RefCell<Option<Box<ParentData>>>,
    ext: CloneCell<Rc<dyn SurfaceExt>>,
    pub frame_requests: RefCell<Vec<Rc<WlCallback>>>,
    seat_state: NodeSeatState,
    xdg: CloneCell<Option<Rc<XdgSurface>>>,
    cursors: SmallMap<SeatId, Rc<CursorSurface>, 1>,
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

    fn post_commit(&self) {
        // nothing
    }

    fn is_some(&self) -> bool {
        true
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
}

pub struct NoneSurfaceExt;

impl SurfaceExt for NoneSurfaceExt {
    fn is_some(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct PendingState {
    buffer: Cell<Option<Option<(i32, i32, Rc<WlBuffer>)>>>,
    opaque_region: Cell<Option<Option<Region>>>,
    input_region: Cell<Option<Option<Region>>>,
    frame_request: RefCell<Vec<Rc<WlCallback>>>,
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
    pub fn new(id: WlSurfaceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            node_id: client.state.node_ids.next(),
            client: client.clone(),
            role: Cell::new(SurfaceRole::None),
            pending: Default::default(),
            input_region: Cell::new(None),
            opaque_region: Cell::new(None),
            extents: Default::default(),
            need_extents_update: Cell::new(false),
            effective_extents: Default::default(),
            buffer: CloneCell::new(None),
            buf_x: Default::default(),
            buf_y: Default::default(),
            children: Default::default(),
            ext: CloneCell::new(client.state.none_surface_ext.clone()),
            frame_requests: RefCell::new(vec![]),
            seat_state: Default::default(),
            xdg: Default::default(),
            cursors: Default::default(),
        }
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
        self.cursors.insert(seat.id(), cursor.clone());
        Ok(cursor)
    }

    pub fn belongs_to_toplevel(&self) -> bool {
        if let Some(xdg) = self.xdg.get() {
            return xdg.role() == XdgSurfaceRole::XdgToplevel;
        }
        false
    }

    fn enter_event(self: &Rc<Self>, output: WlOutputId) -> DynEventFormatter {
        Box::new(Enter {
            obj: self.clone(),
            output,
        })
    }

    fn set_xdg_surface(&self, xdg: Option<Rc<XdgSurface>>) {
        let ch = self.children.borrow();
        if let Some(ch) = &*ch {
            for ss in ch.subsurfaces.values() {
                ss.surface.set_xdg_surface(xdg.clone());
            }
        }
        if self.seat_state.is_active() {
            if let Some(xdg) = &xdg {
                xdg.surface_active_changed(true);
            }
        }
        self.xdg.set(xdg);
    }

    fn set_role(&self, role: SurfaceRole) -> Result<(), WlSurfaceError> {
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
        let mut extents = Rect::new_empty(0, 0);
        if let Some(b) = self.buffer.get() {
            extents = b.rect;
        }
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.parse(parser)?;
        self.unset_cursors();
        self.destroy_node(true);
        if self.ext.get().is_some() {
            return Err(DestroyError::ReloObjectStillExists);
        }
        {
            let mut children = self.children.borrow_mut();
            if let Some(children) = &mut *children {
                for ss in children.subsurfaces.values() {
                    ss.surface.unset_ext();
                }
            }
            *children = None;
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn attach(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), AttachError> {
        let req: Attach = self.parse(parser)?;
        let buf = if req.buffer.is_some() {
            Some((req.x, req.y, self.client.get_buffer(req.buffer)?))
        } else {
            None
        };
        self.pending.buffer.set(Some(buf));
        Ok(())
    }

    fn damage(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageError> {
        let _req: Damage = self.parse(parser)?;
        Ok(())
    }

    fn frame(&self, parser: MsgParser<'_, '_>) -> Result<(), FrameError> {
        let req: Frame = self.parse(parser)?;
        let cb = Rc::new(WlCallback::new(req.callback));
        self.client.add_client_obj(&cb)?;
        self.pending.frame_request.borrow_mut().push(cb);
        Ok(())
    }

    fn set_opaque_region(&self, parser: MsgParser<'_, '_>) -> Result<(), SetOpaqueRegionError> {
        let region: SetOpaqueRegion = self.parse(parser)?;
        let region = if region.region.is_some() {
            Some(self.client.get_region(region.region)?.region())
        } else {
            None
        };
        self.pending.opaque_region.set(Some(region));
        Ok(())
    }

    fn set_input_region(&self, parser: MsgParser<'_, '_>) -> Result<(), SetInputRegionError> {
        let req: SetInputRegion = self.parse(parser)?;
        let region = if req.region.is_some() {
            Some(self.client.get_region(req.region)?.region())
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
        if let Some(buffer_change) = self.pending.buffer.take() {
            let mut old_size = None;
            let mut new_size = None;
            if let Some(buffer) = self.buffer.take() {
                old_size = Some(buffer.rect);
                if !buffer.destroyed() {
                    self.client.event(buffer.release());
                }
            }
            if let Some((dx, dy, buffer)) = buffer_change {
                let _ = buffer.update_texture();
                new_size = Some(buffer.rect);
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
            if old_size != new_size {
                self.need_extents_update.set(true);
            }
            for (_, cursor) in &self.cursors {
                cursor.handle_buffer_change();
            }
        }
        {
            let mut pfr = self.pending.frame_request.borrow_mut();
            self.frame_requests.borrow_mut().extend(pfr.drain(..));
        }
        {
            if let Some(region) = self.pending.input_region.take() {
                self.input_region.set(region);
            }
            if let Some(region) = self.pending.opaque_region.take() {
                self.opaque_region.set(region);
            }
        }
        if self.need_extents_update.get() {
            self.calculate_extents();
        }
        ext.post_commit();
        Ok(())
    }

    fn commit(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CommitError> {
        let _req: Commit = self.parse(parser)?;
        self.do_commit(CommitContext::RootCommit)?;
        Ok(())
    }

    fn set_buffer_transform(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetBufferTransformError> {
        let _req: SetBufferTransform = self.parse(parser)?;
        Ok(())
    }

    fn set_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), SetBufferScaleError> {
        let _req: SetBufferScale = self.parse(parser)?;
        Ok(())
    }

    fn damage_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageBufferError> {
        let _req: DamageBuffer = self.parse(parser)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSurfaceError> {
        match request {
            DESTROY => self.destroy(parser)?,
            ATTACH => self.attach(parser)?,
            DAMAGE => self.damage(parser)?,
            FRAME => self.frame(parser)?,
            SET_OPAQUE_REGION => self.set_opaque_region(parser)?,
            SET_INPUT_REGION => self.set_input_region(parser)?,
            COMMIT => self.commit(parser)?,
            SET_BUFFER_TRANSFORM => self.set_buffer_transform(parser)?,
            SET_BUFFER_SCALE => self.set_buffer_scale(parser)?,
            DAMAGE_BUFFER => self.damage_buffer(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }

    fn find_surface_at(self: &Rc<Self>, x: i32, y: i32) -> Option<(Rc<Self>, i32, i32)> {
        let buffer = match self.buffer.get() {
            Some(b) => b,
            _ => return None,
        };
        let children = self.children.borrow();
        let children = match children.deref() {
            Some(c) => c,
            _ => {
                return if buffer.rect.contains(x, y) {
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
        if buffer.rect.contains(x, y) {
            return Some((self.clone(), x, y));
        }
        if let Some(res) = ss(&children.below) {
            return Some(res);
        }
        None
    }
}

handle_request!(WlSurface);

impl Object for WlSurface {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlSurface
    }

    fn num_requests(&self) -> u32 {
        DAMAGE_BUFFER + 1
    }

    fn break_loops(&self) {
        self.unset_cursors();
        self.destroy_node(true);
        *self.children.borrow_mut() = None;
        self.unset_ext();
        mem::take(self.frame_requests.borrow_mut().deref_mut());
        self.buffer.set(None);
        self.xdg.set(None);
    }
}

tree_id!(SurfaceNodeId);
impl Node for WlSurface {
    fn id(&self) -> NodeId {
        self.node_id.into()
    }

    fn seat_state(&self) -> &NodeSeatState {
        &self.seat_state
    }

    fn destroy_node(&self, _detach: bool) {
        let children = self.children.borrow();
        if let Some(ch) = children.deref() {
            for ss in ch.subsurfaces.values() {
                ss.surface.destroy_node(false);
            }
        }
        if let Some(xdg) = self.xdg.get() {
            let mut remove = vec![];
            for (seat, s) in &xdg.focus_surface {
                if s.id == self.id {
                    remove.push(seat);
                }
            }
            for seat in remove {
                xdg.focus_surface.remove(&seat);
            }
            if self.seat_state.is_active() {
                xdg.surface_active_changed(false);
            }
        }
        self.seat_state.destroy_node(self);
    }

    fn active_changed(&self, active: bool) {
        if let Some(xdg) = self.xdg.get() {
            xdg.surface_active_changed(active);
        }
    }

    fn key(&self, seat: &WlSeatGlobal, key: u32, state: u32, mods: Option<ModifierState>) {
        seat.key_surface(self, key, state, mods);
    }

    fn button(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, button: u32, state: KeyState) {
        seat.button_surface(&self, button, state);
    }

    fn scroll(&self, seat: &WlSeatGlobal, delta: i32, axis: ScrollAxis) {
        seat.scroll_surface(self, delta, axis);
    }

    fn focus(self: Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        if let Some(xdg) = self.xdg.get() {
            xdg.focus_surface.insert(seat.id(), self);
        }
    }

    fn unfocus(&self, seat: &WlSeatGlobal) {
        seat.unfocus_surface(self);
    }

    fn leave(&self, seat: &WlSeatGlobal) {
        seat.leave_surface(self);
    }

    fn enter(self: Rc<Self>, seat: &Rc<WlSeatGlobal>, x: Fixed, y: Fixed) {
        seat.enter_surface(&self, x, y)
    }

    fn motion(&self, seat: &WlSeatGlobal, x: Fixed, y: Fixed) {
        seat.motion_surface(self, x, y)
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_surface(self, x, y);
    }

    fn client_id(&self) -> Option<ClientId> {
        Some(self.client.id)
    }
}
